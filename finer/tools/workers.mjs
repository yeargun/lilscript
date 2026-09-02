#!/usr/bin/env node
// The build pool: dispatch port builds to the `lilscript-workers` scale set.
//
// objective.md §9: compiles are the loop's clock and they run on the owner's
// Azure machines, one port per worker, never serialized on this host. This
// tool owns that pool. Measurement stays here: each worker builds a port in
// its own copy of the sibling checkouts and rsyncs `dist/` back, and the
// local `fleet.mjs --measure` scores it with the pinned codec as before.
//
//   node finer/tools/workers.mjs status              # instances, power, IPs
//   node finer/tools/workers.mjs up [N|all]          # start N instances (default all)
//   node finer/tools/workers.mjs down [N|all]        # deallocate (stops billing)
//   node finer/tools/workers.mjs provision           # node 22 + rsync on every running worker
//   node finer/tools/workers.mjs sync                # compiler binaries + every port to every worker
//   node finer/tools/workers.mjs build --ports a,b   # build those ports on the pool, copy dist/ back
//   node finer/tools/workers.mjs fleet [--down]      # up, sync, build every port, measure, [down]
//   node finer/tools/workers.mjs run '<shell>'       # run a command on every running worker
//
// Options: --compiler <path> (default target/release/lilscript), --rg, --vmss,
// --user, --timeout <s> per port build (default 5400), --no-sync, --measure,
// --dist-dir <dir> (bring each port's dist/ back under <dir>/<port>/ instead of
// into the port itself: an A/B snapshot that leaves the working tree alone),
// --instances 3,4,5 (use only those instance ids; up/down/sync/build/run honour it).
// `build` syncs only the ports it builds (plus the compiler); `sync` and `fleet`
// sync every sibling port, which the source-graph ports need.
//
// The pool is discovered from `az` each time (`--cache` reuses the last
// discovery in finer/out/workers/instances.json). Workers are addressed by
// private IP: the scale set shares this host's subnet. A port that fails on a
// worker is retried once on another; a worker that stops answering is
// dropped for the run. Wall clock is logged, never a result (objective.md §8).
import { spawn, spawnSync } from "node:child_process"
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync, appendFileSync } from "node:fs"
import { dirname, join, resolve, basename } from "node:path"
import { fileURLToPath } from "node:url"

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..")
const siblings = resolve(repo, "..")
const outDir = join(repo, "finer", "out", "workers")
mkdirSync(outDir, { recursive: true })

const argv = process.argv.slice(2)
const command = argv[0]
const flag = (name, fallback) => { const at = argv.indexOf(`--${name}`); return at === -1 ? fallback : argv[at + 1] }
const has = (name) => argv.includes(`--${name}`)

const RG = flag("rg", "lilscript-build-farm")
const VMSS = flag("vmss", "lilscript-workers")
const USER = flag("user", "lilfarm")
const REMOTE = "lil" // ~/lil/<checkout> on the worker mirrors /home/azureuser/<checkout> here
const COMPILER = resolve(flag("compiler", join(repo, "target", "release", "lilscript")))
const CODEC = join(repo, "target", "release", "lilscript-codec")
const BUILD_TIMEOUT_S = Number(flag("timeout", 5400))
const SSH_OPTS = ["-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=accept-new", "-o", "ConnectTimeout=10", "-o", "ServerAliveInterval=30"]

const log = (line) => { const stamp = new Date().toISOString().slice(11, 19); process.stderr.write(`[workers ${stamp}] ${line}\n`) }
const fail = (message) => { log(message); process.exit(1) }

function az(args, { json = true } = {}) {
  const r = spawnSync("az", [...args, ...(json ? ["-o", "json"] : [])], { encoding: "utf8", maxBuffer: 64 << 20 })
  if (r.status !== 0) fail(`az ${args.join(" ")} failed:\n${r.stderr.trim().slice(-600)}`)
  return json ? JSON.parse(r.stdout || "null") : r.stdout
}

