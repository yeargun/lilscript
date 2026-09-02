# 038 — the loop runs on one host

**Status: IN PROGRESS — the pool is `lilscript-workers`, a six-instance Standard_F8s_v2 scale set on
this host's own subnet; `finer/tools/workers.mjs` starts it, syncs the compiler and every port, builds
one port per worker and brings `dist/` back; first end-to-end build running 2026-09-02.**
Lane: measure. Objective: brotli. Ports: all 23 that build. Opened: 2026-09-01.

## Claim

A full fleet pass is the slowest step of the loop and it runs on the orchestrator's shared 8-core
host: `fleet.mjs` pins ports to core slices and one pass of 23 ports takes on the order of an hour,
twice that for an A/B (037 measured HEAD against the fix this way). The owner's second brief of
2026-09-01 ([intent](../../intent/2026-09-01.md)) says a pool of Azure machines exists for exactly
this and must be used extensively; objective.md §9 now makes that the contract. The claim is that
dispatching one port per machine cuts a fleet pass to the longest single port build, and an A/B to
the same, so the wall clock of a compiler hypothesis falls from about two hours to about ten minutes.
Confirming number: a fleet pass in under 15 minutes wall clock with byte-identical artifacts to the
single-host pass. Falsifying number: artifacts that differ between hosts (then §8's determinism claim
is what is broken, and that outranks the speed-up) or a pass no faster than the pinned single host.

## Read

- `finer/objective.md` §8–§9, `finer/status.md`
- `finer/tools/fleet.mjs` — `buildPort`, `runPool`, `measure`: the slot pool to replace with a host pool
- The sibling ports' `scripts/build.mjs` — all honor `LILSCRIPT_COMPILER` and `LILSCRIPT_ROOT` except
  motionlil, solidlil and lil-solidjs, which do not build under the fleet anyway (status.md)

## May touch

- `finer/tools/fleet.mjs` (or a new `finer/tools/hosts.mjs` it calls), `finer/README.md` tool table
- Nothing in `src/`; nothing in a port

## Method

1. **Inventory first.** From this host `az account show` is logged in and `az vm list -d` shows seven
   VMs (eastus, westeurope, francecentral), none named or tagged as a build host, and `~/.ssh/config`
   names no hosts. The pool's membership — existing machines, or VMs created on demand — is the
   owner's to state; creating VMs costs money and is not done without that answer. Record the answer
   in this folder.
2. **One host end to end.** On one pool machine: clone or rsync this checkout and the sibling ports
   at the same commit, `cargo build --release`, run one port's `scripts/build.mjs --compile`, copy
   `dist/` back, measure with the local `lilscript-codec`. Confirm the artifact is byte-identical to
   the single-host build of the same commit.
3. **The pool.** `workers.mjs build --ports …`: a worker is a slot; a port is dispatched to a free host
   with `RAYON_NUM_THREADS` set to that host's cores; dists come back over rsync; measurement stays
   local. Keep `--slots` as the fallback when no hosts are given.
4. **Measure the pass.** Wall clock of the whole pass, and per-port build seconds, single host versus
   pool, same commit. Wall clock is the result here because the loop's wall clock is the quantity.

```sh
az vm list -d -o table
node finer/tools/workers.mjs fleet --down
```

## The pool, found

Owner brief of 2026-09-02 ([intent](../../intent/2026-09-02.md)): `lilscript-workers`, a Uniform VM
scale set in resource group `lilscript-build-farm`, West Europe, capacity 6, Standard_F8s_v2 (8 vCPU
Intel Cascade Lake, 16 GiB), Ubuntu 24.04 with Node 24 preinstalled, admin `lilfarm`, SSH key = this
host's `~/.ssh/id_ed25519`, private IPs 10.1.0.5–.10 on this host's subnet (this host is
10.1.0.4, a burstable Standard_B8als_v2 whose CPU credits are why identical work varied 3x). All six
were deallocated when found; deallocated instances cost nothing.

## Prices (West Europe, Linux, USD/hour, Azure retail price API, 2026-09-02)

| SKU | vCPU | CPU | pay-as-you-go | Spot |
|---|---:|---|---:|---:|
| Standard_F8s_v2 (the pool) | 8 | Intel Cascade Lake | 0.388 | 0.072 |
| Standard_D8als_v6 | 8 | AMD Genoa | 0.389 | 0.072 |
| Standard_F8as_v6 | 8 | AMD Genoa, higher clocks, 32 GiB | 0.661 | 0.122 |
| Standard_D16als_v6 | 16 | AMD Genoa | 0.778 | 0.144 |
| Standard_D32als_v6 | 32 | AMD Genoa | 1.555 | 0.287 |
| Standard_F16s_v2 / F32s_v2 | 16 / 32 | Cascade Lake | 0.776 / 1.552 | 0.143 / 0.287 |
| Standard_B8als_v2 (this host) | 8 | AMD Milan, burstable | 0.306 | — |
| Standard_HB120rs_v3 | 120 | Milan-X HPC | 4.680 | 0.865 |

Fsv6 (Intel) is not priced in West Europe. Trade-offs for this workload: one process per port, so
per-core speed first (Genoa over Cascade Lake), memory irrelevant, extra cores only for the four big
ports, Spot 5.4x cheaper and fine for retryable batch builds. Recommendation: six D16als_v6 on Spot
(≈ $0.86/h for 96 Genoa cores; a fleet pass ≈ jquery alone on 16 cores) with pay-as-you-go as the
fallback; Spot needs a second scale set beside this one. The Genoa gain is an estimate (≈ 1.5x per
core from compile benchmarks) until one port is built on each.

## Thread scaling, measured on a dedicated F8s_v2 (2026-09-02)

One port, one worker, nothing else running; `/usr/bin/time`; the artifact's md5 is identical at every
thread count (determinism across thread counts holds on the pool, objective.md §8).

| port (level, search) | 1 thread | 2 | 4 | 8 | speed-up at 8 | parallel share (Amdahl) |
|---|---:|---:|---:|---:|---:|---:|
| micromarklil (13, `always`) | 149.7 s | 123.2 s | 104.8 s | 104.0 s | 1.44x | ≈ 35% |
| mobxlil (13, `always`) | 193.9 s | 128.8 s | 92.7 s | see below | 2.09x at 4 | ≈ 70% |

So a port's build is mostly serial at level 13: past four threads a worker's extra cores are idle.
The fleet's wall clock is cut by *more ports in flight*, not by more cores per port — two ports per
8-core worker at four threads each costs micromark 1 s and mobx about 12 s — and by per-core speed,
which is why Genoa over Cascade Lake matters more than the core count.

## Result

| variant | ports | pass wall clock | artifacts identical | notes |
|---|---:|---:|---|---|
| this host, 4 slots x 2 cores | 23 | 45–70 min, jquerylil and markedlil time out at 90 | — | 2026-09-01/02 passes |
| one F8s_v2 worker, unifiedlil | 1 | 67 s build (+170 s first sync of 26 ports) | yes: 14647 / 5159 / 4666, the local pass's bytes | this host's slot: 517–602 s |
| six F8s_v2 workers, fleet | | | | |

## Verdict

Not yet: the first end-to-end worker build decides whether artifacts are byte-identical across hosts
(the falsifier) before the fleet pass measures the speed-up.

## Next

Finish the first end-to-end build, then a full fleet pass on the six workers with `workers.mjs fleet`;
then the owner's SKU decision (a Spot Dalsv6 set) and `fleet.mjs --workers`.
