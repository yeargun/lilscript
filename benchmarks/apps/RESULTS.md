# Application benchmark results

Generated on 2026-08-06T08:42:22.056Z with LilScript `f3bed41`, Node `v20.12.0`, esbuild `0.28.1`, and Google Closure Compiler `20260803.0.0` on `darwin 24.6.0 arm64`.

Every JavaScript artifact and LilScript native executable passed the same checked-in stdout contract. Negative deltas are smaller or faster than Closure ADVANCED.

Ecosystem JavaScript lanes use Alien Signals `3.2.1`, mitt `3.0.1`, and Motion `13.0.0`.

## Source size

Source bytes describe only checked-in app code and exclude npm dependencies. They measure authoring surface, not shipping size.

| Workload | JS app source | Closure-friendly source | LilScript app source | Hand-specialized JS |
| --- | ---: | ---: | ---: | ---: |
| reactive-store | 633 | 1122 | 1258 | 246 |
| event-pipeline | 411 | 914 | 1010 | 113 |
| binary-telemetry | 1157 | 1131 | 1070 | 410 |
| module-pricing | 1033 | 766 | 916 | 124 |
| motion-values | 397 | 613 | 661 | 120 |

## reactive-store

Expected output: `reactive:1890621774:408`

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| JS raw bundle | 12805 | 2761 | 2485 | +894.0% | 55.82 | +65.8% |
| JS esbuild | 4708 | 1832 | 1710 | +584.0% | 54.99 | +63.3% |
| JS Closure ADVANCED | 489 | 302 | 250 | 0.0% | 33.67 | 0.0% |
| JS hand-specialized | 245 | 194 | 170 | -32.0% | 32.24 | -4.3% |
| LilScript | 513 | 295 | 262 | +4.8% | 33.90 | +0.7% |

## event-pipeline

Expected output: `events:975625712:9718960`

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| JS raw bundle | 989 | 471 | 417 | +110.6% | 38.12 | +19.2% |
| JS esbuild | 497 | 306 | 278 | +40.4% | 38.46 | +20.2% |
| JS Closure ADVANCED | 351 | 244 | 198 | 0.0% | 31.99 | 0.0% |
| JS hand-specialized | 112 | 125 | 102 | -48.5% | 29.86 | -6.6% |
| LilScript | 322 | 224 | 188 | -5.1% | 31.71 | -0.9% |

## binary-telemetry

Expected output: `binary:446359193:32`

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| JS raw bundle | 1208 | 489 | 441 | +50.0% | 41.83 | -0.3% |
| JS esbuild | 545 | 327 | 299 | +1.7% | 41.72 | -0.5% |
| JS Closure ADVANCED | 517 | 316 | 294 | 0.0% | 41.95 | 0.0% |
| JS hand-specialized | 409 | 273 | 257 | -12.6% | 40.37 | -3.8% |
| LilScript | 568 | 331 | 312 | +6.1% | 41.80 | -0.3% |

## module-pricing

Expected output: `module:init modules:593759979:4940`

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| JS raw bundle | 884 | 395 | 332 | +58.1% | 31.07 | +0.1% |
| JS esbuild | 361 | 258 | 214 | +1.9% | 31.08 | +0.2% |
| JS Closure ADVANCED | 335 | 256 | 210 | 0.0% | 31.02 | 0.0% |
| JS hand-specialized | 123 | 122 | 112 | -46.7% | 30.19 | -2.7% |
| LilScript | 272 | 224 | 189 | -10.0% | 31.22 | +0.6% |

## motion-values

Expected output: `motion:14400000:28719240:880000`

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| JS raw bundle | 19570 | 5390 | 4824 | +3273.4% | 33.13 | +6.3% |
| JS esbuild | 7321 | 3243 | 2958 | +1968.5% | 32.82 | +5.3% |
| JS Closure ADVANCED | 144 | 146 | 143 | 0.0% | 31.17 | 0.0% |
| JS hand-specialized | 119 | 126 | 104 | -27.3% | 30.44 | -2.3% |
| LilScript | 179 | 167 | 141 | -1.4% | 31.21 | +0.2% |

## Corpus totals

Runtime is a geometric mean of per-workload ratios to Closure; size columns are sums.

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Runtime ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| JS raw bundle | 35456 | 9506 | 8499 | +676.2% | 1.160x |
| JS esbuild | 13432 | 5966 | 5459 | +398.5% | 1.156x |
| JS Closure ADVANCED | 1836 | 1264 | 1095 | 0.0% | 1.000x |
| JS hand-specialized | 1008 | 840 | 745 | -32.0% | 0.961x |
| LilScript | 1854 | 1241 | 1092 | -0.3% | 1.000x |

## Interpretation limits

- `reactive-store`, `event-pipeline`, and `motion-values` compare complete app behavior, not complete library APIs.
- `motion-values` exercises Motion's real `mix`, `wrap`, and `stagger` exports; it does not claim LilScript implements Motion's DOM animation engine.
- Generated C and native executables are behavior gates; only JavaScript artifacts are included in transfer-size and Node runtime tables.
- Closure receives a readable app-specific implementation, bundled without minification before `ADVANCED` compilation.
- Fresh-process runtime includes Node startup and is intended to catch large regressions, not establish engine-level causality.
- These results apply to this corpus and compiler revision; they do not prove universal superiority over Closure.
