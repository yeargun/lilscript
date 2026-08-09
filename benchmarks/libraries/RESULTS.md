# Complete library compatibility diagnostics

Generated 2026-08-09T09:38:12.119Z from LilScript `85ef5bc` with Node `v24.11.1`, Vite `8.2.1`, esbuild `0.28.1`, and Closure Compiler `20260804.0.0`.

Each row executes the same checked app contract, but size eligibility is measured on the reusable selected root API so whole-program constant specialization cannot remove the library implementation. The npm rows use the installed package, not a hand-specialized substitute. Closure receives an unminified esbuild bundle that exposes the same named public surface. LilScript also emits C and a native executable, and both must match before measurements are considered.

Publication gate: raw and brotli JavaScript must be no larger than both npm/Vite and public-contract-preserving Closure ADVANCED; median library-workload time and retained memory must each be at most 1.05× npm. Eligible: **7/7**. Blocked rows remain below strictly as compiler diagnostics.

## Motion easing

Status: **eligible**.

Scope: **Complete @motionone/easing root entrypoint** using `@motionone/easing@10.18.0`.

Contract: `motion-easing:27:560673:541722`

Translated upstream assertions: **27**. Added package-contract assertions: **0**. Monthly downloads at selection time: **9,574,633**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 840 | 458 | 431 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 445 | 326 | 286 | -33.6% |
| LilScript reusable module | 438 | 310 | 280 | -35.0% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 14.201 | 14.428 | 1.016 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 3830208 | 3215896 | 0.840 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1107 | 608 | 579 | 6.12 |
| Installed npm package + Closure ADVANCED | 1024 | 583 | 530 | 4.53 |
| LilScript port | 996 | 559 | 507 | 5.45 |

## Clamp and lerp

Status: **eligible**.

Scope: **Complete clamp and lerp root entrypoints** using `clamp@1.0.1` and `lerp@1.0.3`.

Contract: `micro-math:10:1800000:86076`

Translated upstream assertions: **10**. Added package-contract assertions: **0**. Monthly downloads at selection time: **4,609,993**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 1137 | 578 | 519 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 833 | 484 | 414 | -20.2% |
| LilScript reusable module | 109 | 142 | 113 | -78.2% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 8.293 | 8.207 | 0.990 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 318272 | 318256 | 1.000 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1134 | 607 | 554 | 2.22 |
| Installed npm package + Closure ADVANCED | 1169 | 630 | 562 | 2.97 |
| LilScript port | 319 | 224 | 201 | 1.08 |

## String hash

Status: **eligible**.

Scope: **Complete string-hash root entrypoint** using `string-hash@1.1.3`.

Contract: `string-hash:4:1670934855`

Translated upstream assertions: **2**. Added package-contract assertions: **2**. Monthly downloads at selection time: **19,930,081**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 969 | 558 | 502 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 744 | 480 | 403 | -19.7% |
| LilScript reusable module | 121 | 134 | 106 | -78.9% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 15.826 | 16.406 | 1.037 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 317704 | 318344 | 1.002 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1076 | 637 | 568 | 4.03 |
| Installed npm package + Closure ADVANCED | 1113 | 656 | 581 | 3.81 |
| LilScript port | 433 | 345 | 299 | 3.53 |

## Levenshtein distance

Status: **eligible**.

Scope: **Complete js-levenshtein root entrypoint** using `js-levenshtein@1.1.6`.

Contract: `js-levenshtein:14:2049950`

Translated upstream assertions: **14**. Added package-contract assertions: **0**. Monthly downloads at selection time: **41,652,021**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 1984 | 908 | 825 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 1453 | 795 | 714 | -13.5% |
| LilScript reusable module | 924 | 456 | 408 | -50.5% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 34.547 | 34.199 | 0.990 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 310816 | 308776 | 0.993 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1970 | 1077 | 969 | 6.59 |
| Installed npm package + Closure ADVANCED | 2030 | 1099 | 996 | 6.47 |
| LilScript port | 1450 | 814 | 697 | 6.98 |

## Emotion hash

Status: **eligible**.

Scope: **Complete @emotion/hash root entrypoint** using `@emotion/hash@0.9.2`.

Contract: `emotion-hash:8:30831534`

Translated upstream assertions: **1**. Added package-contract assertions: **7**. Monthly downloads at selection time: **122,398,176**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 833 | 347 | 330 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 594 | 288 | 240 | -27.3% |
| LilScript reusable module | 434 | 271 | 234 | -29.1% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 34.111 | 33.102 | 0.970 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 637488 | 638168 | 1.001 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 907 | 498 | 430 | 12.73 |
| Installed npm package + Closure ADVANCED | 924 | 496 | 434 | 12.57 |
| LilScript port | 759 | 502 | 431 | 12.63 |

## MurmurHash 2 and 3

Status: **eligible**.

Scope: **Complete murmurhash-js root entrypoint** using `murmurhash-js@1.0.0`.

Contract: `murmurhash-js:18:855861453`

Translated upstream assertions: **0**. Added package-contract assertions: **18**. Monthly downloads at selection time: **24,364,654**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 3131 | 1027 | 902 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 2446 | 957 | 833 | -7.6% |
| LilScript reusable module | 1015 | 461 | 414 | -54.1% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 21.092 | 18.572 | 0.881 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 317640 | 291648 | 0.918 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 2782 | 1118 | 992 | 5.60 |
| Installed npm package + Closure ADVANCED | 2905 | 1203 | 1059 | 5.46 |
| LilScript port | 1520 | 719 | 641 | 5.04 |

## Robust geometric predicates

Status: **eligible**.

Scope: **Complete robust-predicates root entrypoint** using `robust-predicates@3.0.3`.

Contract: `robust-predicates:8`

Translated upstream assertions: **23798**. Added package-contract assertions: **320016**. Monthly downloads at selection time: **101,525,533**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 38722 | 8365 | 6456 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 25208 | 7933 | 6228 | -3.5% |
| LilScript reusable module | 22347 | 8045 | 6192 | -4.1% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 7.988 | 7.936 | 0.993 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 253552 | 253648 | 1.000 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 24227 | 7463 | 5960 | 0.94 |
| Installed npm package + Closure ADVANCED | 25369 | 7920 | 6230 | 1.17 |
| LilScript port | 1213 | 495 | 411 | 0.24 |

## Limits

- Complete means the documented callable root-entrypoint API for the statically typed input domain, not every accidental JavaScript coercion.
- @motionone/easing is a complete published Motion ecosystem package; it is not motion@13 or its DOM engine.
- Runtime measures cache-busted Node module parsing and deterministic app execution. It is not a browser rendering benchmark.
- API throughput and retained memory use medians from 9 isolated Node processes per implementation and mode, with alternating order, identical workloads and checksums, forced GC, and equivalent retained results. The memory lane performs one complete unretained workload before its baseline GC so JIT tier-up is outside the retained delta.
- Reusable-surface transfer sizes sum independently compressed module files. Demo-app bytes remain diagnostics and cannot hide or establish full-library eligibility.
- A passing translated upstream suite and differential workload are strong regression evidence, not a mathematical proof over every input.

