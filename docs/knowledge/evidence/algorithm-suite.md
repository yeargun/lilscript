# Structural algorithm suite

Parent: [evidence](README.md). Verification contract:
[algorithm challenges](../verification/algorithm-challenges.md). Executable source:
[`comparison/algorithms/`](../../../comparison/algorithms/).

This lane is distinct from the 525-case micro catalog. It combines multiple
functions, runtime-varying inputs, and interacting optimization opportunities so
that a high micro-case count cannot be mistaken for whole-program evidence. It is
invoked by `comparison/run-all.sh` and therefore by the release check. Without an
explicit `LILSCRIPT` + `LILSCRIPT_CODEC` pair, its runner builds fresh release
compiler and scorer binaries together. Supplying only one override is rejected; an
explicit pair is hashed and fixture-checked but remains diagnostic unless its shared
build identity is independently attested.

Every result must identify the structural tier, opportunity tags, function/module
and boundary counts, vectors, exact source and emitted-artifact hashes, compiler and
minifier options/versions, ES target, codec implementation, and the metric-specific
winner. Raw, gzip-9, and Brotli-11 are hard per-case gates against the smallest valid
independently authored JavaScript artifact. Failed semantic candidates do not enter
the frontier, but their failure still makes the case red.

Those are three independent LilScript compilations and three independent decisions:
the raw-cost artifact gates only raw, the gzip-cost artifact gates only gzip-9, and
the Brotli-cost artifact gates only Brotli-11. Each artifact's other measured sizes
are diagnostic and may lose; no single-artifact Pareto win is required.

Every quoted transfer cell uses the shared statically linked canonical scorer:
upstream stock zlib C 1.3.1 at gzip level 9 with deterministic `mtime = 0`, and
official Google Brotli C 1.1.0 in generic mode at quality 11 with `lgwin = 22`.
System zlib, Node's patched codec build, and alternate Brotli encoders are ineligible
even when nominal versions or parameters match.

## Durable corpus checkpoint

The maintained corpus now contains **11 independently designed pairs and 42 fixed
runtime vectors**:

- five small cases: aggregate ledger (4 vectors), collection geometry (3), state
  machine parser (3), static rule engine (3), and string dictionary (4);
- five medium cases: dictionary template router (5), helper sharing (4), policy
  specialization (3), shape invoice pipeline (5), and stateful packet decoder (3);
- one large event-analytics graph (5), with 22 declared functions and six modules.

Inputs cross an opaque runtime host boundary, so neither side can precompute the
complete answer. Candidate eligibility checks fixed stdout plus the exact ordered
sequence of `algorithmCount`, `algorithmInt(index)`, and `algorithmString(index)`
accesses. The sources also pass structural parity checks for module names, import
edges, per-module function names, host-boundary names, tier, and reachable call depth.

An independent runtime-coverage audit exercised every function on every vector in
the ten small/medium cases. Every large-case vector exercised all 20 entry-reachable
functions across all six modules. Its only two unreachable declarations are the
intentionally tagged diagnostic export and that export's otherwise-unused helper,
which make export reachability and tree shaking real work rather than inert padding.

The JavaScript-only fairness audit covered the complete current frontier: 83
case-candidate artifacts and 327 candidate-vector executions across Terser, Oxc,
esbuild, Closure `ADVANCED`, and the applicable direct esbuild, direct Closure,
Vite/Oxc, Vite/Terser, and restricted property-mangling lanes. Every execution
matched both stdout and indexed host-access order. The restricted property lane
renames only static `_`-prefixed owned fields, with built-ins excluded and quoted
keys retained. Dynamic dictionaries use own-property lookup and quoted key ABI;
negative remainder routing and multiply-before-division i32 behavior have explicit
edge vectors.

## Size status

The first canonical full 11-case run is a retained **red migration checkpoint**, not
a release claim: three cases passed, eight failed, with 21 objective-specific failure
events.
It compiled and validated separate raw-, gzip-, and Brotli-objective LilScript
artifacts and compared each only on its selected metric. The failures isolated
missing loop-aggregate decomposition and pure guard-return helper specialization;
they were retained instead of being averaged away.

The first extracted compiler fix is now independently verified on
`aggregate-ledger`. A closed-use loop-phi struct decomposition changed its matching
objective results from `254 > 221`, `183 > 182`, and `163 > 160` to:

| objective | LilScript | best JavaScript | relation |
|---|---:|---:|---|
| raw | 212 | 221 (Closure ADVANCED) | strict win |
| gzip-9 | 162 | 182 (Closure ADVANCED) | strict win |
| Brotli-11 | 139 | 160 (Closure ADVANCED) | strict win |

All four fixed vectors and exact ordered host traces passed, and the complete Rust
library test suite passed after the transform.

The focused report that closed the formerly losing large structural case made it
non-regressing in all three objective lanes:

| objective | LilScript | best JavaScript | relation |
|---|---:|---:|---|
| raw | 803 | 853 (Closure ADVANCED) | strict win |
| gzip-9 | 475 | 489 (Closure ADVANCED) | strict win |
| Brotli-11 | 424 | 425 (Closure ADVANCED) | strict win |

All five `large-event-analytics` vectors and their ordered opaque-host traces passed.
Those same cells are now confirmed in the authoritative post-fix full report.

The current canonical schema-2 report has `selectedBy = "all"`: **11/11 algorithms
passed with zero failure events**. Every one of the 11 raw lanes, 11 gzip-9 lanes,
and 11 Brotli-11 lanes is an observed strict win over its independently minimized
valid JavaScript baseline. All 42 fixed vectors and ordered opaque-host traces passed.
The report records compiler SHA-256
`6962cea23cf08148e2d2fc76b9f1c2a7ca6876c5e488d56da1f1bcb5341de897`
and scorer SHA-256
`975bf256f7c8dfdeb5864a9e96d1a26e43a8a90726dc2227daf1026c2f1d20e7`.

Generated `summary.json`/`summary.md` can still be overwritten by a later focused
run. Inspect `selectedBy`, case count, binary hashes, and codec provenance before
quoting them as a corpus result.
