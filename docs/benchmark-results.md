# LilScript v0.1 Bundle Benchmark

Measured on 2026-08-06 with LilScript release mode and Google Closure Compiler
`v20260803` at `ADVANCED` compilation level. The Closure version is pinned in
`benchmarks/run.sh` and downloaded from Maven Central:

`https://repo.maven.apache.org/maven2/com/google/javascript/closure-compiler/v20260803/`

## Results

### `v01`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 111 | 126 | 103 |
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
| LilScript | 355 | 271 | 221 |
| Closure ADVANCED v20260803 | 547 | 368 | 309 |

This broader case combines structs, mutable classes, direct functions, global
and local closures, while/for control flow, break/continue, short-circuit
logic, compound arithmetic, arrays, strings, and templates.

### `optimizer_stress`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 128 | 87 | 69 |
| Closure ADVANCED v20260803 | 239 | 126 | 90 |

This case isolates shared-constant propagation, deep inlining, local value
numbering, class dissolution, repeated-string aliasing, and dead function and
branch removal.

### `algorithms`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 201 | 167 | 136 |
| Closure ADVANCED v20260803 | 205 | 169 | 144 |

This case combines recursion with single-use multi-block CFG inlining, two
iterative algorithms, loop-phi coalescing, conditional returns, and integer
normalization.

### `data_model`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 95 | 88 | 75 |
| Closure ADVANCED v20260803 | 248 | 170 | 130 |

This case measures nested value structs, class construction, mutable methods,
field-index lowering, devirtualization, and scalar replacement.

### `higher_order`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 135 | 136 | 115 |
| Closure ADVANCED v20260803 | 141 | 140 | 124 |

This case measures map/filter fusion at emission time, reduce, block-arrow
inlining, callback effects, and captured immutable globals.

### `string_optimization`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 119 | 97 | 77 |
| Closure ADVANCED v20260803 | 265 | 132 | 97 |

This case measures compile-time string predicates and case conversion,
short-circuit folding, templates, and repeated long-string reuse.

### `modules`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 115 | 129 | 108 |
| Closure ADVANCED v20260803 | 122 | 134 | 108 |

This case gives each compiler three real source modules. It measures relative
import resolution, transitive linking, aliases, exported and private bindings,
cross-module multi-block inlining, purity-guided DCE, loop-phi copy ordering,
and complete removal of module syntax and unused exports.

### Corpus total

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 1,527 | 1,308 | 1,094 |
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
nine raw and gzip cells, smaller in eight Brotli cells, and tied in the ninth.
The suite is reproducible evidence for these features, not proof of universal
superiority over arbitrary JavaScript or future language features. The broader
application comparison, including ecosystem dependencies, hand-specialized
JavaScript, runtime samples, generated C, and native behavior checks, is in
[`../benchmarks/apps/RESULTS.md`](../benchmarks/apps/RESULTS.md).
