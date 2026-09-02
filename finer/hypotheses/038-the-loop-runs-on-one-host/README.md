# 038 — the loop runs on one host

**Status: OPEN — the fleet dispatcher does not exist; the owner's pool of Azure machines is named in the
brief and nowhere else.**
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
3. **The pool.** `fleet.mjs --hosts a,b,c`: a host is a slot; a port is dispatched to a free host
   with `RAYON_NUM_THREADS` set to that host's cores; dists come back over rsync; measurement stays
   local. Keep `--slots` as the fallback when no hosts are given.
4. **Measure the pass.** Wall clock of the whole pass, and per-port build seconds, single host versus
   pool, same commit. Wall clock is the result here because the loop's wall clock is the quantity.

```sh
az vm list -d -o table
node finer/tools/fleet.mjs --hosts <host>,<host> --measure
```

## Result

| variant | ports | pass wall clock | artifacts identical | notes |
|---|---:|---:|---|---|
| this host, 4 slots x 2 cores | | | — | |
| pool | | | | |

## Verdict

<open>

## Next

Ask the owner which machines form the pool, or whether the loop may create them; then step 2.
