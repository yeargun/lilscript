# Web-number representation gate

LilScript `int` deliberately has wrapping signed-i32 semantics. The `number`
type instead names JavaScript's IEEE-754 binary64 representation (the existing
`float` spelling remains an alias). This benchmark uses a statically bounded,
integer-valued kernel whose results are identical under both representations;
that makes the transfer and runtime comparison valid for this corpus without
pretending the two types are globally interchangeable.

The exported function keeps the public-parameter boundary observable, where an
`int` must retain coercions and a `number` must not. Both variants execute in
JavaScript and natively. Exact gzip and Brotli compiler configurations require
strict wins, while eleven alternating Node processes gate runtime and retained
heap.

```sh
node benchmarks/web-number/run.mjs
```
