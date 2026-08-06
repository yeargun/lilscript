# Complete library compatibility results

Generated 2026-08-06T17:16:57.300Z from LilScript `808a2b9` with Node `v24.11.1`, Vite `8.2.0`, esbuild `0.28.1`, and Closure Compiler `20260803.0.0`.

Each row executes the same checked app contract. The npm rows use the installed package, not a hand-specialized substitute. Closure receives an unminified esbuild bundle of that npm app because Closure does not install or resolve the package itself in this lab. LilScript also emits C and a native executable, and both must match before measurements are published.

## Motion easing

Scope: **Complete @motionone/easing root entrypoint** using `@motionone/easing@10.18.0`.

Contract: `motion-easing:27:560673:541722`

Translated upstream assertions: **27**. Added package-contract assertions: **0**. Monthly downloads at selection time: **9,574,633**.

| Deployable JavaScript | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli | Median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1091 | 595 | 560 | 0.0% | 6.30 |
| Installed npm package + Closure ADVANCED | 1023 | 574 | 524 | -6.4% | 4.93 |
| LilScript port | 1432 | 814 | 715 | +27.7% | 5.49 |

| Full deploy (HTML + JS) | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Installed npm package + Vite | 1371 | 817 | 686 |
| Installed npm package + Closure ADVANCED | 1209 | 733 | 610 |
| LilScript port | 1618 | 973 | 801 |

## Clamp and lerp

Scope: **Complete clamp and lerp root entrypoints** using `clamp@1.0.1` and `lerp@1.0.3`.

Contract: `micro-math:10:1800000:86076`

Translated upstream assertions: **10**. Added package-contract assertions: **0**. Monthly downloads at selection time: **4,609,993**.

| Deployable JavaScript | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli | Median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1134 | 607 | 554 | 0.0% | 2.64 |
| Installed npm package + Closure ADVANCED | 1169 | 630 | 562 | +1.4% | 2.69 |
| LilScript port | 557 | 350 | 304 | -45.1% | 1.19 |

| Full deploy (HTML + JS) | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Installed npm package + Vite | 1411 | 828 | 687 |
| Installed npm package + Closure ADVANCED | 1355 | 789 | 648 |
| LilScript port | 743 | 509 | 390 |

## String hash

Scope: **Complete string-hash root entrypoint** using `string-hash@1.1.3`.

Contract: `string-hash:4:1670934855`

Translated upstream assertions: **2**. Added package-contract assertions: **2**. Monthly downloads at selection time: **19,930,081**.

| Deployable JavaScript | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli | Median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1076 | 637 | 568 | 0.0% | 3.82 |
| Installed npm package + Closure ADVANCED | 1113 | 656 | 581 | +2.3% | 3.96 |
| LilScript port | 533 | 391 | 342 | -39.8% | 3.24 |

| Full deploy (HTML + JS) | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Installed npm package + Vite | 1354 | 857 | 698 |
| Installed npm package + Closure ADVANCED | 1299 | 815 | 667 |
| LilScript port | 719 | 550 | 428 |

## Levenshtein distance

Scope: **Complete js-levenshtein root entrypoint** using `js-levenshtein@1.1.6`.

Contract: `js-levenshtein:14:2049950`

Translated upstream assertions: **14**. Added package-contract assertions: **0**. Monthly downloads at selection time: **41,652,021**.

| Deployable JavaScript | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli | Median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1970 | 1077 | 969 | 0.0% | 6.57 |
| Installed npm package + Closure ADVANCED | 2030 | 1099 | 996 | +2.8% | 6.64 |
| LilScript port | 1737 | 976 | 854 | -11.9% | 7.47 |

| Full deploy (HTML + JS) | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Installed npm package + Vite | 2251 | 1300 | 1099 |
| Installed npm package + Closure ADVANCED | 2216 | 1258 | 1082 |
| LilScript port | 1923 | 1135 | 940 |

## Emotion hash

Scope: **Complete @emotion/hash root entrypoint** using `@emotion/hash@0.9.2`.

Contract: `emotion-hash:8:30831534`

Translated upstream assertions: **1**. Added package-contract assertions: **7**. Monthly downloads at selection time: **122,398,176**.

| Deployable JavaScript | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli | Median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 907 | 498 | 430 | 0.0% | 12.49 |
| Installed npm package + Closure ADVANCED | 924 | 496 | 434 | +0.9% | 12.49 |
| LilScript port | 986 | 587 | 506 | +17.7% | 11.96 |

| Full deploy (HTML + JS) | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Installed npm package + Vite | 1186 | 716 | 555 |
| Installed npm package + Closure ADVANCED | 1110 | 655 | 520 |
| LilScript port | 1172 | 746 | 592 |

## MurmurHash 2 and 3

Scope: **Complete murmurhash-js root entrypoint** using `murmurhash-js@1.0.0`.

Contract: `murmurhash-js:18:855861453`

Translated upstream assertions: **0**. Added package-contract assertions: **18**. Monthly downloads at selection time: **24,364,654**.

| Deployable JavaScript | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli | Median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 2782 | 1118 | 992 | 0.0% | 5.15 |
| Installed npm package + Closure ADVANCED | 2905 | 1203 | 1059 | +6.8% | 5.17 |
| LilScript port | 1990 | 913 | 789 | -20.5% | 4.49 |

| Full deploy (HTML + JS) | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Installed npm package + Vite | 3062 | 1340 | 1131 |
| Installed npm package + Closure ADVANCED | 3091 | 1362 | 1145 |
| LilScript port | 2176 | 1072 | 875 |

## Limits

- Complete means the documented callable root-entrypoint API for the statically typed input domain, not every accidental JavaScript coercion.
- @motionone/easing is a complete published Motion ecosystem package; it is not motion@13 or its DOM engine.
- Runtime measures cache-busted Node module parsing and deterministic app execution. It is not a browser rendering benchmark.
- Transfer sizes sum independently compressed HTTP files. Source bytes are not used as shipping-size evidence.
- A passing translated upstream suite and differential workload are strong regression evidence, not a mathematical proof over every input.

