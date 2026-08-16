# Compact intrinsic gate

This paired benchmark proves the collection, string, and typed-array intrinsic
surface against explicit LilScript loops. Both programs produce the same
observable output. The runner compiles each program twice, once with the gzip
cost model and once with the Brotli cost model, then measures every emitted
artifact with raw bytes, gzip level 9, and Brotli quality 11.

The gate fails unless the gzip-selected intrinsic artifact is strictly smaller
under gzip and the Brotli-selected intrinsic artifact is strictly smaller under
Brotli. Eleven alternating isolated Node processes also enforce a 10% runtime
regression ceiling and a bounded retained-heap ceiling. A separate edge corpus
runs through JavaScript and native code and covers SameValueZero `NaN`, nullable
join fields, negative search offsets, overlapping typed-array aliases,
`copyWithin`, UTF-16 string indices, and `repeat`.

Run it with:

```sh
node benchmarks/compact-intrinsics/run.mjs
```

The benchmark is an isolated proof for these operations, not a universal size
claim. The release gate recompiles it with the current compiler, so checked-in
numbers cannot substitute for current behavior.
