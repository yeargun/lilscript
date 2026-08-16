# Current result

Measured on 2026-08-10 with the repository release compiler and Node.js
22.21.1:

| selected cost model | representation | raw | gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: | ---: |
| gzip | LilScript enum/match | 195 | 150 | 144 |
| gzip | hand-written integers | 195 | 150 | 144 |
| gzip | string tags | 252 | 180 | 164 |
| Brotli | LilScript enum/match | 215 | 158 | 128 |
| Brotli | hand-written integers | 219 | 161 | 132 |
| Brotli | string tags | 258 | 186 | 162 |

The eleven-process medians were 2.169 ms and 117,728 retained bytes for enum
syntax, 2.096 ms and 115,584 bytes for hand-written integers, and 2.300 ms and
119,400 bytes for string tags. The gzip-selected enum and integer artifacts are
byte-identical. Brotli candidate search selected different but semantically
equivalent layouts: enum syntax is 4 Brotli bytes smaller, measured 3.5% slower,
and retained 2,144 more bytes than the integer control, all within the strict 5%
time and memory gates. It remains both smaller and faster than string tags
in this run.
