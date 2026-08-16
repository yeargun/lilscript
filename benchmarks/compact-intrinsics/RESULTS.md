# Current result

Measured on 2026-08-10 with the repository release compiler, Node.js 22.21.1,
and Node's gzip-9 and Brotli-11 encoders:

| selected cost model | variant | raw | gzip | Brotli |
| --- | ---: | ---: | ---: | ---: |
| gzip | intrinsics | 465 | 314 | 285 |
| gzip | manual loops | 722 | 402 | 375 |
| Brotli | intrinsics | 467 | 316 | 268 |
| Brotli | manual loops | 709 | 409 | 369 |

The eleven-process median was 6.604 ms for intrinsics and 10.779 ms for manual
loops. Median post-GC retained heap was 124,016 bytes versus 503,048 bytes. In
this fixture the intrinsic spelling is therefore strictly smaller, 39% faster,
and retains about 370 KiB less heap. The edge suite separately checks empty and
non-empty arrays, strings, and typed arrays so the size result is not obtained
by narrowing observable behavior.
