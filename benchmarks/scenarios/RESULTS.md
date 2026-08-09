# Real-application and mangling results

Generated 2026-08-09 with Node 24.11.1, Vite 8.2.1, Terser 5.43.1,
Closure Compiler 20260804.0.0, and the checked-in LilScript compiler.

Every JavaScript lane below matches the fixed contract. The first three
projects also match emitted C and native execution. Raw, gzip-9, and Brotli-11
are independent byte measurements. Compare rows within one project only.

## Login risk scoring

Contract: `login-risk:3000:3876750:51:15kjg8z`

| Lane | Raw | Gzip-9 | Brotli-11 | Properties |
| --- | ---: | ---: | ---: | --- |
| Vite 8 / Rolldown, minify false | 5639 | 1983 | 1787 | off |
| Vite 8 / Oxc default | 3871 | 1577 | 1412 | off |
| Vite 8 / Terser, `_` properties | 2704 | 1264 | 1156 | private-prefix |
| Closure Compiler ADVANCED / closed app | 2707 | 1325 | 1208 | closed-world |
| LilScript / optimization and mangling off | 4048 | 1641 | 1428 | off |
| LilScript / public-safe identifiers | 1987 | 995 | 902 | off |
| LilScript / closed-world properties | 1987 | 995 | 902 | closed-world |
| LilScript closed world + Vite 8 / Oxc | 2678 | 1100 | 990 | closed-world |

## Animation timeline

Contract: `animation-timeline:72600:173275200:32000000:89375`

| Lane | Raw | Gzip-9 | Brotli-11 | Properties |
| --- | ---: | ---: | ---: | --- |
| Vite 8 / Rolldown, minify false | 4014 | 1564 | 1411 | off |
| Vite 8 / Oxc default | 2301 | 1118 | 1026 | off |
| Vite 8 / Terser, `_` properties | 1483 | 818 | 740 | private-prefix |
| Closure Compiler ADVANCED / closed app | 1512 | 869 | 772 | closed-world |
| LilScript / optimization and mangling off | 1996 | 914 | 820 | off |
| LilScript / public-safe identifiers | 677 | 454 | 419 | off |
| LilScript / closed-world properties | 677 | 454 | 419 | closed-world |
| LilScript closed world + Vite 8 / Oxc | 936 | 526 | 510 | closed-world |

## Geometry hit testing

Contract: `geometry-hit-test:40800:43767221:1000000:true`

| Lane | Raw | Gzip-9 | Brotli-11 | Properties |
| --- | ---: | ---: | ---: | --- |
| Vite 8 / Rolldown, minify false | 4265 | 1509 | 1360 | off |
| Vite 8 / Oxc default | 2870 | 1208 | 1071 | off |
| Vite 8 / Terser, `_` properties | 1814 | 856 | 764 | private-prefix |
| Closure Compiler ADVANCED / closed app | 1712 | 848 | 743 | closed-world |
| LilScript / optimization and mangling off | 68658 | 22816 | 16760 | off |
| LilScript / public-safe identifiers | 1553 | 760 | 642 | off |
| LilScript / closed-world properties | 1553 | 760 | 642 | closed-world |
| LilScript closed world + Vite 8 / Oxc | 1698 | 660 | 604 | closed-world |

## Closed property ledger

This is a focused host-boundary stress contract, not an npm or application
claim. The host observes record values but not keys, so property renaming is
legal while scalar replacement is not.

Contract: `property-ledger:84960`

| Lane | Raw | Gzip-9 | Brotli-11 | Properties |
| --- | ---: | ---: | ---: | --- |
| Vite 8 / Rolldown, minify false | 289 | 205 | 158 | off |
| Vite 8 / Oxc default | 250 | 194 | 150 | off |
| Vite 8 / Terser, `_` properties | 112 | 125 | 97 | private-prefix |
| Closure Compiler ADVANCED / closed app | 113 | 128 | 100 | closed-world |
| LilScript / optimization and mangling off | 206 | 170 | 141 | off |
| LilScript / public-safe identifiers | 155 | 143 | 107 | off |
| LilScript / closed-world properties | 105 | 117 | 90 | closed-world |
| LilScript closed world + Vite 8 / Oxc | 147 | 141 | 115 | closed-world |