/** Instances with private IP and power state, from az (or the cached last discovery). */
function discover({ cache = has("cache") } = {}) {
  const cachePath = join(outDir, "instances.json")
  if (cache && existsSync(cachePath)) return JSON.parse(readFileSync(cachePath, "utf8"))
  const nics = az(["vmss", "nic", "list", "-g", RG, "--vmss-name", VMSS])
  const power = az(["vmss", "list-instances", "-g", RG, "-n", VMSS, "--expand", "instanceView",
    "--query", "[].{id:instanceId,name:name,statuses:instanceView.statuses[].code}"])
  const ipById = {}
  for (const nic of nics) {
    const id = nic.virtualMachine?.id?.split("/").pop()
    const ip = nic.ipConfigurations?.[0]?.privateIPAddress
    if (id != null && ip) ipById[id] = ip
  }
  const instances = power.map((p) => ({
    id: p.id, name: p.name, ip: ipById[p.id] ?? null,
    power: (p.statuses || []).find((s) => s.startsWith("PowerState/"))?.slice("PowerState/".length) ?? "unknown",
  })).sort((a, b) => Number(a.id) - Number(b.id))
  writeFileSync(cachePath, JSON.stringify(instances, null, 2))
  return instances
}

const ONLY = (flag("instances", "") || "").split(",").filter(Boolean)
const selected = (instances) => (ONLY.length ? instances.filter((i) => ONLY.includes(String(i.id))) : instances)
const running = (instances) => selected(instances).filter((i) => i.power === "running" && i.ip)

function ssh(ip, script, { timeoutS = 60 } = {}) {
  const r = spawnSync("ssh", [...SSH_OPTS, `${USER}@${ip}`, script], { encoding: "utf8", timeout: timeoutS * 1000, maxBuffer: 64 << 20 })
  return { ok: r.status === 0, out: (r.stdout || "").trim(), err: (r.stderr || "").trim(), status: r.status }
}

function sshAsync(ip, script, { timeoutS, logFile }) {
  return new Promise((done) => {
    const child = spawn("ssh", [...SSH_OPTS, `${USER}@${ip}`, script], { stdio: ["ignore", "pipe", "pipe"] })
    let tail = ""
    const onData = (d) => { const s = d.toString(); tail = (tail + s).slice(-4000); if (logFile) appendFileSync(logFile, s) }
    child.stdout.on("data", onData); child.stderr.on("data", onData)
    const killer = setTimeout(() => { try { child.kill("SIGKILL") } catch {} }, timeoutS * 1000)
    child.on("exit", (code, signal) => { clearTimeout(killer); done({ code, signal, tail }) })
  })
}

function waitForSsh(instances, { attempts = 40, everyMs = 8000 } = {}) {
  const pending = new Set(instances.map((i) => i.ip))
  for (let attempt = 0; attempt < attempts && pending.size; attempt++) {
    for (const ip of [...pending]) if (ssh(ip, "true", { timeoutS: 12 }).ok) pending.delete(ip)
    if (pending.size) spawnSync("sleep", [String(everyMs / 1000)])
  }
  if (pending.size) log(`no SSH after ${attempts} attempts: ${[...pending].join(", ")}`)
  return instances.filter((i) => !pending.has(i.ip))
}

// ---- commands ---------------------------------------------------------------

function status() {
  const instances = discover()
  console.log(`${VMSS} (${RG}): ${instances.length} instances`)
  for (const i of instances) console.log(`  ${i.id}  ${i.power.padEnd(12)} ${i.ip ?? "-"}`)
  const up = running(instances)
  if (up.length) {
    for (const i of up) {
      const r = ssh(i.ip, "nproc; node --version 2>/dev/null || echo no-node; test -x ~/lil/lilscript/target/release/lilscript && echo compiler || echo no-compiler", { timeoutS: 15 })
      console.log(`  ${i.id}  ${r.ok ? r.out.replace(/\n/g, " ") : "ssh: " + (r.err.split("\n")[0] || "unreachable")}`)
    }
  }
  return instances
}

function up() {
  const want = argv[1] && argv[1] !== "all" ? Number(argv[1]) : null
  const instances = discover()
  const stopped = selected(instances).filter((i) => i.power !== "running")
  const targets = want == null ? stopped : stopped.slice(0, Math.max(0, want - running(instances).length))
  if (targets.length) {
    log(`starting ${targets.map((i) => i.id).join(",")}`)
    az(["vmss", "start", "-g", RG, "-n", VMSS, "--instance-ids", ...targets.map((i) => i.id)], { json: false })
  }
  const now = running(discover())
  const reachable = waitForSsh(now)
  log(`${reachable.length} worker(s) reachable: ${reachable.map((i) => i.ip).join(", ")}`)
  return reachable
}

