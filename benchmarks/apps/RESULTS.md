# Application benchmark results

Generated on 2026-08-06T13:20:05.682Z with LilScript `c04c4c2`, Node `v24.11.1`, Vite `8.2.0`, esbuild `0.28.1`, and Google Closure Compiler `20260803.0.0` on `darwin 24.6.0 arm64`.

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
| Reference JS bundle | 1163 | 464 | 379 | +51.6% | 3.28 | +6.4% |
| Reference JS esbuild | 544 | 310 | 265 | +6.0% | 3.11 | +0.8% |
| JS Closure ADVANCED | 489 | 302 | 250 | 0.0% | 3.09 | 0.0% |
| JS hand-specialized | 245 | 194 | 170 | -32.0% | 2.90 | -6.0% |
| LilScript | 519 | 305 | 275 | +10.0% | 3.28 | +6.1% |

Context-only production build: **Alien Signals via Vite**. This uses a different library implementation and is excluded from every compiler delta and total.

Vite output contract: `reactive:1890621774:408`

| Vite production assets | Raw | Gzip-9 | Brotli-11 | Median ms |
| --- | ---: | ---: | ---: | ---: |
| assets/index-BAb3oBAp.js<br>index.html | 4900 | 1932 | 1752 | 22.19 |

## Event pipeline

Expected output: `events:975625712:9718960`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 1033 | 393 | 320 | +61.6% | 1.59 | +5.7% |
| Reference JS esbuild | 506 | 280 | 244 | +23.2% | 1.54 | +2.3% |
| JS Closure ADVANCED | 351 | 244 | 198 | 0.0% | 1.51 | 0.0% |
| JS hand-specialized | 112 | 125 | 102 | -48.5% | 0.62 | -59.0% |
| LilScript | 324 | 236 | 196 | -1.0% | 1.66 | +10.1% |

Context-only production build: **mitt via Vite**. This uses a different library implementation and is excluded from every compiler delta and total.

Vite output contract: `events:975625712:9718960`

| Vite production assets | Raw | Gzip-9 | Brotli-11 | Median ms |
| --- | ---: | ---: | ---: | ---: |
| assets/index-9bl6fb9B.js<br>index.html | 697 | 477 | 388 | 6.35 |

## Binary telemetry

Expected output: `binary:446359193:32`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 1208 | 489 | 441 | +48.0% | 10.36 | -1.3% |
| Reference JS esbuild | 545 | 327 | 299 | +0.3% | 10.39 | -1.1% |
| JS Closure ADVANCED | 521 | 319 | 298 | 0.0% | 10.50 | 0.0% |
| JS hand-specialized | 409 | 273 | 257 | -13.8% | 9.40 | -10.5% |
| LilScript | 604 | 370 | 333 | +11.7% | 10.40 | -0.9% |

## Module pricing

Expected output: `module:init modules:593759979:4940`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 884 | 395 | 332 | +57.3% | 1.15 | +4.6% |
| Reference JS esbuild | 361 | 258 | 214 | +1.4% | 1.14 | +3.8% |
| JS Closure ADVANCED | 335 | 257 | 211 | 0.0% | 1.10 | 0.0% |
| JS hand-specialized | 123 | 122 | 112 | -46.9% | 0.55 | -50.0% |
| LilScript | 277 | 226 | 194 | -8.1% | 1.11 | +0.4% |

## Animation value kernel

Expected output: `motion:14400000:28719240:880000`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 698 | 360 | 305 | +113.3% | 0.87 | +22.6% |
| Reference JS esbuild | 270 | 205 | 174 | +21.7% | 0.85 | +20.2% |
| JS Closure ADVANCED | 144 | 146 | 143 | 0.0% | 0.71 | 0.0% |
| JS hand-specialized | 119 | 126 | 104 | -27.3% | 0.59 | -17.1% |
| LilScript | 164 | 155 | 133 | -7.0% | 0.68 | -3.8% |
| LilScript specialized source (diagnostic) | 133 | 137 | 106 | -25.9% | 0.56 | -20.9% |

Context-only production build: **Motion value and spring APIs via Vite**. This uses a different library implementation and is excluded from every compiler delta and total.

Vite output contract: `motion-vite:14400000:28719240:880000:5494928`

| Vite production assets | Raw | Gzip-9 | Brotli-11 | Median ms |
| --- | ---: | ---: | ---: | ---: |
| assets/index-CZj057qH.js<br>index.html | 10571 | 4546 | 4152 | 2.22 |

## Corpus totals

Runtime is a geometric mean of per-workload ratios to Closure; size columns are sums.

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Runtime ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 4986 | 2101 | 1777 | +61.5% | 1.073x |
| Reference JS esbuild | 2226 | 1380 | 1196 | +8.7% | 1.049x |
| JS Closure ADVANCED | 1840 | 1268 | 1100 | 0.0% | 1.000x |
| JS hand-specialized | 1008 | 840 | 745 | -32.3% | 0.678x |
| LilScript | 1888 | 1292 | 1131 | +2.8% | 1.023x |

## Interpretation limits

- Hand-specialized JavaScript is an oracle for expert whole-program rewriting, not source given to Closure or LilScript.
- Real package builds are Vite context measurements only. No package row is compared with a specialized rewrite or included in corpus totals.
- LilScript does not currently implement Motion. A complete claim requires the public package surface and upstream behavioral tests, including DOM, timing, cancellation, gestures, scrolling, SVG, and React entry points.
- Generated C and native executables are behavior gates; only JavaScript artifacts are included in transfer-size and Node runtime tables.
- Closure receives the exact readable JavaScript reference used by the unminified and esbuild rows.
- Matching one deterministic stdout contract can have false negatives; it is regression evidence, not a proof of general equivalence.
- Runtime is repeated cache-busted module parsing plus execution inside one dedicated Node process per artifact. It excludes process startup but is not a browser-frame benchmark.
- These results apply to this corpus and compiler revision; they do not prove universal superiority over Closure.
