# 008 — The build pool

Parent: [index](index.md). Directive: [D5](001-directives.md#d5--compiles-run-on-the-pool-not-on-this-host).
Status: the pool exists and works. The A/B tooling this migration needs does not, and Phase 0 builds it.

---

## The rule

Every gate in this plan is a fleet sweep, and **no gate runs on the orchestrator host.**

This is not new policy. [`finer/objective.md` §9](../finer/objective.md) already states it as a
defect rule rather than a preference:

> "Compiles are the loop's clock, and the loop runs on a pool of Azure machines, not on one host…
> **a tool that serializes them on the orchestrator's host is the defect, not the workload.** … the
> wall clock of the *loop* is a cost the owner pays, and it is bought down with machines before
> patience."

What this migration adds is that the rule now has teeth: the sweep matrix is **61 config files
across 26 ports and seven optimization levels**, run twice per A/B. That does not fit on one machine
and must never be attempted on one.

Evidence from this session, on the orchestrator host: cnlil — 1,591 source lines — exceeded a
two-minute timeout, and markedlil took 115.9 s wall. This host is a burstable `Standard_B8als_v2`
whose credits drain after roughly 30 minutes of sustained compilation; it was measured at load
average 28.65 on 8 cores. **Any wall-clock or per-port timing taken here is triage, never evidence.**

---

## What exists

| | |
|---|---|
| Resource group | `lilscript-build-farm`, westeurope |
| Default scale set | `lilscript-workers-v7` — 6 × `Standard_D16als_v7`, 96 AMD Turin vCPU total |
| Fallback scale set | 6 × `Standard_F8s_v2`, 48 Cascade Lake vCPU |
| Driver | `finer/tools/workers.mjs` — discovers instances via `az vmss nic list` / `list-instances`, addresses them by private IP on this host's subnet |
| Remote layout | `~/lil/` mirrors `/home/azureuser/`; compiler + `lilscript-codec` rsynced to `~/lil/lilscript/target/release` |
| Per-port build | rsync port tree (excluding `.git`, `finer/out`) → `node scripts/build.mjs --compile` with `LILSCRIPT_TIMING=1` → rsync `dist/` back |
| Provisioning | `finer/tools/worker-provision.sh` — idempotent; installs rsync, Node 22, `/usr/bin/time`, and a `lil-idle-stop` systemd timer |
| Idle watchdog | deallocates after `IDLE_MINUTES` (default 20) with no compiler process, no fresh `~/lil/.heartbeat`, and nobody logged in |
| Measurement | stays on **this** host, single pinned `lilscript-codec` call (zlib 1.3.1, Brotli 1.1.0 q11 lgwin22) — 13.4 s for 22 artifacts |

### What it costs and what it buys

| | Pool | This host |
|---|---:|---:|
| Full 23-port pass | **1,533 s** (25.5 min) on six workers | 45–70 min **plus two timeouts** |
| Two-arm A/B, end to end | ~45–55 min | 2 h+ |
| Cost of that A/B | ~$5.50 (6 × D16als_v7 at $0.778/h, plus the 20-min idle tail) | free, and wrong |

Slowest ports on the current pool: jquerylil 807 s, markedlil 283 s, motionlil 71 s. First sync is
2.81 GB across 27 port trees; incremental thereafter.

Capacity note: Spot is capped at 3 cores pending Azure review, so the pool is pay-as-you-go and
**quota is the limiting factor** on a wider sweep. Scaling the migration's matrix beyond six workers
needs a quota request, not just a flag.

---

## What is missing, and must be built in Phase 0

The pool can build. It cannot yet *certify*, and every gap below has already produced a wrong result.

### F1 — `fleet.mjs --workers` cannot run an A/B

Its `--workers` branch builds a fixed argv and forwards **none** of `--compiler`, `--dist-dir`,
`--instances`, `--per-worker`, `--vmss` (`finer/tools/fleet.mjs:171-173`). So the one-command pool
path always builds with `target/release/lilscript` and always writes into each port's working-tree
`dist/` — the two things an A/B under concurrent sessions must never do.

**Fix:** forward the flags. Make `--compiler` and `--dist-dir` mandatory when `--workers` is used
for a measurement.

### F2 — the A/B tools live in an ignored scratch directory

`fleet-compare.mjs` and `fleet-tests.mjs` are in `finer/out/048/`, which is git-ignored, with
hardcoded paths and a regex scrape of `fleet.mjs`'s baseline table.

**Fix:** promote both into `finer/tools/`, parameterised, with the baseline read from data rather
than scraped.

### F3 — a skipped compile reports a false byte-identical row

**Reproduced.** katexlil is the only port with an mtime cache: `compileIfRequested()` skips
compilation unless `--force`, and it does not consult `--compile`
(`katexlil/scripts/build.mjs:58-79`). `workers.mjs:248` always runs `--compile`, never `--force`.
Result: katexlil "builds" in 0.904 s with no `lilscript-timing` line, and **hypothesis 060 published
a false byte-identical / ±0 row** where both arms were the untouched local dist.

**Fix:** the gate fails any port whose build is under 10 s or produces no `lilscript-timing` line
([D4](001-directives.md#d4--repair-the-instrument-before-trusting-it)), and the fleet passes
`--force` where a port supports it. A skipped compile is a failed run, not a pass.

### F4 — an A/B destroys arm A's compile-cost telemetry

`buildOn` writes to `join(outDir, "<port>.log")` and **truncates it at the start of every build**
(`workers.mjs:234-235`). `--dist-dir` isolates artifacts but not logs. Since `LILSCRIPT_TIMING=1` is
exported on every pool build, arm B overwrites exactly the `emit_ms` / `emit_calls` / `lex_calls`
evidence that [D9](001-directives.md#d9--record-cost-with-size) and this migration's "compilation
must get faster" constraint depend on.

**Fix:** key worker log directories per arm.

### F5 — no fleet number records which compiler produced it

Nothing anywhere stores the compiler binary's digest alongside a fleet result. With 28 Claude
processes and 13 worktrees sharing this checkout, another session running `cargo build --release`
between arm A and arm B silently makes the A/B compare two unknown binaries.

**Fix:** every row records the compiler SHA-256, and both arms assert the same digest they were
launched with. Arm binaries are copied to an arm-specific path before the run, never referenced from
`target/release`.

### F6 — the noise floor is wider than most migration steps

About **±100 Brotli per single build**, and a single-build delta under ~150 bytes is not evidence.
The entire 121-fold text layer is worth 189 bytes on markedlil, so most steps in this migration land
inside the noise.

**Fix:** this is why [D3](001-directives.md#d3--compression-is-a-hard-constraint-and-byte-identity-is-the-only-clean-proof)
requires byte-identity rather than byte-similarity for neutral steps. Do not try to measure a neutral
step; prove it instead. Reserve fleet A/B for steps that *intend* a byte change.

---

## Use the harness that already knows about this migration

`comparison/large-libraries/matrix.json` pins compilers by revision **and** git tree **and**
primary-source SHA-256, and already declares two roles — `migration-incumbent` and
`migration-candidate` — with:

```json
"regressionPolicy": { "maxRegressionBytes": { "raw": 0, "gzip9": 0, "brotli11": 0 } },
"semanticStatusRequired": "passed"
```

A **zero-byte** regression policy with a mandatory semantic gate is exactly this plan's acceptance
criterion, already expressed in a fingerprinted, fail-closed harness. Do not invent a parallel
mechanism. Extend this one: add the missing ports, wire it to `portgate.mjs`
([002](002-the-instrument.md#02--make-the-ports-a-gate)), and make it the thing every phase gate runs.

---

## The standard sweep

Once F1–F5 are closed, a phase gate is one command per arm:

```sh
# once per session — six workers, ~3 min for the first sync
node finer/tools/workers.mjs up --instances 6

# arm A: the incumbent, from an arm-specific binary
node finer/tools/fleet.mjs --workers \
  --compiler  $ARMS/incumbent/lilscript \
  --dist-dir  $ARMS/incumbent/dist \
  --log-dir   $ARMS/incumbent/logs \
  --configs   all            # all 61, not just each port's default

# arm B: the candidate
node finer/tools/fleet.mjs --workers \
  --compiler  $ARMS/candidate/lilscript \
  --dist-dir  $ARMS/candidate/dist \
  --log-dir   $ARMS/candidate/logs \
  --configs   all

# measure both arms here, one pinned codec call, and diff
node finer/tools/fleet-compare.mjs $ARMS/incumbent $ARMS/candidate --require-identical-unless-declared

# behaviour, not just bytes
node finer/tools/portgate.mjs --arms $ARMS --diff-failing-set
```

`--require-identical-unless-declared` is the point: a phase declares in advance which configs are
allowed to move bytes and by how much. Every other config must be **byte-identical**, and a diff
there fails the gate regardless of direction or magnitude.

Keep a heartbeat running for the duration — the watchdog deallocates 20 minutes after the last one,
and it has already taken a run's tail on 2026-09-02.

---

## Ports the pool cannot currently certify

Record these as data, not prose, so a green is never over-read
([002 §0.4](002-the-instrument.md#04--freeze-the-baseline)):

| Port | Why it cannot certify | Needed |
|---|---|---|
| katexlil | mtime cache skips the compile (F3) | `--force`, plus the under-10 s failure rule |
| motionlil | fails to build under the fleet (`ERR_MODULE_NOT_FOUND`) | fix the port build |
| zodlil | suite runs at level 8, peephole disabled | test at shipped config ([002 G4](002-the-instrument.md#what-is-broken-verified)) |
| mobxlil | suite builds `--dev`, never the level-13 artifact | test at shipped config ([002 G3](002-the-instrument.md#what-is-broken-verified)) |
| cnlil, monacolil, playcanvaslil, solidlil, lil-solidjs | no baseline row in `fleet.mjs` | add baselines |
| 18 of 25 ports | `npm test` never rebuilds | `test:compiler` script ([002 §0.2](002-the-instrument.md#02--make-the-ports-a-gate)) |

Current fleet test-gate state, for honesty: 23 ports, 2,401 s total, katexlil killed at the 2,400 s
timeout, 8 ports failing — most on mid-migration sources, mobxlil on a jest-resolve environment
fault. **That is the baseline this migration starts from, and Phase 0's job is to make it mean
something.**
