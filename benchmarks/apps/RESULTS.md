# Application benchmark results

Generated on 2026-08-09T09:31:48.356Z with LilScript `85ef5bc`, Node `v24.11.1`, Vite `8.2.1`, esbuild `0.28.1`, and Google Closure Compiler `20260804.0.0` on `darwin 24.6.0 arm64`.

This report contains two deliberately separate datasets. Compiler rows use a readable JavaScript reference and a LilScript implementation with the same app algorithm and abstraction scope. Ecosystem rows build real npm packages with Vite and are never included in compiler totals.

Every emitted artifact passed its checked-in stdout contract. That rejects observed behavior mismatches for these inputs; it does not prove complete semantic or library API equivalence.

Context-only ecosystem builds use Alien Signals `3.2.1`, mitt `3.0.1`, and Motion `13.0.0`.

## Source size

Source bytes describe only checked-in app code and exclude npm dependencies. They measure authoring surface, not shipping size.

| Workload | Reference JS | LilScript | Hand-specialized JS |
| --- | ---: | ---: | ---: |
| Reactive store | 1106 | 1258 | 246 |
| Event pipeline | 910 | 1010 | 113 |
| Binary telemetry | 1145 | 1070 | 410 |
| Module pricing | 1027 | 916 | 124 |
| Animation value kernel | 2067 | 2539 | 128 |

## Reactive store

Expected output: `reactive:1890621774:408`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 1151 | 458 | 375 | +55.0% | 3.62 | +10.1% |
| Reference JS esbuild | 523 | 302 | 259 | +7.0% | 3.22 | -1.9% |
| JS Closure ADVANCED | 468 | 295 | 242 | 0.0% | 3.28 | 0.0% |
| JS hand-specialized | 245 | 194 | 170 | -29.8% | 2.95 | -10.1% |
| LilScript | 386 | 258 | 216 | -10.7% | 3.11 | -5.3% |

Context-only production build: **Alien Signals via Vite**. This uses a different library implementation and is excluded from every compiler delta and total.

Vite output contract: `reactive:1890621774:408`

| Vite production assets | Raw | Gzip-9 | Brotli-11 | Median ms |
| --- | ---: | ---: | ---: | ---: |
| assets/index-DVPqLfDU.js<br>index.html | 4884 | 1924 | 1737 | 22.85 |

## Event pipeline

Expected output: `events:975625712:9718960`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 1029 | 389 | 317 | +65.1% | 1.94 | +16.6% |
| Reference JS esbuild | 499 | 275 | 239 | +24.5% | 1.68 | +0.9% |
| JS Closure ADVANCED | 344 | 238 | 192 | 0.0% | 1.66 | 0.0% |
| JS hand-specialized | 112 | 125 | 102 | -46.9% | 0.73 | -55.9% |
| LilScript | 292 | 210 | 169 | -12.0% | 1.58 | -4.8% |

Context-only production build: **mitt via Vite**. This uses a different library implementation and is excluded from every compiler delta and total.

Vite output contract: `events:975625712:9718960`

| Vite production assets | Raw | Gzip-9 | Brotli-11 | Median ms |
| --- | ---: | ---: | ---: | ---: |
| assets/index-C8ji9Tfp.js<br>index.html | 690 | 471 | 382 | 7.43 |

## Binary telemetry

Expected output: `binary:446359193:32`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 1196 | 481 | 438 | +51.0% | 11.35 | +3.7% |
| Reference JS esbuild | 521 | 320 | 291 | +0.3% | 10.95 | +0.1% |
| JS Closure ADVANCED | 497 | 313 | 290 | 0.0% | 10.94 | 0.0% |
| JS hand-specialized | 409 | 273 | 257 | -11.4% | 10.05 | -8.1% |
| LilScript | 427 | 304 | 266 | -8.3% | 10.05 | -8.1% |

## Module pricing

Expected output: `module:init modules:593759979:4940`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 878 | 388 | 327 | +58.7% | 1.38 | +19.3% |
| Reference JS esbuild | 352 | 251 | 208 | +1.0% | 1.20 | +3.6% |
| JS Closure ADVANCED | 328 | 250 | 206 | 0.0% | 1.16 | 0.0% |
| JS hand-specialized | 123 | 122 | 112 | -45.6% | 0.65 | -43.6% |
| LilScript | 227 | 188 | 157 | -23.8% | 1.04 | -9.9% |

## Animation value kernel

Expected output: `motion:14400000:28719240:880000:5494928`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 2149 | 792 | 713 | +91.2% | 1.33 | +26.6% |
| Reference JS esbuild | 688 | 436 | 395 | +5.9% | 1.20 | +13.5% |
| JS Closure ADVANCED | 594 | 416 | 373 | 0.0% | 1.05 | 0.0% |
| JS hand-specialized | 127 | 132 | 111 | -70.2% | 0.82 | -22.1% |
| LilScript | 462 | 323 | 280 | -24.9% | 1.07 | +1.7% |
| LilScript specialized source (diagnostic) | 124 | 133 | 101 | -72.9% | 0.79 | -24.6% |

Context-only production build: **Motion value and spring APIs via Vite**. This uses a different library implementation and is excluded from every compiler delta and total.

Vite output contract: `motion-vite:14400000:28719240:880000:5494928`

| Vite production assets | Raw | Gzip-9 | Brotli-11 | Median ms |
| --- | ---: | ---: | ---: | ---: |
| assets/index-CZj057qH.js<br>index.html | 10571 | 4546 | 4152 | 2.81 |

## Corpus totals

Runtime is a geometric mean of per-workload ratios to Closure; size columns are sums.

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Runtime ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 6403 | 2508 | 2170 | +66.5% | 1.150x |
| Reference JS esbuild | 2583 | 1584 | 1392 | +6.8% | 1.031x |
| JS Closure ADVANCED | 2231 | 1512 | 1303 | 0.0% | 1.000x |
| JS hand-specialized | 1016 | 846 | 752 | -42.3% | 0.693x |
| LilScript | 1794 | 1283 | 1088 | -16.5% | 0.946x |

## Interpretation limits

- Hand-specialized JavaScript is an oracle for expert whole-program rewriting, not source given to Closure or LilScript.
- Real package builds are Vite context measurements only. No package row is compared with a specialized rewrite or included in corpus totals.
- The Motion compiler workload matches the selected numeric mix/wrap/stagger and underdamped-spring equations and digest; LilScript still does not implement Motion's package API. A complete claim requires the public package surface and upstream behavioral tests, including DOM, timing, cancellation, gestures, scrolling, SVG, and React entry points.
- Generated C and native executables are behavior gates; only JavaScript artifacts are included in transfer-size and Node runtime tables.
- Closure receives the exact readable JavaScript reference used by the unminified and esbuild rows.
- Matching one deterministic stdout contract can have false negatives; it is regression evidence, not a proof of general equivalence.
- The checked methodology gate requires every LilScript workload to be no larger than Closure in raw, gzip-9, and Brotli-11 bytes; full 20+-sample runs also require median runtime within 5% of Closure.
- Runtime is repeated cache-busted module parsing plus execution inside one dedicated Node process per artifact. It excludes process startup but is not a browser-frame benchmark.
- These results apply to this corpus and compiler revision; they do not prove universal superiority over Closure.
