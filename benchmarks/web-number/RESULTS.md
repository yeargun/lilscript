# Current result

Measured on 2026-08-10 with the repository release compiler and Node.js
22.21.1:

| selected cost model | representation | raw | gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: | ---: |
| gzip | `number` | 108 | 119 | 92 |
| gzip | wrapping `int` | 123 | 127 | 98 |
| Brotli | `number` | 109 | 121 | 90 |
| Brotli | wrapping `int` | 126 | 131 | 100 |

The eleven-process median was 0.731 ms for `number` and 0.830 ms for `int`.
Median post-GC retained heap was 11,648 bytes versus 11,776 bytes. The
non-wrapping web-number representation is smaller and faster here while
retaining 128 fewer bytes. Wrapping `int` semantics remain available when the
program requires i32 normalization.
