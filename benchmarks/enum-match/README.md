# Enum and match representation gate

This benchmark measures closed LilScript enums compiled to zero-based numeric
discriminants. It checks the same five-state workload against two deliberately
separate controls:

- a hand-written integer model, which is the performance and no-overhead
  control; enum syntax may tie it but may not produce a larger selected-codec
  artifact;
- a string-tag model, which is the representation-size control and must be
  strictly larger under independently selected gzip and Brotli configurations.

All three variants must produce the same JavaScript and native output. Eleven
alternating isolated Node processes gate median runtime and retained heap
against the integer control. This does not claim numeric enums are globally
interchangeable with string-valued external protocols; conversion at those
boundaries remains explicit.

```sh
node benchmarks/enum-match/run.mjs
```
