# Results

Measured with Node 22.21.1 on 2026-08-10. Each codec uses its own production
candidate-search configuration. Runtime and retained heap are medians of 11
fresh-process samples in alternating order.

| selection | spelling | raw | gzip | Brotli |
|---|---:|---:|---:|---:|
| gzip | collection syntax | 466 B | 283 B | 254 B |
| gzip | manual copy/loops | 546 B | 321 B | 285 B |
| Brotli | collection syntax | 473 B | 286 B | 240 B |
| Brotli | manual copy/loops | 562 B | 327 B | 275 B |

The gzip-selected artifact is 38 B smaller under gzip. The Brotli-selected
artifact is 35 B smaller under Brotli. Against a hand-written JavaScript
reference with the same i32, missing-value, shallow-copy, and null-prototype
semantics, the compiled lane measured 6.956 ms versus 7.416 ms and retained
118,296 B versus 120,256 B. Both JavaScript and native edge suites produce the
same output, including short arrays, evaluation once, source mutation after
copy, numeric record-key order, `__proto__`, and inherited-key absence.
