# Current result

Measured on 2026-08-10 with the repository release compiler and Node.js
22.21.1:

| selected cost model | variant | raw | gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: | ---: |
| gzip | nullable operators | 348 | 261 | 242 |
| gzip | explicit branches | 363 | 269 | 247 |
| Brotli | nullable operators | 357 | 276 | 236 |
| Brotli | explicit branches | 364 | 269 | 240 |

The eleven-process median was 2.934 ms for nullable operators versus 2.902 ms
for explicit branches. Median post-GC retained heap was 120,184 bytes versus
124,936 bytes. The transfer win is strict under both independently selected
codec configurations; this sample is 1.1% slower and retains 4,752 fewer bytes,
inside the benchmark's strict 5% runtime and memory policies. The report does
not present the noisy runtime difference as a performance win.
