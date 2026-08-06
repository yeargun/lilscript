# Application benchmark results

Generated on 2026-08-06T23:35:06.347Z with LilScript `b069356`, Node `v24.11.1`, Vite `8.2.0`, esbuild `0.28.1`, and Google Closure Compiler `20260803.0.0` on `darwin 24.6.0 arm64`.

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
| Animation value kernel | 613 | 661 | 120 |

## Reactive store

Expected output: `reactive:1890621774:408`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 1151 | 458 | 375 | +55.0% | 3.48 | +7.9% |
| Reference JS esbuild | 523 | 302 | 259 | +7.0% | 3.31 | +2.7% |
| JS Closure ADVANCED | 468 | 295 | 242 | 0.0% | 3.23 | 0.0% |
| JS hand-specialized | 245 | 194 | 170 | -29.8% | 2.95 | -8.7% |
| LilScript | 419 | 281 | 252 | +4.1% | 3.33 | +3.2% |

Context-only production build: **Alien Signals via Vite**. This uses a different library implementation and is excluded from every compiler delta and total.

Vite output contract: `reactive:1890621774:408`

| Vite production assets | Raw | Gzip-9 | Brotli-11 | Median ms |
| --- | ---: | ---: | ---: | ---: |
| assets/index-DVPqLfDU.js<br>index.html | 4884 | 1924 | 1737 | 22.46 |

## Event pipeline

Expected output: `events:975625712:9718960`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 1029 | 389 | 317 | +65.1% | 1.72 | +3.8% |
| Reference JS esbuild | 499 | 275 | 239 | +24.5% | 1.70 | +2.9% |
| JS Closure ADVANCED | 344 | 238 | 192 | 0.0% | 1.66 | 0.0% |
| JS hand-specialized | 112 | 125 | 102 | -46.9% | 0.63 | -61.7% |
| LilScript | 312 | 224 | 189 | -1.6% | 1.63 | -1.8% |

Context-only production build: **mitt via Vite**. This uses a different library implementation and is excluded from every compiler delta and total.

Vite output contract: `events:975625712:9718960`

| Vite production assets | Raw | Gzip-9 | Brotli-11 | Median ms |
| --- | ---: | ---: | ---: | ---: |
| assets/index-C8ji9Tfp.js<br>index.html | 690 | 471 | 382 | 6.33 |

## Binary telemetry

Expected output: `binary:446359193:32`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 1196 | 481 | 438 | +51.0% | 10.26 | -1.3% |
| Reference JS esbuild | 521 | 320 | 291 | +0.3% | 10.23 | -1.6% |
| JS Closure ADVANCED | 497 | 313 | 290 | 0.0% | 10.40 | 0.0% |
| JS hand-specialized | 409 | 273 | 257 | -11.4% | 9.72 | -6.5% |
| LilScript | 534 | 334 | 297 | +2.4% | 10.18 | -2.1% |

## Module pricing

Expected output: `module:init modules:593759979:4940`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 878 | 388 | 327 | +58.7% | 0.98 | +6.2% |
| Reference JS esbuild | 352 | 251 | 208 | +1.0% | 0.96 | +3.2% |
| JS Closure ADVANCED | 328 | 250 | 206 | 0.0% | 0.93 | 0.0% |
| JS hand-specialized | 123 | 122 | 112 | -45.6% | 0.55 | -40.7% |
| LilScript | 242 | 199 | 167 | -18.9% | 0.95 | +2.4% |

## Animation value kernel

Expected output: `motion:14400000:28719240:880000`

Comparable compiler artifacts:

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 698 | 360 | 305 | +113.3% | 0.86 | +17.3% |
| Reference JS esbuild | 270 | 205 | 174 | +21.7% | 0.88 | +20.4% |
| JS Closure ADVANCED | 144 | 146 | 143 | 0.0% | 0.73 | 0.0% |
| JS hand-specialized | 119 | 126 | 104 | -27.3% | 0.60 | -18.5% |
| LilScript | 149 | 147 | 120 | -16.1% | 0.70 | -4.8% |
| LilScript specialized source (diagnostic) | 123 | 131 | 98 | -31.5% | 0.55 | -24.4% |

Context-only production build: **Motion value and spring APIs via Vite**. This uses a different library implementation and is excluded from every compiler delta and total.

Vite output contract: `motion-vite:14400000:28719240:880000:5494928`

| Vite production assets | Raw | Gzip-9 | Brotli-11 | Median ms |
| --- | ---: | ---: | ---: | ---: |
| assets/index-CZj057qH.js<br>index.html | 10571 | 4546 | 4152 | 2.27 |

## Corpus totals

Runtime is a geometric mean of per-workload ratios to Closure; size columns are sums.

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Runtime ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| Reference JS bundle | 4952 | 2076 | 1762 | +64.2% | 1.066x |
| Reference JS esbuild | 2165 | 1353 | 1171 | +9.1% | 1.053x |
| JS Closure ADVANCED | 1781 | 1242 | 1073 | 0.0% | 1.000x |
| JS hand-specialized | 1008 | 840 | 745 | -30.6% | 0.691x |
| LilScript | 1656 | 1185 | 1025 | -4.5% | 0.993x |

## Interpretation limits

- Hand-specialized JavaScript is an oracle for expert whole-program rewriting, not source given to Closure or LilScript.
- Real package builds are Vite context measurements only. No package row is compared with a specialized rewrite or included in corpus totals.
- LilScript does not currently implement Motion. A complete claim requires the public package surface and upstream behavioral tests, including DOM, timing, cancellation, gestures, scrolling, SVG, and React entry points.
- Generated C and native executables are behavior gates; only JavaScript artifacts are included in transfer-size and Node runtime tables.
- Closure receives the exact readable JavaScript reference used by the unminified and esbuild rows.
- Matching one deterministic stdout contract can have false negatives; it is regression evidence, not a proof of general equivalence.
- Runtime is repeated cache-busted module parsing plus execution inside one dedicated Node process per artifact. It excludes process startup but is not a browser-frame benchmark.
- These results apply to this corpus and compiler revision; they do not prove universal superiority over Closure.
