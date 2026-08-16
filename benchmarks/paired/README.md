# Mechanically paired compiler gate

This lane removes source-style judgment from the comparison. A small neutral
integer-and-boolean expression/statement schema in `specs.json` is rendered
into readable LilScript and JavaScript by the same generator. The JavaScript
renderer emits ordinary multiplication with i32 normalization for LilScript
`*` and preserves schema-authored `Math.imul` calls, so the two integer
multiplication contracts are compared separately.

`run.mjs` builds each generated LilScript program as JavaScript, C, and a native
executable, compiles the paired JavaScript with pinned Closure ADVANCED, and
requires identical output. It publishes raw, gzip-9, and Brotli-11 for every
row. LilScript is compiled with the explicit checked-in Brotli cost-model config,
then the runner rejects the release only if that artifact is larger than Closure
under Brotli-11. Its raw and gzip sizes are diagnostic and may lose. Requiring
one artifact to win all three codecs
would contradict codec-specific candidate selection when the compressors rank
equivalent programs differently. The gate is deliberately corpus-scoped; it is
not a claim about arbitrary programs.

```sh
node benchmarks/paired/run.mjs
node benchmarks/paired/run.mjs --check
```

The first command refreshes the checked-in reports. `--check` runs the same
behavior and size gates without rewriting published metadata.
