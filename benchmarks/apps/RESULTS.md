# Application benchmark results

Generated on 2026-08-14T13:02:56.079Z with LilScript `51ee9b9`, Node `v24.11.1`, Vite `8.2.1`, esbuild `0.28.1`, and Google Closure Compiler `20260804.0.0` on `darwin 24.6.0 arm64`.

This report contains two deliberately separate datasets. Compiler rows use a readable JavaScript reference and a LilScript implementation with the same app algorithm and abstraction scope. Ecosystem rows build real npm packages with Vite and are never included in compiler totals.

LilScript's raw, gzip-9, and Brotli-11 cells come from three independent objective builds. Each build is judged only on its matching metric; cross-metric sizes remain attached to the machine report as diagnostics. Runtime uses the Brotli-objective artifact.

Every emitted artifact passed its checked-in stdout contract. That rejects observed behavior mismatches for these inputs; it does not prove complete semantic or library API equivalence.

Context-only ecosystem builds use Alien Signals `3.2.1`, mitt `3.0.1`, and Motion `13.0.0`.

## Source size

Source bytes describe only checked-in app code and exclude npm dependencies. They measure authoring surface, not shipping size.

| Workload | Reference JS | LilScript | Hand-specialized JS |
| --- | ---: | ---: | ---: |
| Reactive store | 1106 | 1258 | 246 |
| Event pipeline | 910 | 1022 | 113 |
| Binary telemetry | 1145 | 1070 | 410 |
| Module pricing | 1027 | 916 | 124 |
| Animation value kernel | 2067 | 2539 | 128 |

## Reactive store

Expected output: `reactive:1890621774:408`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 1151 | 452 | 375 | +53.1% | 3.51 | +6.2% |
| Reference JS esbuild | 523 | 302 | 259 | +5.7% | 3.35 | +1.2% |
| JS Closure ADVANCED | 469 | 295 | 245 | 0.0% | 3.31 | 0.0% |
| JS hand-specialized | 245 | 192 | 170 | -30.6% | 2.96 | -10.4% |
| LilScript objective builds | 453 | 260 | 217 | -11.4% | 3.06 | -7.4% |

Context-only production build: **Alien Signals via Vite**. This uses a different library implementation and is excluded from every compiler delta and total.

Vite output contract: `reactive:1890621774:408`

| Vite production assets | Raw | Gzip-9 | Brotli-11 | Median ms |
| --- | ---: | ---: | ---: | ---: |
| assets/index-DVPqLfDU.js<br>index.html | 4885 | 1916 | 1737 | 22.51 |

## Event pipeline

Expected output: `events:975625712:9718960`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 1029 | 387 | 317 | +62.6% | 1.84 | +10.3% |
| Reference JS esbuild | 499 | 274 | 239 | +22.6% | 1.75 | +5.0% |
| JS Closure ADVANCED | 345 | 239 | 195 | 0.0% | 1.67 | 0.0% |
| JS hand-specialized | 112 | 124 | 102 | -47.7% | 0.67 | -59.9% |
| LilScript objective builds | 257 | 199 | 162 | -16.9% | 1.64 | -1.6% |

Context-only production build: **mitt via Vite**. This uses a different library implementation and is excluded from every compiler delta and total.

Vite output contract: `events:975625712:9718960`

| Vite production assets | Raw | Gzip-9 | Brotli-11 | Median ms |
| --- | ---: | ---: | ---: | ---: |
| assets/index-C8ji9Tfp.js<br>index.html | 691 | 469 | 382 | 6.90 |

## Binary telemetry

Expected output: `binary:446359193:32`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 1196 | 481 | 438 | +50.0% | 10.48 | -1.5% |
| Reference JS esbuild | 521 | 318 | 291 | -0.3% | 10.46 | -1.8% |
| JS Closure ADVANCED | 498 | 312 | 292 | 0.0% | 10.64 | 0.0% |
| JS hand-specialized | 409 | 274 | 257 | -12.0% | 9.87 | -7.2% |
| LilScript objective builds | 416 | 291 | 262 | -10.3% | 10.33 | -2.9% |

## Module pricing

Expected output: `module:init modules:593759979:4940`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 878 | 387 | 327 | +58.0% | 0.98 | +6.9% |
| Reference JS esbuild | 352 | 248 | 208 | +0.5% | 0.98 | +7.5% |
| JS Closure ADVANCED | 329 | 251 | 207 | 0.0% | 0.92 | 0.0% |
| JS hand-specialized | 123 | 120 | 112 | -45.9% | 0.56 | -39.1% |
| LilScript objective builds | 222 | 178 | 151 | -27.1% | 0.95 | +4.1% |

## Animation value kernel

Expected output: `motion:14400000:28719240:880000:5494928`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 2149 | 788 | 713 | +90.1% | 1.01 | +11.3% |
| Reference JS esbuild | 688 | 433 | 395 | +5.3% | 0.98 | +7.6% |
| JS Closure ADVANCED | 595 | 412 | 375 | 0.0% | 0.91 | 0.0% |
| JS hand-specialized | 127 | 130 | 111 | -70.4% | 0.63 | -30.8% |
| LilScript objective builds | 456 | 308 | 278 | -25.9% | 0.80 | -11.6% |
| LilScript specialized source (diagnostic) | 124 | 130 | 100 | -73.3% | 0.60 | -34.2% |

Context-only production build: **Motion value and spring APIs via Vite**. This uses a different library implementation and is excluded from every compiler delta and total.

Vite output contract: `motion-vite:14400000:28719240:880000:5494928`

| Vite production assets | Raw | Gzip-9 | Brotli-11 | Median ms |
| --- | ---: | ---: | ---: | ---: |
| assets/index-CZj057qH.js<br>index.html | 10572 | 4535 | 4152 | 2.26 |

## Corpus totals

Runtime is a geometric mean of per-workload ratios to Closure; size columns are sums.

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Runtime ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 6403 | 2495 | 2170 | +65.1% | 1.065x |
| Reference JS esbuild | 2583 | 1575 | 1392 | +5.9% | 1.038x |
| JS Closure ADVANCED | 2236 | 1509 | 1314 | 0.0% | 1.000x |
| JS hand-specialized | 1016 | 840 | 752 | -42.8% | 0.675x |
| LilScript objective builds | 1804 | 1236 | 1070 | -18.6% | 0.960x |

## Interpretation limits

- Hand-specialized JavaScript is an oracle for expert whole-program rewriting, not source given to Closure or LilScript.
- Real package builds are Vite context measurements only. No package row is compared with a specialized rewrite or included in corpus totals.
- The Motion compiler workload matches the selected numeric mix/wrap/stagger and underdamped-spring equations and digest; LilScript still does not implement Motion's package API. A complete claim requires the public package surface and upstream behavioral tests, including DOM, timing, cancellation, gestures, scrolling, SVG, and React entry points.
- Generated C and native executables are behavior gates; only JavaScript artifacts are included in transfer-size and Node runtime tables.
- Closure receives the exact readable JavaScript reference used by the unminified and esbuild rows.
- Matching one deterministic stdout contract can have false negatives; it is regression evidence, not a proof of general equivalence.
- The checked methodology gate requires every objective-specific LilScript workload to be no larger than Closure in its matching raw, gzip-9, or Brotli-11 metric; full 20+-sample runs also require the Brotli-objective artifact's median runtime to remain within 5% of Closure.
- Runtime is repeated cache-busted module parsing plus execution inside one dedicated Node process per artifact. It excludes process startup but is not a browser-frame benchmark.
- These results apply to this corpus and compiler revision; they do not prove universal superiority over Closure.
