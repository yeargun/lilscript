# Application benchmark results

Generated on 2026-08-06T07:24:30.956Z with LilScript `6105991`, Node `v20.12.0`, esbuild `0.28.1`, and Google Closure Compiler `20260803.0.0` on `darwin 24.6.0 arm64`.

Every JavaScript artifact and LilScript native executable passed the same checked-in stdout contract. Negative deltas are smaller or faster than Closure ADVANCED.

Ecosystem JavaScript lanes use Alien Signals `3.2.1` and mitt `3.0.1`.

## Source size

Source bytes describe only checked-in app code and exclude npm dependencies. They measure authoring surface, not shipping size.

| Workload | JS app source | Closure-friendly source | LilScript app source | Hand-specialized JS |
| --- | ---: | ---: | ---: | ---: |
| reactive-store | 633 | 1122 | 1258 | 246 |
| event-pipeline | 411 | 914 | 1010 | 113 |
| binary-telemetry | 1157 | 1131 | 1070 | 410 |
| module-pricing | 1033 | 766 | 916 | 124 |

## reactive-store

Expected output: `reactive:1890621774:408`

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| JS raw bundle | 12805 | 2761 | 2485 | +894.0% | 56.23 | +64.1% |
| JS esbuild | 4708 | 1832 | 1710 | +584.0% | 55.83 | +62.9% |
| JS Closure ADVANCED | 489 | 302 | 250 | 0.0% | 34.26 | 0.0% |
| JS hand-specialized | 245 | 194 | 170 | -32.0% | 33.12 | -3.3% |
| LilScript | 458 | 292 | 259 | +3.6% | 34.42 | +0.4% |

## event-pipeline

Expected output: `events:975625712:9718960`

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| JS raw bundle | 989 | 471 | 417 | +110.6% | 38.66 | +18.3% |
| JS esbuild | 497 | 306 | 278 | +40.4% | 39.05 | +19.5% |
| JS Closure ADVANCED | 351 | 244 | 198 | 0.0% | 32.68 | 0.0% |
| JS hand-specialized | 112 | 125 | 102 | -48.5% | 30.60 | -6.4% |
| LilScript | 322 | 224 | 188 | -5.1% | 32.34 | -1.1% |

## binary-telemetry

Expected output: `binary:446359193:32`

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| JS raw bundle | 1208 | 489 | 441 | +50.0% | 42.72 | +0.5% |
| JS esbuild | 545 | 327 | 299 | +1.7% | 43.15 | +1.5% |
| JS Closure ADVANCED | 517 | 316 | 294 | 0.0% | 42.51 | 0.0% |
| JS hand-specialized | 409 | 273 | 257 | -12.6% | 41.29 | -2.9% |
| LilScript | 568 | 331 | 312 | +6.1% | 42.42 | -0.2% |

## module-pricing

Expected output: `module:init modules:593759979:4940`

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Median ms | vs Closure time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| JS raw bundle | 884 | 395 | 332 | +58.1% | 31.50 | -0.5% |
| JS esbuild | 361 | 258 | 214 | +1.9% | 31.34 | -1.0% |
| JS Closure ADVANCED | 335 | 256 | 210 | 0.0% | 31.67 | 0.0% |
| JS hand-specialized | 123 | 122 | 112 | -46.7% | 30.75 | -2.9% |
| LilScript | 272 | 224 | 189 | -10.0% | 31.69 | +0.1% |

## Corpus totals

Runtime is a geometric mean of per-workload ratios to Closure; size columns are sums.

| Artifact | Raw | Gzip-9 | Brotli-11 | vs Closure Brotli | Runtime ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| JS raw bundle | 15886 | 4116 | 3675 | +286.0% | 1.180x |
| JS esbuild | 6111 | 2723 | 2501 | +162.7% | 1.183x |
| JS Closure ADVANCED | 1692 | 1118 | 952 | 0.0% | 1.000x |
| JS hand-specialized | 889 | 714 | 641 | -32.7% | 0.961x |
| LilScript | 1620 | 1071 | 948 | -0.4% | 0.998x |

## Interpretation limits

- `reactive-store` and `event-pipeline` compare complete app behavior, not complete library APIs.
- Generated C and native executables are behavior gates; only JavaScript artifacts are included in transfer-size and Node runtime tables.
- Closure receives a readable app-specific implementation, bundled without minification before `ADVANCED` compilation.
- Fresh-process runtime includes Node startup and is intended to catch large regressions, not establish engine-level causality.
- These results apply to this corpus and compiler revision; they do not prove universal superiority over Closure.
