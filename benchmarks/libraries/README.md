# Library compatibility lab

This project compares version-pinned, installed npm packages with complete LilScript ports
of their documented callable root entrypoints. It is separate from the synthetic
compiler corpus and from context-only ecosystem builds.

```sh
cd benchmarks/libraries
npm install
npm run benchmark
```

The harness performs four behavior gates for each app:

1. installed package built by Vite 8;
2. the same installed package prebundled by esbuild and optimized by Closure ADVANCED;
3. LilScript-generated JavaScript;
4. LilScript-generated C and the native executable compiled from it.

It then runs dense differential API tests from `test/compatibility.test.mjs`.
The current gate covers seven npm packages across six independently built apps.
Only JavaScript and complete deploy sizes are reported. Native artifacts are
correctness gates, not transfer-size rows.

Selection evidence and exclusions live in `compatibility/libraries.json`.
Generated measurements are written to `RESULTS.md`, `build/results.json`, and
`web/src/library-results.json`.
