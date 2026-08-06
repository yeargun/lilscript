# LilScript v0.1 Bundle Benchmark

Measured on 2026-08-06 with LilScript release mode and Google Closure Compiler
`v20260803` at `ADVANCED` compilation level. The Closure version is pinned in
`benchmarks/run.sh` and downloaded from Maven Central:

`https://repo.maven.apache.org/maven2/com/google/javascript/closure-compiler/v20260803/`

## Results

### `v01`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 91 | 107 | 84 |
| Closure ADVANCED v20260803 | 203 | 182 | 161 |

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
| LilScript | 327 | 263 | 218 |
| Closure ADVANCED v20260803 | 520 | 355 | 296 |

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
| LilScript | 191 | 159 | 134 |
| Closure ADVANCED v20260803 | 196 | 161 | 135 |

This case combines recursion with single-use multi-block CFG inlining, two
iterative algorithms, loop-phi coalescing, conditional returns, and integer
normalization. Scalar and tuple parallel-copy schedules are both emitted as
bounded candidates; Brotli-11 selects the scalar schedule for this case.

### `data_model`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 85 | 82 | 67 |
| Closure ADVANCED v20260803 | 239 | 164 | 149 |

This case measures nested value structs, class construction, mutable methods,
field-index lowering, devirtualization, and scalar replacement.

### `higher_order`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 126 | 128 | 110 |
| Closure ADVANCED v20260803 | 132 | 133 | 111 |

This case measures map/filter fusion at emission time, reduce, block-arrow
inlining, callback effects, and captured immutable globals.

### `string_optimization`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 119 | 97 | 65 |
| Closure ADVANCED v20260803 | 265 | 132 | 97 |

This case measures compile-time string predicates and case conversion,
short-circuit folding, templates, and repeated long-string reuse.

### `alias_optimization`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 29 | 40 | 33 |
| Closure ADVANCED v20260803 | 161 | 133 | 114 |

This case creates and mutates local arrays, maps, and sets whose states are
never observed, alongside two observable results. Allocation-root alias
analysis removes the complete unobserved mutation graph while preserving the
observable collection behavior.

### `modules`

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 102 | 117 | 92 |
| Closure ADVANCED v20260803 | 117 | 127 | 104 |

This case gives each compiler three real source modules. It measures relative
import resolution, transitive linking, aliases, exported and private bindings,
cross-module multi-block inlining, purity-guided DCE, loop-phi copy ordering,
and complete removal of module syntax and unused exports.

### Corpus total

| Compiler | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| LilScript | 1,466 | 1,287 | 1,061 |
| Closure ADVANCED v20260803 | 2,450 | 1,771 | 1,471 |

## Finite-value pass ablation

This separate optimizer ablation uses
`tests/cases/interprocedural_finite_values.lil`. Inlining and scalar replacement
are disabled in both variants; only `finite_value_propagation` changes. Both
artifacts must execute the checked output contract before sizes are accepted.

| Variant | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Finite values enabled | 143 | 108 | 77 |
| Finite values disabled | 214 | 157 | 121 |

Run it with `node benchmarks/finite-values/run.mjs`.

## Inlining IR pass ablation

`tests/cases/ir_inlining_variant.lil` holds the final emission search and every
optimizer setting constant while omitting only `ir-inlining-variants`. Both
artifacts execute the JavaScript/native contract before measurement.

| Variant | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Inlining IR variants enabled | 221 | 114 | 89 |
| Inlining IR variants disabled | 283 | 152 | 108 |

Run it with `node benchmarks/ir-variants/run.mjs`. The complete-library lab also
shows an independent production-sized win: Emotion hash improves from
`866/542/463` to `816/538/456` raw/gzip-9/Brotli-11.

## Closure factory IR ablation

`tests/cases/closure_factory_variant.lil` creates twelve closures from one
factory with distinct capture signatures. Both builds retain ordinary inlining,
the fully outlined IR candidate, and identical final-emission search; the
disabled build omits only `ir-closure-factory-variants`. Every JavaScript, C,
and native result must match before size measurement.

| Variant | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Factory IR variants enabled | 627 | 243 | 172 |
| Factory IR variants disabled | 677 | 244 | 173 |

Run it with `node benchmarks/closure-factory-variants/run.mjs`.

## Loop spelling ablation

The complete `murmurhash-js` LilScript port holds optimizer and emitter policy
constant while omitting only `loop-spelling-selection`. Both outputs execute
the package contract before measurement. `while(condition)` and
`for(;condition;)` have equal raw length, so the win comes from exact Brotli
token context rather than a source-length heuristic.

| Variant | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Codec-selected spelling | 1741 | 840 | 734 |
| Frequency heuristic | 1741 | 840 | 740 |

Run it with `node benchmarks/loop-spelling/run.mjs`.

## Mutation spelling ablation

The complete Emotion hash port holds every optimizer and emitter choice
constant while omitting only `mutation-spelling-selection`. Prefix or postfix
increment is considered only for a one-use SSA add feeding its own phi and
only when range analysis proves no signed-i32 coercion is required. Both
artifacts execute the same package contract before measurement.

| Variant | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Mutation spelling selected | 814 | 536 | 455 |
| Assignment spelling only | 816 | 538 | 456 |

Run it with `node benchmarks/mutation-spelling/run.mjs`.

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
30 raw, gzip, and Brotli cells.
The suite is reproducible evidence for these features, not proof of universal
superiority over arbitrary JavaScript or future language features. The broader
application comparison, including ecosystem dependencies, hand-specialized
JavaScript, runtime samples, generated C, and native behavior checks, is in
[`../benchmarks/apps/RESULTS.md`](../benchmarks/apps/RESULTS.md).

## Complete library ports

The separate [`../benchmarks/libraries`](../benchmarks/libraries) project uses
installed, version-pinned npm packages rather than matching-scope synthetic
implementations. It currently covers the complete documented callable root
entrypoints of `@motionone/easing@10.18.0`, `clamp@1.0.1`, `lerp@1.0.3`,
`string-hash@1.1.3`, `js-levenshtein@1.1.6`, `@emotion/hash@0.9.2`, and
`murmurhash-js@1.0.0` for their typed input domains.

Each case must match through Vite, an esbuild-to-Closure ADVANCED pipeline,
LilScript JavaScript, emitted C, and a native executable. Translated upstream
assertions and dense differential API grids run after the app contracts. The
generated tables are in
[`../benchmarks/libraries/RESULTS.md`](../benchmarks/libraries/RESULTS.md).
They show smaller LilScript Brotli payloads in five of six complete apps,
including Motion easing, but a larger payload for Emotion hash; no universal
size claim follows.