function down() {
  const which = argv[1]
  const instances = discover()
  const targets = which && which !== "all" ? instances.filter((i) => String(i.id) === which) : running(instances)
  if (!targets.length) { log("nothing running"); return }
  log(`deallocating ${targets.map((i) => i.id).join(",")}`)
  az(["vmss", "deallocate", "-g", RG, "-n", VMSS, "--instance-ids", ...targets.map((i) => i.id)], { json: false })
  discover()
}

const PROVISION = `set -e
if ! command -v rsync >/dev/null; then sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -q rsync >/dev/null; fi
if ! node --version 2>/dev/null | grep -q '^v2[2-9]'; then
  curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash - >/dev/null 2>&1
  sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -q nodejs >/dev/null
fi
mkdir -p ~/${REMOTE}
echo "nproc=$(nproc) node=$(node --version) rsync=$(rsync --version | head -1 | awk '{print $3}')"`

function provision(workers = running(discover())) {
  for (const w of workers) {
    const r = ssh(w.ip, PROVISION, { timeoutS: 600 })
    log(`${w.ip} provision: ${r.ok ? r.out.split("\n").pop() : "FAILED " + r.err.slice(-300)}`)
  }
}

/** Every sibling checkout with a build script, plus this repo's binaries and finer/tools. */
function portDirs() {
  return readdirSync(siblings).filter((n) => /lil$|^lil-/.test(n) && n !== "lilscript" && existsSync(join(siblings, n, "scripts", "build.mjs")))
}

function rsync(args, { timeoutS = 1800 } = {}) {
  const r = spawnSync("rsync", ["-az", "--delete", "-e", `ssh ${SSH_OPTS.join(" ")}`, ...args], { encoding: "utf8", timeout: timeoutS * 1000, maxBuffer: 64 << 20 })
  return { ok: r.status === 0, err: (r.stderr || "").trim() }
}

function syncWorker(w, ports = portDirs()) {
  const started = Date.now()
  // The compiler and codec go where LILSCRIPT_ROOT expects them; nothing else of this repo is needed to build a port.
  const remoteRoot = `${USER}@${w.ip}:~/${REMOTE}/lilscript/`
  let r = ssh(w.ip, `mkdir -p ~/${REMOTE}/lilscript/target/release ~/${REMOTE}/lilscript/finer/tools`, { timeoutS: 20 })
  if (!r.ok) return { ok: false, err: r.err }
  r = rsync([COMPILER, CODEC, `${remoteRoot}target/release/`])
  if (!r.ok) return r
  r = rsync([join(repo, "finer", "tools") + "/", `${remoteRoot}finer/tools/`])
  if (!r.ok) return r
  for (const port of ports) {
    r = rsync(["--exclude", ".git", "--exclude", "finer/out", join(siblings, port) + "/", `${USER}@${w.ip}:~/${REMOTE}/${port}/`])
    if (!r.ok) return { ok: false, err: `${port}: ${r.err}` }
  }
  return { ok: true, seconds: (Date.now() - started) / 1000 }
}

function sync(workers = running(discover()), ports = null) {
  const only = (flag("ports", "") || "").split(",").filter(Boolean)
  ports = ports ?? (only.length ? only : portDirs())
  log(`syncing ${ports.length} ports + compiler to ${workers.length} worker(s)`)
  const results = workers.map((w) => ({ w, r: syncWorker(w, ports) }))
  for (const { w, r } of results) log(`${w.ip} sync: ${r.ok ? `ok ${r.seconds.toFixed(0)}s` : "FAILED " + r.err.slice(-300)}`)
  return results.filter(({ r }) => r.ok).map(({ w }) => w)
}

/** Build one port on one worker and bring its dist/ back. */
async function buildOn(w, port) {
  const logFile = join(outDir, `${port}.log`)
  writeFileSync(logFile, `# ${port} on ${w.ip} (${VMSS}/${w.id}) ${new Date().toISOString()}\n`)
  const script = `cd ~/${REMOTE}/${port} && export LILSCRIPT_ROOT=~/${REMOTE}/lilscript LILSCRIPT_COMPILER=~/${REMOTE}/lilscript/target/release/lilscript RAYON_NUM_THREADS=$(nproc) LILSCRIPT_TIMING=1 && node scripts/build.mjs --compile`
  const started = Date.now()
  const r = await sshAsync(w.ip, script, { timeoutS: BUILD_TIMEOUT_S, logFile })
  const seconds = (Date.now() - started) / 1000
  if (r.code !== 0) return { ok: false, seconds, error: r.signal ? `killed (${r.signal}) after ${seconds.toFixed(0)}s` : r.tail.trim().slice(-600) }
  const distDir = flag("dist-dir", null)
  const target = distDir ? join(resolve(distDir), port) : join(siblings, port, "dist")
  mkdirSync(target, { recursive: true })
  const back = rsync([`${USER}@${w.ip}:~/${REMOTE}/${port}/dist/`, target + "/"])
  if (!back.ok) return { ok: false, seconds, error: `dist rsync back: ${back.err.slice(-300)}` }
  return { ok: true, seconds }
}

