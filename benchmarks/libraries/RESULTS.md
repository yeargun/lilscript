# Complete library compatibility diagnostics

Generated 2026-09-27T14:44:22.219Z from LilScript `24968659` with Node `v24.11.1`, Vite `8.2.1`, esbuild `0.28.1`, and Closure Compiler `20260804.0.0`.

Each row executes the same checked app contract, but size eligibility is measured on the reusable selected root API so whole-program constant specialization cannot remove the library implementation. The npm rows use the installed package, not a hand-specialized substitute. Closure receives an unminified esbuild bundle that exposes the same named public surface. LilScript's raw, gzip-9, and Brotli-11 cells come from independent objective builds, and each build is judged only on its matching metric. LilScript also emits C and a native executable, and both must match before measurements are considered.

Publication gate: the raw-objective artifact's raw bytes and the brotli-objective artifact's matching compressed bytes must be no larger than both npm/Vite and public-contract-preserving Closure ADVANCED; median library-workload time and retained memory must each be at most 1.05× npm. Eligible: **3/7**. Blocked rows remain below strictly as compiler diagnostics.

## Motion easing

Status: **blocked** — raw 498 B exceeds closure 445 B; brotli 305 B exceeds closure 286 B.

Scope: **Complete @motionone/easing root entrypoint** using `@motionone/easing@10.18.0`.

Contract: `motion-easing:27:560673:541722`

Translated upstream assertions: **27**. Added package-contract assertions: **0**. Monthly downloads at selection time: **9,574,633**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 840 | 456 | 431 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 445 | 325 | 286 | -33.6% |
| LilScript reusable objective builds | 498 | 326 | 305 | -29.2% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 34.185 | 34.162 | 0.999 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 3830280 | 3215784 | 0.840 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1107 | 606 | 579 | 11.46 |
| Installed npm package + Closure ADVANCED | 1024 | 577 | 530 | 9.23 |
| LilScript port | 1088 | 605 | 564 | 11.60 |

## Clamp and lerp

Status: **eligible**.

Scope: **Complete clamp and lerp root entrypoints** using `clamp@1.0.1` and `lerp@1.0.3`.

Contract: `micro-math:10:1800000:86076`

Translated upstream assertions: **10**. Added package-contract assertions: **0**. Monthly downloads at selection time: **4,609,993**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 1137 | 577 | 519 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 833 | 482 | 414 | -20.2% |
| LilScript reusable objective builds | 138 | 166 | 141 | -72.8% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 18.960 | 19.267 | 1.016 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 318448 | 318416 | 1.000 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1134 | 608 | 554 | 4.55 |
| Installed npm package + Closure ADVANCED | 1169 | 625 | 562 | 4.53 |
| LilScript port | 480 | 249 | 232 | 2.07 |

## String hash

Status: **eligible**.

Scope: **Complete string-hash root entrypoint** using `string-hash@1.1.3`.

Contract: `string-hash:4:1670934855`

Translated upstream assertions: **2**. Added package-contract assertions: **2**. Monthly downloads at selection time: **19,930,081**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 969 | 559 | 502 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 744 | 473 | 403 | -19.7% |
| LilScript reusable objective builds | 154 | 154 | 129 | -74.3% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 29.613 | 30.744 | 1.038 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 317736 | 318376 | 1.002 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1076 | 638 | 568 | 6.77 |
| Installed npm package + Closure ADVANCED | 1113 | 651 | 581 | 7.11 |
| LilScript port | 496 | 346 | 309 | 5.68 |

## Levenshtein distance

Status: **blocked** — throughput ratio 1.110 exceeds 1.05.

Scope: **Complete js-levenshtein root entrypoint** using `js-levenshtein@1.1.6`.

Contract: `js-levenshtein:14:2049950`

