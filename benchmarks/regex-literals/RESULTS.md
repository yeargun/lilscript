# Results

Measured with Node 22.21.1 on 2026-08-10 from a fresh release compiler build.
Runtime and retained heap are medians of 11 fresh-process samples in alternating
order.

| spelling | raw | gzip | Brotli | runtime | retained heap |
|---|---:|---:|---:|---:|---:|
| proven regex literals | 225 B | 169 B | 134 B | 2.622 ms | 124,080 B |
| `new RegExp` constructors | 283 B | 182 B | 146 B | 2.636 ms | 122,104 B |
| direct JavaScript reference | — | — | — | 2.642 ms | 125,288 B |

Literal selection saves 58 raw bytes, 13 gzip bytes, and 12 Brotli bytes. Its
median runtime is within 1% of both constructor output and the direct reference.
It retains 1,976 bytes more than the constructor control and 1,208 fewer than
the direct reference, inside the bounded-memory gate. Both modes produce
`60000`. The separate edge
suite also verifies `g`-flag `lastIndex` state changes, metadata, an escaped
complex pattern that must remain a constructor, and identical observable output.
