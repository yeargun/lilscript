# Native runtime recipes

These are the production C source/header definitions embedded into a standalone
C translation unit. `native_runtime::Helper` owns their dependency closure and
emission order. The compiler admits their bytes through the artifact budget;
there is no runtime library search or emitted-source rewriting step.

`prologue.h` enforces strict C11 numeric assumptions. `string.h`, `memory.h` and
`value.h` define the public ownership interfaces. `memory.c` owns every retained
allocation and the cycle collector. Generated array, class, product, box and
closure traces enumerate exactly the references retained by their schema;
borrowed pointers and raw buffers are not owner edges.

Cycle collection uses synchronous candidate trial deletion: nonzero decrements
buffer traceable objects; a gray traversal subtracts internal edges from trial
counts; surviving external counts seed a black reachability traversal; the
remaining white objects lose their outgoing edges before any white allocation
is freed. Real counts are not modified during discovery. Intrusive work lists
bound stack use in traversal and zero-count destruction. The default allocation
interval is configurable and grows with live heap size. Collection runs before
allocation and at shutdown; generated destruction cannot call user code.
This follows the synchronous trial-deletion model discussed by
[Bacon and Rajan](https://doi.org/10.1007/3-540-45337-7_12); the implementation is
single-threaded and does not claim their concurrent collector protocol.

Strings use ABI 2: `{data, length, owner}`. Static literal/host storage has a null
owner. Stored copies retain, overwrites/exits release, returned strings transfer
one owner, and slices own their backing allocation. String conversions used in
template joining release every temporary. Values stored in products and tagged
containers follow the same ownership rules.

`tests/native/runtime-ownership.c` exercises these exact C recipes independently
of source optimization. `tests/native/qualify-ownership.mjs` also compiles real
source into C and runs it with GCC, Clang and ASan/UBSan, under `-Werror`.
The tests are a focused gate, not a claim of complete native coverage.
