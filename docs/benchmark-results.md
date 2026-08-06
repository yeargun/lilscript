# LilScript v0.1 Bundle Benchmark

Measured on 2026-08-06 with LilScript release mode and Google Closure Compiler
`v20260803` at `ADVANCED` compilation level. The Closure version is pinned in
`benchmarks/run.sh` and downloaded from Maven Central:

`https://repo.maven.apache.org/maven2/com/google/javascript/closure-compiler/v20260803/`

## Results

### `v01`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 111 | 124 | 97 |
| Closure ADVANCED v20260803 | 212 | 190 | 166 |

This case exercises class constructor/method devirtualization, scalar
replacement, array map callbacks, integer lowering, a counted loop, branch
folding, and template output.

### `conformance`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 268 | 207 | 190 |
| Closure ADVANCED v20260803 | 378 | 258 | 214 |

This case exercises mutable array operations, filter/reduce/forEach callbacks,
string predicates, case conversion, templates, and constant propagation.

### `full_conformance`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 348 | 270 | 224 |
| Closure ADVANCED v20260803 | 547 | 368 | 309 |

This broader case combines structs, mutable classes, direct functions, global
and local closures, while/for control flow, break/continue, short-circuit
logic, compound arithmetic, arrays, strings, and templates.

### `optimizer_stress`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 128 | 87 | 68 |
| Closure ADVANCED v20260803 | 239 | 126 | 90 |

This case isolates shared-constant propagation, deep inlining, local value
numbering, class dissolution, repeated-string aliasing, and dead function and
branch removal.

### `algorithms`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 192 | 167 | 139 |
| Closure ADVANCED v20260803 | 205 | 169 | 144 |

This case combines recursion with single-use multi-block CFG inlining, two
iterative algorithms, loop-phi coalescing, conditional returns, and integer
normalization.

### `data_model`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 95 | 88 | 68 |
| Closure ADVANCED v20260803 | 248 | 170 | 130 |

This case measures nested value structs, class construction, mutable methods,
field-index lowering, devirtualization, and scalar replacement.

### `higher_order`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 126 | 128 | 110 |
| Closure ADVANCED v20260803 | 141 | 140 | 124 |

This case measures map/filter fusion at emission time, reduce, block-arrow
inlining, callback effects, and captured immutable globals.

### `string_optimization`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 119 | 97 | 65 |
| Closure ADVANCED v20260803 | 265 | 132 | 97 |

This case measures compile-time string predicates and case conversion,
short-circuit folding, templates, and repeated long-string reuse.

### `modules`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 113 | 127 | 98 |
| Closure ADVANCED v20260803 | 122 | 134 | 108 |

This case gives each compiler three real source modules. It measures relative
import resolution, transitive linking, aliases, exported and private bindings,
cross-module multi-block inlining, purity-guided DCE, loop-phi copy ordering,
and complete removal of module syntax and unused exports.

### Corpus total

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 1,500 | 1,295 | 1,059 |
| Closure ADVANCED v20260803 | 2,357 | 1,687 | 1,382 |

## Method

Each LilScript workload has a behaviorally equivalent JavaScript input under
`benchmarks/`. Integer operations in the JavaScript references explicitly use
32-bit normalization so Closure is measured against the same semantics.

For every case, `benchmarks/run.sh`:

1. builds LilScript in Cargo release mode;
2. compiles LilScript to optimized JavaScript;
3. invokes Closure with `--compilation_level ADVANCED`;
4. executes both outputs under Node and rejects any stdout difference;
5. removes trailing whitespace and measures UTF-8 bytes;
6. measures Node `gzipSync` level 9 with deterministic `mtime: 0`;
7. measures Node Brotli quality 11.

Run the complete comparison with:

```sh
benchmarks/run.sh
```

The measured conclusion is deliberately scoped: LilScript is smaller in all
27 raw, gzip, and Brotli cells.
The suite is reproducible evidence for these features, not proof of universal
superiority over arbitrary JavaScript or future language features. The broader
application comparison, including ecosystem dependencies, hand-specialized
JavaScript, runtime samples, generated C, and native behavior checks, is in
[`../benchmarks/apps/RESULTS.md`](../benchmarks/apps/RESULTS.md).

## Complete library ports

The separate [`../benchmarks/libraries`](../benchmarks/libraries) project uses
installed, version-pinned npm packages rather than matching-scope synthetic
implementations. It currently covers the complete documented callable root
entrypoints of `@motionone/easing@10.18.0`, `clamp@1.0.1`, `lerp@1.0.3`, and
`string-hash@1.1.3` for their typed input domains.

Each case must match through Vite, an esbuild-to-Closure ADVANCED pipeline,
LilScript JavaScript, emitted C, and a native executable. Translated upstream
assertions and dense differential API grids run after the app contracts. The
generated tables are in
[`../benchmarks/libraries/RESULTS.md`](../benchmarks/libraries/RESULTS.md).
They show smaller LilScript Brotli payloads for the clamp/lerp and string-hash
apps, but a larger payload for Motion easing; no universal size claim follows.
