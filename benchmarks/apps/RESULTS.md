# Application benchmark results

Generated on 2026-08-06T15:04:47.884Z with LilScript `9e970e6`, Node `v24.11.1`, Vite `8.2.0`, esbuild `0.28.1`, and Google Closure Compiler `20260803.0.0` on `darwin 24.6.0 arm64`.

This report contains two deliberately separate datasets. Compiler rows use a readable JavaScript reference and a LilScript implementation with the same app algorithm and abstraction scope. Ecosystem rows build real npm packages with Vite and are never included in compiler totals.

Every emitted artifact passed its checked-in stdout contract. That rejects observed behavior mismatches for these inputs; it does not prove complete semantic or library API equivalence.

Context-only ecosystem builds use Alien Signals `3.2.1`, mitt `3.0.1`, and Motion `13.0.0`.

## Source size

Source bytes describe only checked-in app code and exclude npm dependencies. They measure authoring surface, not shipping size.

| Workload | Reference JS | LilScript | Hand-specialized JS |
| --- | ---: | ---: | ---: |
| Reactive store | 1122 | 1258 | 246 |
| Event pipeline | 914 | 1010 | 113 |
| Binary telemetry | 1157 | 1070 | 410 |
| Module pricing | 1033 | 916 | 124 |
| Animation value kernel | 613 | 661 | 120 |

## Reactive store

Expected output: `reactive:1890621774:408`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 1163 | 464 | 379 | +51.6% | 3.37 | +9.6% |
| Reference JS esbuild | 544 | 310 | 265 | +6.0% | 3.12 | +1.4% |
| JS Closure ADVANCED | 489 | 302 | 250 | 0.0% | 3.08 | 0.0% |
| JS hand-specialized | 245 | 194 | 170 | -32.0% | 2.94 | -4.4% |
| LilScript | 519 | 305 | 275 | +10.0% | 3.34 | +8.6% |

Context-only production build: **Alien Signals via Vite**. This uses a different library implementation and is excluded from every compiler delta and total.

Vite output contract: `reactive:1890621774:408`

| Vite production assets | Raw | Gzip-9 | Brotli-11 | Median ms |
| --- | ---: | ---: | ---: | ---: |
| assets/index-BAb3oBAp.js<br>index.html | 4900 | 1932 | 1752 | 22.44 |

## Event pipeline

Expected output: `events:975625712:9718960`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 1033 | 393 | 320 | +61.6% | 1.63 | +7.8% |
| Reference JS esbuild | 506 | 280 | 244 | +23.2% | 1.52 | +0.8% |
| JS Closure ADVANCED | 351 | 244 | 198 | 0.0% | 1.51 | 0.0% |
| JS hand-specialized | 112 | 125 | 102 | -48.5% | 0.63 | -58.3% |
| LilScript | 315 | 227 | 192 | -3.0% | 1.66 | +9.8% |

Context-only production build: **mitt via Vite**. This uses a different library implementation and is excluded from every compiler delta and total.

Vite output contract: `events:975625712:9718960`

| Vite production assets | Raw | Gzip-9 | Brotli-11 | Median ms |
| --- | ---: | ---: | ---: | ---: |
| assets/index-9bl6fb9B.js<br>index.html | 697 | 477 | 388 | 6.36 |

## Binary telemetry

Expected output: `binary:446359193:32`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 1208 | 489 | 441 | +48.0% | 10.57 | -0.2% |
| Reference JS esbuild | 545 | 327 | 299 | +0.3% | 10.57 | -0.2% |
| JS Closure ADVANCED | 521 | 319 | 298 | 0.0% | 10.59 | 0.0% |
| JS hand-specialized | 409 | 273 | 257 | -13.8% | 9.72 | -8.3% |
| LilScript | 604 | 370 | 333 | +11.7% | 10.60 | +0.1% |

## Module pricing

Expected output: `module:init modules:593759979:4940`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 884 | 395 | 332 | +57.3% | 1.16 | +5.7% |
| Reference JS esbuild | 361 | 258 | 214 | +1.4% | 1.15 | +4.9% |
| JS Closure ADVANCED | 335 | 257 | 211 | 0.0% | 1.10 | 0.0% |
| JS hand-specialized | 123 | 122 | 112 | -46.9% | 0.55 | -49.6% |
| LilScript | 277 | 226 | 191 | -9.5% | 1.12 | +1.7% |

## Animation value kernel

Expected output: `motion:14400000:28719240:880000`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 698 | 360 | 305 | +113.3% | 0.86 | +21.8% |
| Reference JS esbuild | 270 | 205 | 174 | +21.7% | 0.87 | +22.6% |
| JS Closure ADVANCED | 144 | 146 | 143 | 0.0% | 0.71 | 0.0% |
| JS hand-specialized | 119 | 126 | 104 | -27.3% | 0.58 | -17.4% |
| LilScript | 164 | 155 | 133 | -7.0% | 0.70 | -0.2% |
| LilScript specialized source (diagnostic) | 133 | 137 | 106 | -25.9% | 0.56 | -20.5% |

Context-only production build: **Motion value and spring APIs via Vite**. This uses a different library implementation and is excluded from every compiler delta and total.

Vite output contract: `motion-vite:14400000:28719240:880000:5494928`

| Vite production assets | Raw | Gzip-9 | Brotli-11 | Median ms |
| --- | ---: | ---: | ---: | ---: |
| assets/index-CZj057qH.js<br>index.html | 10571 | 4546 | 4152 | 2.26 |

## Corpus totals

Runtime is a geometric mean of per-workload ratios to Closure; size columns are sums.

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Runtime ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 4986 | 2101 | 1777 | +61.5% | 1.087x |
| Reference JS esbuild | 2226 | 1380 | 1196 | +8.7% | 1.056x |
| JS Closure ADVANCED | 1840 | 1268 | 1100 | 0.0% | 1.000x |
| JS hand-specialized | 1008 | 840 | 745 | -32.3% | 0.686x |
| LilScript | 1879 | 1283 | 1124 | +2.2% | 1.039x |

## Interpretation limits

- Hand-specialized JavaScript is an oracle for expert whole-program rewriting, not source given to Closure or LilScript.
- Real package builds are Vite context measurements only. No package row is compared with a specialized rewrite or included in corpus totals.
- LilScript does not currently implement Motion. A complete claim requires the public package surface and upstream behavioral tests, including DOM, timing, cancellation, gestures, scrolling, SVG, and React entry points.
- Generated C and native executables are behavior gates; only JavaScript artifacts are included in transfer-size and Node runtime tables.
- Closure receives the exact readable JavaScript reference used by the unminified and esbuild rows.
- Matching one deterministic stdout contract can have false negatives; it is regression evidence, not a proof of general equivalence.
- Runtime is repeated cache-busted module parsing plus execution inside one dedicated Node process per artifact. It excludes process startup but is not a browser-frame benchmark.
- These results apply to this corpus and compiler revision; they do not prove universal superiority over Closure.
