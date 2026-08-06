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
| `motion-values` | Compares an animation-value kernel; separately builds real Motion `mix`/`wrap`/`stagger` and deterministic `spring` APIs with Vite. |

The Vite package builds are context only. They are excluded from compiler
deltas and corpus totals because their implementation scope differs. In
particular, the LilScript animation-value kernel is not a Motion port. Full
Motion compatibility is tracked in
[`../../docs/motion-compatibility.md`](../../docs/motion-compatibility.md), with
machine-enforced status in [`compatibility/motion-v13.json`](compatibility/motion-v13.json).

## Artifact matrix

Every workload produces and executes these artifacts:

| Artifact | Construction |
| --- | --- |
| `Reference JS bundle` | the readable reference bundled without minification |
| `Reference JS esbuild` | the exact reference bundled and minified by esbuild |
| `JS Closure ADVANCED` | that exact readable reference compiled with `ADVANCED` |
| `JS hand-specialized` | checked-in, manually minified code specialized to the app contract |
| `LilScript` | a readable LilScript implementation with matching app scope |

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
per artifact after warmup; process startup is outside the interval. Readable
JavaScript references use
`Math.imul` and signed 32-bit coercions where LilScript `int` operations require
them; the hand-specialized baseline may remove coercions that are provably
irrelevant for the fixed app inputs.

## Run

```sh
npm --prefix benchmarks/apps install
npm --prefix benchmarks/apps run benchmark
```

Use `npm --prefix benchmarks/apps run verify` for compilation, behavior, and one
runtime smoke execution per artifact. Full results are written to
[`RESULTS.md`](RESULTS.md), with machine-readable details under the ignored
`build/results.json`.
