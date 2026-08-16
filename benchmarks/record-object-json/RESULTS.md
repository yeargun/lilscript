# Record, Object, and JSON results

Measured on 2026-08-10 with Node.js 22.21.1. Each production artifact was selected independently for its configured compressor.

| selection | variant | raw | gzip | Brotli |
| --- | ---: | ---: | ---: | ---: |
| gzip | intrinsic | 343 B | 248 B | 208 B |
| gzip | fixed-schema control | 690 B | 330 B | 305 B |
| Brotli | intrinsic | 346 B | 250 B | 199 B |
| Brotli | fixed-schema control | 697 B | 342 B | 297 B |

The intrinsic artifact is 82 B (24.8%) smaller under gzip selection and 98 B (33.0%) smaller under Brotli selection. The control has the same output but assumes the benchmark's six fixed keys; the intrinsic code retains dynamic record behavior.

Eleven isolated-process samples, reported as medians against a capability-equivalent direct JavaScript reference:

| implementation | elapsed | retained heap |
| --- | ---: | ---: |
| compiled intrinsic | 5.856 ms | 127,864 B |
| direct JavaScript | 5.817 ms | 127,416 B |

The compiled result was 0.7% slower in this sample and retained 448 B more heap,
inside the strict 5% runtime and memory guards. JavaScript and native
executables produced the same `530134` result.
