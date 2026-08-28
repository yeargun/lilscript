# LilScript application benchmark lab

This lab keeps compiler comparisons and ecosystem package measurements
separate. It answers three separate questions:

1. How do Closure and LilScript compile readable programs with the same
   algorithm and abstraction scope?
2. How far is either compiler from a manually specialized JavaScript oracle?
3. What does a real npm integration ship when built as an independent Vite
   production app?

## Workloads

| Workload | Purpose |
| --- | --- |
| `reactive-store` | Compares equivalent typed signal graphs; separately builds an Alien Signals app with Vite. |
| `event-pipeline` | Compares equivalent typed event dispatchers; separately builds a mitt app with Vite. |
| `binary-telemetry` | Exercises `ArrayBuffer`, `SharedArrayBuffer`, `Uint8Array`, loops, and integer semantics without a dependency. |
| `module-pricing` | Exercises relative imports, required module side effects, unused exports, structs/classes, and whole-program DCE. |
| `motion-values` | Compares the numeric algorithms used by Motion `mix`, `wrap`, `stagger`, and the selected underdamped `spring`; separately builds those real APIs with Vite. |

The Vite package builds are context only. They are excluded from compiler
deltas and corpus totals because their implementation scope differs. In
particular, the LilScript animation-value kernel now matches the selected
numeric algorithms and spring sample digest, but it is not a Motion package
port or a substitute for Motion's dynamic overloads and generator API. Full
Motion compatibility is tracked in
[`../../docs/knowledge/evidence/motion-compatibility.md`](../../docs/knowledge/evidence/motion-compatibility.md), with
machine-enforced status in [`compatibility/motion-v13.json`](compatibility/motion-v13.json).

## Artifact matrix

Every workload produces and executes these artifacts:

| Artifact | Construction |
| --- | --- |
| `Reference JS bundle` | the readable reference bundled without minification |
| `Reference JS esbuild` | the exact reference bundled and minified by esbuild |
| `JS Closure ADVANCED` | that exact readable reference compiled with `ADVANCED` |
| `JS hand-specialized` | checked-in, manually minified code specialized to the app contract |
| `LilScript objective builds` | the same readable LilScript implementation compiled independently for raw, gzip, and Brotli |

The Motion-derived kernel also emits a specialized LilScript diagnostic. It
shows how much of the gap is authoring/optimization versus backend syntax, and
is deliberately excluded from corpus totals.

The LilScript lane also emits C and a native executable from the same source.
The native executable must pass the same stdout contract, but native and C file
sizes are not mixed into the JavaScript transfer-size table.

Real Alien Signals, mitt, and Motion entries are independent HTML applications
built with pinned Vite production defaults. Their emitted HTML/CSS/JavaScript
assets are compressed per file and reported in a context-only table. They are
executed against checked-in output contracts, but never compared numerically
with specialized implementations.

All artifacts must produce the checked-in expected output before any result is
reported. This catches regressions for the measured inputs; it does not prove
general semantic or API compatibility. Methodology tests also require Closure's
input to be byte-identical to the readable reference and prevent Vite ecosystem
records from entering compiler totals. Sizes are normalized UTF-8 bytes,
deterministic gzip level 9, and Brotli quality 11. Runtime is the median of
cache-busted module parsing plus execution inside one dedicated Node process
per artifact after warmup; the LilScript runtime row uses its Brotli-objective
artifact. The raw, gzip-9, and Brotli-11 LilScript size cells come from separate
objective builds and each is gated only against the matching Closure metric;
cross-metric sizes are retained in `build/results.json` only as diagnostics and
may regress. Process startup is outside the interval. Readable
JavaScript references use ordinary multiplication plus signed 32-bit
normalization for LilScript `*`, and retain `Math.imul` only where the LilScript
source calls it explicitly. The hand-specialized baseline may remove coercions
that are provably irrelevant for the fixed app inputs.

## Run

```sh
npm --prefix benchmarks/apps install
npm --prefix benchmarks/apps run benchmark
```

Use `npm --prefix benchmarks/apps run verify` for compilation, behavior, and one
runtime smoke execution per artifact. Full results are written to
[`RESULTS.md`](RESULTS.md), with machine-readable details under the ignored
`build/results.json`.
