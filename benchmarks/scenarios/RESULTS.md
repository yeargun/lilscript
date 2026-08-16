# Real-application and mangling results

Generated 2026-08-15T23:45:29.077Z with Node v22.21.1, Vite 8.2.1, Terser 5.43.1, and Closure Compiler 20260804.0.0.

Every JavaScript lane matches the fixed contract before measurement. Application scenarios also match C/native output. This is a configuration/mangling study: its LilScript configs are Brotli-oriented, while raw and gzip are diagnostic cross-metrics that may regress. It is not a three-objective language-superiority gate. Compare rows within one project only.

## Login risk scoring


Contract: `login-risk:3000:3876750:51:15kjg8z`

| Lane | Raw | Gzip-9 | Brotli-11 | Properties |
| --- | ---: | ---: | ---: | --- |
| Vite 8 / Rolldown, minify false | 5639 | 1961 | 1787 | off |
| Vite 8 / Oxc default | 3871 | 1568 | 1412 | off |
| Vite 8 / Terser, `_` properties | 2704 | 1263 | 1156 | private-prefix |
| Closure Compiler ADVANCED / closed app | 2707 | 1308 | 1208 | closed-world |
| LilScript / optimization and mangling off | 4005 | 1627 | 1431 | off |
| LilScript / public-safe identifiers | 1842 | 956 | 875 | off |
| LilScript / closed-world properties | 1871 | 964 | 888 | closed-world |
| LilScript closed world + Vite 8 / Oxc | 2591 | 1087 | 989 | closed-world |

## Animation timeline


Contract: `animation-timeline:72600:173275200:32000000:89375`

| Lane | Raw | Gzip-9 | Brotli-11 | Properties |
| --- | ---: | ---: | ---: | --- |
| Vite 8 / Rolldown, minify false | 4014 | 1550 | 1411 | off |
| Vite 8 / Oxc default | 2301 | 1121 | 1026 | off |
| Vite 8 / Terser, `_` properties | 1483 | 815 | 740 | private-prefix |
| Closure Compiler ADVANCED / closed app | 1512 | 859 | 772 | closed-world |
| LilScript / optimization and mangling off | 1992 | 904 | 824 | off |
| LilScript / public-safe identifiers | 647 | 422 | 382 | off |
| LilScript / closed-world properties | 647 | 422 | 382 | closed-world |
| LilScript closed world + Vite 8 / Oxc | 840 | 474 | 450 | closed-world |

## Geometry hit testing


Contract: `geometry-hit-test:40800:43767221:1000000:true`

| Lane | Raw | Gzip-9 | Brotli-11 | Properties |
| --- | ---: | ---: | ---: | --- |
| Vite 8 / Rolldown, minify false | 4265 | 1495 | 1360 | off |
| Vite 8 / Oxc default | 2870 | 1196 | 1071 | off |
| Vite 8 / Terser, `_` properties | 1814 | 846 | 764 | private-prefix |
| Closure Compiler ADVANCED / closed app | 1712 | 832 | 743 | closed-world |
| LilScript / optimization and mangling off | 68497 | 22868 | 16762 | off |
| LilScript / public-safe identifiers | 489 | 352 | 329 | off |
| LilScript / closed-world properties | 497 | 362 | 327 | closed-world |
| LilScript closed world + Vite 8 / Oxc | 796 | 470 | 431 | closed-world |

## Closed property ledger

This focused host-boundary stress contract is not an npm or application claim. The host observes values but not keys, so property renaming is legal while scalar replacement is not.

Contract: `property-ledger:84960`

| Lane | Raw | Gzip-9 | Brotli-11 | Properties |
| --- | ---: | ---: | ---: | --- |
| Vite 8 / Rolldown, minify false | 289 | 204 | 158 | off |
| Vite 8 / Oxc default | 250 | 195 | 150 | off |
| Vite 8 / Terser, `_` properties | 112 | 125 | 97 | private-prefix |
| Closure Compiler ADVANCED / closed app | 113 | 126 | 100 | closed-world |
| LilScript / optimization and mangling off | 205 | 168 | 139 | off |
| LilScript / public-safe identifiers | 155 | 143 | 107 | off |
| LilScript / closed-world properties | 157 | 143 | 106 | closed-world |
| LilScript closed world + Vite 8 / Oxc | 198 | 166 | 128 | closed-world |
