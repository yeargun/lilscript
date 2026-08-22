# Complete library compatibility diagnostics

Generated 2026-08-18T15:01:20.724Z from LilScript `1e3b916` with Node `v22.21.1`, Vite `8.2.1`, esbuild `0.28.1`, and Closure Compiler `20260804.0.0`.

Each row executes the same checked app contract, but size eligibility is measured on the reusable selected root API so whole-program constant specialization cannot remove the library implementation. The npm rows use the installed package, not a hand-specialized substitute. Closure receives an unminified esbuild bundle that exposes the same named public surface. LilScript's raw, gzip-9, and Brotli-11 cells come from independent objective builds, and each build is judged only on its matching metric. LilScript also emits C and a native executable, and both must match before measurements are considered.

Publication gate: the raw-objective artifact's raw bytes and the brotli-objective artifact's matching compressed bytes must be no larger than both npm/Vite and public-contract-preserving Closure ADVANCED; median library-workload time and retained memory must each be at most 1.05× npm. Eligible: **5/7**. Blocked rows remain below strictly as compiler diagnostics.

## Motion easing

Status: **blocked** — throughput ratio 1.061 exceeds 1.05.

Scope: **Complete @motionone/easing root entrypoint** using `@motionone/easing@10.18.0`.

Contract: `motion-easing:27:560673:541722`

Translated upstream assertions: **27**. Added package-contract assertions: **0**. Monthly downloads at selection time: **9,574,633**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 840 | 456 | 431 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 445 | 325 | 286 | -33.6% |
| LilScript reusable objective builds | 407 | 285 | 266 | -38.3% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 13.709 | 14.550 | 1.061 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 3833112 | 3502352 | 0.914 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1107 | 606 | 579 | 6.52 |
| Installed npm package + Closure ADVANCED | 1024 | 577 | 530 | 5.49 |
| LilScript port | 961 | 527 | 487 | 7.14 |

## Clamp and lerp

Status: **eligible**.

Scope: **Complete clamp and lerp root entrypoints** using `clamp@1.0.1` and `lerp@1.0.3`.

Contract: `micro-math:10:1800000:86076`

Translated upstream assertions: **10**. Added package-contract assertions: **0**. Monthly downloads at selection time: **4,609,993**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 1137 | 577 | 519 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 833 | 482 | 414 | -20.2% |
| LilScript reusable objective builds | 109 | 142 | 112 | -78.4% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 8.141 | 8.119 | 0.997 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 317720 | 317816 | 1.000 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1134 | 608 | 554 | 3.85 |
| Installed npm package + Closure ADVANCED | 1169 | 625 | 562 | 3.83 |
| LilScript port | 314 | 224 | 201 | 1.59 |

## String hash

Status: **blocked** — throughput ratio 1.090 exceeds 1.05.

Scope: **Complete string-hash root entrypoint** using `string-hash@1.1.3`.

Contract: `string-hash:4:1670934855`

Translated upstream assertions: **2**. Added package-contract assertions: **2**. Monthly downloads at selection time: **19,930,081**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 969 | 559 | 502 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 744 | 473 | 403 | -19.7% |
| LilScript reusable objective builds | 118 | 131 | 94 | -81.3% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 14.184 | 15.467 | 1.090 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 317656 | 317800 | 1.000 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1076 | 638 | 568 | 3.46 |
| Installed npm package + Closure ADVANCED | 1113 | 651 | 581 | 3.39 |
| LilScript port | 441 | 327 | 282 | 2.88 |

## Levenshtein distance

Status: **eligible**.

Scope: **Complete js-levenshtein root entrypoint** using `js-levenshtein@1.1.6`.

Contract: `js-levenshtein:14:2049950`

Translated upstream assertions: **14**. Added package-contract assertions: **0**. Monthly downloads at selection time: **41,652,021**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 1984 | 914 | 825 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 1453 | 788 | 714 | -13.5% |
| LilScript reusable objective builds | 830 | 438 | 404 | -51.0% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 34.708 | 33.575 | 0.967 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 317392 | 317536 | 1.000 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1970 | 1067 | 969 | 9.37 |
| Installed npm package + Closure ADVANCED | 2030 | 1081 | 996 | 9.43 |
| LilScript port | 1381 | 778 | 671 | 8.64 |

