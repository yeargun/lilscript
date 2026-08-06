# Mechanically paired compiler gate

This lane removes source-style judgment from the comparison. A small neutral
integer-and-boolean expression/statement schema in `specs.json` is rendered
into readable LilScript and JavaScript by the same generator. The JavaScript
renderer inserts the signed-32-bit operations required to match LilScript
`int` semantics.

`run.mjs` builds each generated LilScript program as JavaScript, C, and a native
executable, compiles the paired JavaScript with pinned Closure ADVANCED, and
requires identical output. It then rejects the release if LilScript is larger
than Closure for any individual workload under raw bytes, gzip-9, or Brotli-11.
The gate is deliberately corpus-scoped; it is not a claim about arbitrary
programs.

```sh
node benchmarks/paired/run.mjs
node benchmarks/paired/run.mjs --check
```

The first command refreshes the checked-in reports. `--check` runs the same
behavior and size gates without rewriting published metadata.
