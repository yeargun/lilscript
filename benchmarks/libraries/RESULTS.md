# Complete library compatibility results

Generated 2026-08-06T13:20:12.498Z from LilScript `c04c4c2` with Node `v24.11.1`, Vite `8.2.0`, esbuild `0.28.1`, and Closure Compiler `20260803.0.0`.

Each row executes the same checked app contract. The npm rows use the installed package, not a hand-specialized substitute. Closure receives an unminified esbuild bundle of that npm app because Closure does not install or resolve the package itself in this lab. LilScript also emits C and a native executable, and both must match before measurements are published.

## Motion easing

Scope: **Complete @motionone/easing root entrypoint** using `@motionone/easing@10.18.0`.

Contract: `motion-easing:27:560673:541722`

Translated upstream assertions: **27**. Monthly downloads at selection time: **9,574,633**.

| Deployable JavaScript | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli | Median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1091 | 595 | 560 | 0.0% | 5.37 |
| Installed npm package + Closure ADVANCED | 1023 | 574 | 524 | -6.4% | 4.11 |
| LilScript port | 1434 | 815 | 718 | +28.2% | 4.63 |

| Full deploy (HTML + JS) | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Installed npm package + Vite | 1371 | 817 | 686 |
| Installed npm package + Closure ADVANCED | 1209 | 733 | 610 |
| LilScript port | 1620 | 974 | 804 |

## Clamp and lerp

Scope: **Complete clamp and lerp root entrypoints** using `clamp@1.0.1` and `lerp@1.0.3`.

Contract: `micro-math:10:1800000:86076`

Translated upstream assertions: **10**. Monthly downloads at selection time: **4,609,993**.

| Deployable JavaScript | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli | Median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1134 | 607 | 554 | 0.0% | 2.31 |
| Installed npm package + Closure ADVANCED | 1169 | 630 | 562 | +1.4% | 2.35 |
| LilScript port | 557 | 350 | 304 | -45.1% | 0.97 |

| Full deploy (HTML + JS) | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Installed npm package + Vite | 1411 | 828 | 687 |
| Installed npm package + Closure ADVANCED | 1355 | 789 | 648 |
| LilScript port | 743 | 509 | 390 |

## String hash

Scope: **Complete string-hash root entrypoint** using `string-hash@1.1.3`.

Contract: `string-hash:4:1670934855`

Translated upstream assertions: **2**. Monthly downloads at selection time: **19,930,081**.

| Deployable JavaScript | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli | Median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Installed npm package + Vite | 1076 | 637 | 568 | 0.0% | 3.31 |
| Installed npm package + Closure ADVANCED | 1113 | 656 | 581 | +2.3% | 3.31 |
| LilScript port | 532 | 395 | 350 | -38.4% | 2.88 |

| Full deploy (HTML + JS) | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Installed npm package + Vite | 1354 | 857 | 698 |
| Installed npm package + Closure ADVANCED | 1299 | 815 | 667 |
| LilScript port | 718 | 554 | 436 |

## Limits

- Complete means the documented callable root-entrypoint API for the statically typed input domain, not every accidental JavaScript coercion.
- @motionone/easing is a complete published Motion ecosystem package; it is not motion@13 or its DOM engine.
- Runtime measures cache-busted Node module parsing and deterministic app execution. It is not a browser rendering benchmark.
- Transfer sizes sum independently compressed HTTP files. Source bytes are not used as shipping-size evidence.
- A passing translated upstream suite and differential workload are strong regression evidence, not a mathematical proof over every input.