## Emotion hash

Status: **eligible**.

Scope: **Complete @emotion/hash root entrypoint** using `@emotion/hash@0.9.2`.

Contract: `emotion-hash:8:30831534`

Translated upstream assertions: **1**. Added package-contract assertions: **7**. Monthly downloads at selection time: **122,398,176**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 833 | 348 | 330 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 594 | 287 | 240 | -27.3% |
| LilScript reusable objective builds | 427 | 265 | 231 | -30.0% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 35.340 | 34.138 | 0.966 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 637440 | 637568 | 1.000 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 907 | 492 | 430 | 14.05 |
| Installed npm package + Closure ADVANCED | 924 | 487 | 434 | 13.97 |
| LilScript port | 766 | 493 | 428 | 14.00 |

## MurmurHash 2 and 3

Status: **eligible**.

Scope: **Complete murmurhash-js root entrypoint** using `murmurhash-js@1.0.0`.

Contract: `murmurhash-js:18:855861453`

Translated upstream assertions: **0**. Added package-contract assertions: **18**. Monthly downloads at selection time: **24,364,654**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 3131 | 1029 | 902 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 2446 | 951 | 833 | -7.6% |
| LilScript reusable objective builds | 992 | 446 | 409 | -54.7% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 22.887 | 18.717 | 0.818 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 317712 | 317856 | 1.000 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 2782 | 1115 | 992 | 6.13 |
| Installed npm package + Closure ADVANCED | 2905 | 1190 | 1059 | 6.14 |
| LilScript port | 1499 | 692 | 627 | 6.56 |

## Robust geometric predicates

Status: **eligible**.

Scope: **Complete robust-predicates root entrypoint** using `robust-predicates@3.0.3`.

Contract: `robust-predicates:8`

Translated upstream assertions: **23798**. Added package-contract assertions: **320016**. Monthly downloads at selection time: **101,525,533**.

| Reusable selected API | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite library mode | 38722 | 8329 | 6456 | 0.0% |
| Installed npm package + Closure ADVANCED public surface | 25208 | 7872 | 6228 | -3.5% |
| LilScript reusable objective builds | 19759 | 7691 | 5972 | -7.5% |

| Isolated API workload | npm | LilScript | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| Median time (ms) | 11.018 | 10.998 | 0.998 | ≤1.05 |
| Retained heap + ArrayBuffer (B) | 258616 | 258296 | 0.999 | ≤1.05 |

| Checked demo app | Raw JS | Gzip-9 | Brotli-11 | Median load + execution ms |
| --- | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 24227 | 7418 | 5960 | 0.63 |
| Installed npm package + Closure ADVANCED | 25369 | 7856 | 6230 | 0.97 |
| LilScript port | 117 | 103 | 93 | 0.06 |

## Limits

- Complete means the documented callable root-entrypoint API for the statically typed input domain, not every accidental JavaScript coercion.
- @motionone/easing is a complete published Motion ecosystem package; it is not motion@13 or its DOM engine.
- Runtime measures cache-busted Node module parsing and deterministic app execution. It is not a browser rendering benchmark.
- API throughput and retained memory use medians from 9 isolated Node processes per implementation and mode, with alternating order, identical workloads and checksums, forced GC, and equivalent retained results. The memory lane performs one complete unretained workload before its baseline GC so JIT tier-up is outside the retained delta.
- Reusable-surface transfer sizes sum independently compressed module files. Every LilScript size cell uses the build selected for that exact objective; the other metrics of each build are diagnostic and may lose. Demo-app bytes and runtime come from the explicitly declared Brotli-objective build, remain diagnostics, and cannot hide or establish full-library eligibility.
- A passing translated upstream suite and differential workload are strong regression evidence, not a mathematical proof over every input.