Translated upstream assertions: **14**. Added package-contract assertions: **0**. Monthly downloads at selection time: **41,652,021**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 1984 | 914 | 825 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 1453 | 788 | 714 | -13.5% |
| LilScript reusable objective builds | 941 | 456 | 413 | -49.9% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 59.164 | 65.679 | 1.110 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 315400 | 318280 | 1.009 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1970 | 1067 | 969 | 13.81 |
| Installed npm package + Closure ADVANCED | 2030 | 1081 | 996 | 12.85 |
| LilScript port | 1550 | 790 | 721 | 15.22 |

## Emotion hash

Status: **blocked** — brotli 249 B exceeds closure 240 B.

Scope: **Complete @emotion/hash root entrypoint** using `@emotion/hash@0.9.2`.

Contract: `emotion-hash:8:30831534`

Translated upstream assertions: **1**. Added package-contract assertions: **7**. Monthly downloads at selection time: **122,398,176**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 833 | 348 | 330 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 594 | 287 | 240 | -27.3% |
| LilScript reusable objective builds | 509 | 281 | 249 | -24.5% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 46.283 | 46.171 | 0.998 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 637488 | 642120 | 1.007 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 907 | 492 | 430 | 14.27 |
| Installed npm package + Closure ADVANCED | 924 | 487 | 434 | 14.14 |
| LilScript port | 873 | 506 | 449 | 15.81 |

## MurmurHash 2 and 3

Status: **eligible**.

Scope: **Complete murmurhash-js root entrypoint** using `murmurhash-js@1.0.0`.

Contract: `murmurhash-js:18:855861453`

Translated upstream assertions: **0**. Added package-contract assertions: **18**. Monthly downloads at selection time: **24,364,654**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 3131 | 1029 | 902 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 2446 | 951 | 833 | -7.6% |
| LilScript reusable objective builds | 1200 | 506 | 457 | -49.3% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 40.247 | 35.810 | 0.890 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 328656 | 318376 | 0.969 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 2782 | 1115 | 992 | 9.85 |
| Installed npm package + Closure ADVANCED | 2905 | 1190 | 1059 | 10.16 |
| LilScript port | 1748 | 761 | 696 | 10.52 |

## Robust geometric predicates

Status: **blocked** — raw 26191 B exceeds closure 25208 B.

Scope: **Complete robust-predicates root entrypoint** using `robust-predicates@3.0.3`.

Contract: `robust-predicates:8`

Translated upstream assertions: **23798**. Added package-contract assertions: **320016**. Monthly downloads at selection time: **101,525,533**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 38722 | 8329 | 6456 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 25208 | 7872 | 6228 | -3.5% |
| LilScript reusable objective builds | 26191 | 7632 | 6015 | -6.8% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 17.811 | 15.656 | 0.879 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 253552 | 253744 | 1.001 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 24227 | 7418 | 5960 | 1.60 |
| Installed npm package + Closure ADVANCED | 25369 | 7856 | 6230 | 2.45 |
| LilScript port | 25307 | 7478 | 5854 | 2.10 |

## Limits

- Complete means the documented callable root-entrypoint API for the statically typed input domain, not every accidental JavaScript coercion.
- @motionone/easing is a complete published Motion ecosystem package; it is not motion@13 or its DOM engine.
- Runtime measures cache-busted Node module parsing and deterministic app execution. It is not a browser rendering benchmark.
- API throughput and retained memory use medians from 9 isolated Node processes per implementation and mode, with alternating order, identical workloads and checksums, forced GC, and equivalent retained results. The memory lane performs one complete unretained workload before its baseline GC so JIT tier-up is outside the retained delta.
- Reusable-surface transfer sizes sum independently compressed module files. Every LilScript size cell uses the build selected for that exact objective; the other metrics of each build are diagnostic and may lose. Demo-app bytes and runtime come from the explicitly declared Brotli-objective build, remain diagnostics, and cannot hide or establish full-library eligibility.
- A passing translated upstream suite and differential workload are strong regression evidence, not a mathematical proof over every input.