/** Biggest ports first so the pool's tail is one giant, not a queue of them. */
const KNOWN_COST = { jquerylil: 6000, markedlil: 3300, katexlil: 2000, "react-markdownlil": 1500, posthoglil: 1600, micromarklil: 1300, mobxlil: 1000, "remark-gfmlil": 1000, remarklil: 800, "remark-parselil": 700, rehypelil: 700, unifiedlil: 600, "mdast-util-from-markdownlil": 550, "remark-rehypelil": 300, "mdast-util-to-hastlil": 280, "remark-mathlil": 190, "remark-breakslil": 190 }

async function build(workers, ports) {
  const queue = [...ports].sort((a, b) => (KNOWN_COST[b] ?? 0) - (KNOWN_COST[a] ?? 0))
  const results = {}
  const retried = new Set()
  const worker = async (w) => {
    for (;;) {
      const port = queue.shift()
      if (!port) return
      log(`${w.ip} building ${port}`)
      const r = await buildOn(w, port)
      log(`${w.ip} ${port} ${r.ok ? "ok" : "FAILED"} ${r.seconds.toFixed(0)}s${r.ok ? "" : ": " + r.error.split("\n").pop()}`)
      if (!r.ok && !retried.has(port) && !/killed/.test(r.error)) { retried.add(port); queue.push(port); continue }
      results[port] = { worker: w.id, ...r }
    }
  }
  await Promise.all(workers.map(worker))
  writeFileSync(join(outDir, "last-build.json"), JSON.stringify({ at: new Date().toISOString(), results }, null, 2))
  return results
}

function measure() {
  const r = spawnSync(process.execPath, [join(repo, "finer", "tools", "fleet.mjs"), "--measure"], { stdio: "inherit", env: { ...process.env, FORCE_COLOR: undefined } })
  return r.status === 0
}

async function main() {
  switch (command) {
    case "status": status(); break
    case "up": up(); break
    case "down": down(); break
    case "provision": provision(); break
    case "sync": sync(); break
    case "run": {
      const script = argv[1]; if (!script) fail("run needs a shell command")
      for (const w of running(discover())) { const r = ssh(w.ip, script, { timeoutS: 600 }); console.log(`--- ${w.id} ${w.ip}\n${r.out}${r.err ? "\n" + r.err : ""}`) }
      break
    }
    case "build": {
      const ports = (flag("ports", "") || "").split(",").filter(Boolean)
      if (!ports.length) fail("build needs --ports a,b")
      let workers = running(discover())
      if (!workers.length) fail("no running worker; `workers.mjs up` first")
      if (!has("no-sync")) workers = sync(workers)
      const results = await build(workers, ports)
      const bad = Object.entries(results).filter(([, r]) => !r.ok)
      log(`${Object.keys(results).length - bad.length} ok, ${bad.length} failed`)
      if (has("measure")) measure()
      process.exit(bad.length ? 1 : 0)
    }
    case "fleet": {
      let workers = up()
      if (!workers.length) fail("no worker reachable")
      provision(workers)
      workers = sync(workers, portDirs())
      const ports = (flag("ports", "") || "").split(",").filter(Boolean)
      const results = await build(workers, ports.length ? ports : portDirs())
      const bad = Object.entries(results).filter(([, r]) => !r.ok)
      log(`${Object.keys(results).length - bad.length} ok, ${bad.length} failed`)
      measure()
      if (has("down")) down()
      process.exit(bad.length ? 1 : 0)
    }
    default:
      console.error(readFileSync(fileURLToPath(import.meta.url), "utf8").split("\n").filter((l) => l.startsWith("//")).map((l) => l.slice(3)).join("\n"))
      process.exit(command ? 1 : 0)
  }
}

main().catch((error) => fail(String(error?.stack || error)))
