# LilScript application benchmark lab

This lab measures behavior-equivalent applications, not isolated syntax
snippets. It answers three separate questions:

1. How large is an ordinary JavaScript application after normal bundling and
   minification?
2. How far can Google Closure Compiler `ADVANCED` reduce the same application?
3. How does LilScript output compare with both compiler output and a manually
   specialized JavaScript lower bound?

## Workloads

| Workload | Purpose |
| --- | --- |
| `reactive-store` | Uses `alien-signals` from JavaScript and a typed, whole-program-specialized signal graph in LilScript. |
| `event-pipeline` | Uses `mitt` from JavaScript and a typed event surface in LilScript. |
| `binary-telemetry` | Exercises `ArrayBuffer`, `SharedArrayBuffer`, `Uint8Array`, loops, and integer semantics without a dependency. |
| `module-pricing` | Exercises relative imports, required module side effects, unused exports, structs/classes, and whole-program DCE. |

The first two cases are application comparisons against real npm packages.
They are not claims that the LilScript sources implement every edge case or
public API of those libraries. The observable application contract is the unit
of equivalence.

## Artifact matrix

Every workload produces and executes these artifacts:

| Artifact | Construction |
| --- | --- |
| `JS raw bundle` | esbuild dependency resolution and bundling, without minification |
| `JS esbuild` | the same JavaScript entry bundled and minified by esbuild |
| `JS Closure ADVANCED` | a readable, Closure-friendly implementation compiled with `ADVANCED` |
| `JS hand-specialized` | checked-in, manually minified code specialized to the app contract |
| `LilScript` | the LilScript source compiled by the repository's release compiler |

The LilScript lane also emits C and a native executable from the same source.
The native executable must pass the same stdout contract, but native and C file
sizes are not mixed into the JavaScript transfer-size table.

The Closure lane is deliberately separate from the ecosystem JavaScript lane.
Running `ADVANCED` directly over a generic Alien Signals bundle is not sound:
the library probes internal property names with strings, while Closure renames
the corresponding dot properties. The checked-in Closure source expresses the
same app contract with Closure-compatible property access. It is bundled
without minification for dependency resolution before Closure runs.

All artifacts must produce the checked-in expected output before any result is
reported. Sizes are normalized UTF-8 bytes, deterministic gzip level 9, and
Brotli quality 11. Runtime is the median wall time of fresh Node processes after
warmup; it includes process startup, so the report also shows ratios rather than
claiming microbenchmark precision. Readable JavaScript references use
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
