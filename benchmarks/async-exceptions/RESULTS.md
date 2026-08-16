# Async and exception results

Measured on 2026-08-10 with Node 22.21.1, release Lilscript, zlib gzip level 9,
and Brotli quality 11. Both codec-specific configurations selected the same
artifact for this fixture.

| Variant | Raw | gzip | Brotli | Median runtime | Median retained heap |
| --- | ---: | ---: | ---: | ---: | ---: |
| `unused-catch-binding-elision` on | 332 B | 244 B | 209 B | 2.697 ms | 121,968 B |
| off | 335 B | 246 B | 211 B | 2.672 ms | 122,040 B |
| handwritten JavaScript reference | — | — | — | 2.706 ms | 121,808 B |

The enabled artifact is 3 raw bytes and 2 bytes smaller under both gzip and
Brotli. Runtime is 0.9% slower than the disabled artifact and 0.3% faster than
the reference; retained heap differs by at most 160 bytes. All comparisons are
inside the strict 5% runtime and memory gates.
The edge fixture additionally verifies caught `null`, rejected tasks, mutable
state visible to catch/finally, return-through-finally, and binding-free catch.
