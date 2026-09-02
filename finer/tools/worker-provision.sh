#!/usr/bin/env bash
# Provision one lilscript worker (run on the worker itself, as the admin user).
# Idempotent. Installs what a port build needs and an idle watchdog that
# deallocates the instance through its own managed identity when no build has
# run for IDLE_MINUTES, so a pool left alone stops billing on its own
# (objective.md §9; the owner's brief of 2026-09-02: "shut itself automatically").
#
#   bash worker-provision.sh [IDLE_MINUTES]      # default 20
#
# The watchdog needs the scale set's system-assigned identity to hold the
# "Virtual Machine Contributor" role on the scale set (workers.mjs grants it
# once: `workers.mjs grant`). Deallocating, not stopping: a stopped VM still
# bills; a deallocated one bills only its disk, and the synced state on that
# disk is what makes the next start warm.
set -euo pipefail
IDLE_MINUTES=${1:-20}

if ! command -v rsync >/dev/null; then
  sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -q rsync >/dev/null
fi
if ! node --version 2>/dev/null | grep -qE '^v(2[2-9]|[3-9][0-9])'; then
  curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash - >/dev/null 2>&1
  sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -q nodejs >/dev/null
fi
command -v /usr/bin/time >/dev/null || sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -q time >/dev/null
mkdir -p ~/lil
touch ~/lil/.heartbeat

# --- idle watchdog ------------------------------------------------------------
sudo tee /usr/local/bin/lil-idle-stop >/dev/null <<'EOF'
#!/usr/bin/env bash
# Deallocate this scale-set instance when idle: no lilscript compile running and
# the heartbeat (touched by workers.mjs at every build start and end) older than
# IDLE_MINUTES. Uses the instance's managed identity against the ARM REST API,
# so nothing on the host has to reach in to stop it.
set -u
IDLE_MINUTES=${IDLE_MINUTES:-20}
HEARTBEAT=/home/lilfarm/lil/.heartbeat
pgrep -f 'target/release/lilscript ' >/dev/null && exit 0
pgrep -f 'scripts/build.mjs' >/dev/null && exit 0
[ -f "$HEARTBEAT" ] || touch "$HEARTBEAT"
age=$(( ( $(date +%s) - $(stat -c %Y "$HEARTBEAT") ) / 60 ))
[ "$age" -lt "$IDLE_MINUTES" ] && exit 0
# Also treat an SSH session as activity: someone may be looking.
who | grep -q . && exit 0
meta=$(curl -s -H Metadata:true "http://169.254.169.254/metadata/instance/compute?api-version=2021-02-01")
sub=$(echo "$meta" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["subscriptionId"])')
rg=$(echo "$meta" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["resourceGroupName"])')
vmss=$(echo "$meta" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["vmScaleSetName"])')
name=$(echo "$meta" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["name"])')
id=${name##*_}
token=$(curl -s -H Metadata:true "http://169.254.169.254/metadata/identity/oauth2/token?api-version=2018-02-01&resource=https%3A%2F%2Fmanagement.azure.com%2F" | python3 -c 'import json,sys; print(json.load(sys.stdin)["access_token"])')
logger -t lil-idle-stop "idle ${age}m: deallocating ${vmss}/${id}"
curl -s -o /dev/null -w "%{http_code}\n" -X POST -H "Authorization: Bearer $token" -H "Content-Length: 0" \
  "https://management.azure.com/subscriptions/$sub/resourceGroups/$rg/providers/Microsoft.Compute/virtualMachineScaleSets/$vmss/virtualMachines/$id/deallocate?api-version=2024-07-01" | logger -t lil-idle-stop
EOF
sudo chmod +x /usr/local/bin/lil-idle-stop

sudo tee /etc/systemd/system/lil-idle-stop.service >/dev/null <<EOF
[Unit]
Description=Deallocate this lilscript worker when idle
[Service]
Type=oneshot
Environment=IDLE_MINUTES=${IDLE_MINUTES}
ExecStart=/usr/local/bin/lil-idle-stop
EOF
sudo tee /etc/systemd/system/lil-idle-stop.timer >/dev/null <<'EOF'
[Unit]
Description=Check every 5 minutes whether this lilscript worker is idle
[Timer]
OnBootSec=10min
OnUnitActiveSec=5min
AccuracySec=30s
[Install]
WantedBy=timers.target
EOF
sudo systemctl daemon-reload
sudo systemctl enable --now lil-idle-stop.timer >/dev/null 2>&1
echo "nproc=$(nproc) node=$(node --version) rsync=$(rsync --version | head -1 | awk '{print $3}') idle_stop=${IDLE_MINUTES}m"
