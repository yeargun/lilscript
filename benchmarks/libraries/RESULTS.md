# Complete library compatibility results

Generated 2026-08-06T22:15:13.123Z from LilScript `21c9b5d` with Node `v24.11.1`, Vite `8.2.0`, esbuild `0.28.1`, and Closure Compiler `20260803.0.0`.

Each row executes the same checked app contract. The npm rows use the installed package, not a hand-specialized substitute. Closure receives an unminified esbuild bundle of that npm app because Closure does not install or resolve the package itself in this lab. LilScript also emits C and a native executable, and both must match before measurements are published.

## Motion easing

Scope: **Complete @motionone/easing root entrypoint** using `@motionone/easing@10.18.0`.

Contract: `motion-easing:27:560673:541722`

Translated upstream assertions: **27**. Added package-contract assertions: **0**. Monthly downloads at selection time: **9,574,633**.

| Deployable JavaScript | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli | Median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1107 | 608 | 579 | 0.0% | 5.01 |
| Installed npm package + Closure ADVANCED | 1024 | 583 | 530 | -8.5% | 3.92 |
| LilScript port | 1151 | 614 | 562 | -2.9% | 4.36 |

| Full deploy (HTML + JS) | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Installed npm package + Vite | 1387 | 829 | 712 |
| Installed npm package + Closure ADVANCED | 1210 | 742 | 616 |
| LilScript port | 1337 | 773 | 648 |

## Clamp and lerp

Scope: **Complete clamp and lerp root entrypoints** using `clamp@1.0.1` and `lerp@1.0.3`.

Contract: `micro-math:10:1800000:86076`

Translated upstream assertions: **10**. Added package-contract assertions: **0**. Monthly downloads at selection time: **4,609,993**.

| Deployable JavaScript | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli | Median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1134 | 607 | 554 | 0.0% | 2.33 |
| Installed npm package + Closure ADVANCED | 1169 | 630 | 562 | +1.4% | 2.35 |
| LilScript port | 529 | 319 | 280 | -49.5% | 0.95 |

| Full deploy (HTML + JS) | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Installed npm package + Vite | 1411 | 828 | 687 |
| Installed npm package + Closure ADVANCED | 1355 | 789 | 648 |
| LilScript port | 715 | 478 | 366 |

## String hash

Scope: **Complete string-hash root entrypoint** using `string-hash@1.1.3`.

Contract: `string-hash:4:1670934855`

Translated upstream assertions: **2**. Added package-contract assertions: **2**. Monthly downloads at selection time: **19,930,081**.

| Deployable JavaScript | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli | Median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1076 | 637 | 568 | 0.0% | 3.37 |
| Installed npm package + Closure ADVANCED | 1113 | 656 | 581 | +2.3% | 3.41 |
| LilScript port | 481 | 354 | 308 | -45.8% | 2.83 |

| Full deploy (HTML + JS) | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Installed npm package + Vite | 1354 | 857 | 698 |
| Installed npm package + Closure ADVANCED | 1299 | 815 | 667 |
| LilScript port | 667 | 513 | 394 |

## Levenshtein distance

Scope: **Complete js-levenshtein root entrypoint** using `js-levenshtein@1.1.6`.

Contract: `js-levenshtein:14:2049950`

Translated upstream assertions: **14**. Added package-contract assertions: **0**. Monthly downloads at selection time: **41,652,021**.

| Deployable JavaScript | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli | Median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1970 | 1077 | 969 | 0.0% | 6.01 |
| Installed npm package + Closure ADVANCED | 2030 | 1099 | 996 | +2.8% | 6.01 |
| LilScript port | 1582 | 899 | 778 | -19.7% | 6.15 |

| Full deploy (HTML + JS) | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Installed npm package + Vite | 2251 | 1300 | 1099 |
| Installed npm package + Closure ADVANCED | 2216 | 1258 | 1082 |
| LilScript port | 1768 | 1058 | 864 |

## Emotion hash

Scope: **Complete @emotion/hash root entrypoint** using `@emotion/hash@0.9.2`.

Contract: `emotion-hash:8:30831534`

Translated upstream assertions: **1**. Added package-contract assertions: **7**. Monthly downloads at selection time: **122,398,176**.

| Deployable JavaScript | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli | Median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 907 | 498 | 430 | 0.0% | 11.65 |
| Installed npm package + Closure ADVANCED | 924 | 496 | 434 | +0.9% | 11.67 |
| LilScript port | 816 | 538 | 456 | +6.0% | 11.24 |

| Full deploy (HTML + JS) | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Installed npm package + Vite | 1186 | 716 | 555 |
| Installed npm package + Closure ADVANCED | 1110 | 655 | 520 |
| LilScript port | 1002 | 697 | 542 |

## MurmurHash 2 and 3

Scope: **Complete murmurhash-js root entrypoint** using `murmurhash-js@1.0.0`.

Contract: `murmurhash-js:18:855861453`

Translated upstream assertions: **0**. Added package-contract assertions: **18**. Monthly downloads at selection time: **24,364,654**.

| Deployable JavaScript | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli | Median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 2782 | 1118 | 992 | 0.0% | 5.10 |
| Installed npm package + Closure ADVANCED | 2905 | 1203 | 1059 | +6.8% | 5.08 |
| LilScript port | 1741 | 840 | 740 | -25.4% | 4.38 |

| Full deploy (HTML + JS) | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Installed npm package + Vite | 3062 | 1340 | 1131 |
| Installed npm package + Closure ADVANCED | 3091 | 1362 | 1145 |
| LilScript port | 1927 | 999 | 826 |

## Limits

- Complete means the documented callable root-entrypoint API for the statically typed input domain, not every accidental JavaScript coercion.
- @motionone/easing is a complete published Motion ecosystem package; it is not motion@13 or its DOM engine.
- Runtime measures cache-busted Node module parsing and deterministic app execution. It is not a browser rendering benchmark.
- Transfer sizes sum independently compressed HTTP files. Source bytes are not used as shipping-size evidence.
- A passing translated upstream suite and differential workload are strong regression evidence, not a mathematical proof over every input.

