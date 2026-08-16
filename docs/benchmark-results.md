# LilScript v0.1 Bundle Benchmark

Historically measured on 2026-08-07 with LilScript release mode and Google Closure
Compiler `v20260803` at `ADVANCED` compilation level. That archived table used:

`https://repo.maven.apache.org/maven2/com/google/javascript/closure-compiler/v20260803/`

> These checked-in numbers predate both the runner's objective-specific artifact
> split and the shared canonical `lilscript-codec` gate. They are historical
> measurements only: none of the three columns is a current pass claim. The current
> runner pins and verifies Closure `v20260804`, builds and executes separate raw-,
> gzip-, and Brotli-objective artifacts,
> measures them with the statically linked scorer, and gates only matching metrics.
> Regenerate the complete table before citing current byte or pass totals.

## Results

### `v01`

| Compiler                   | Raw | Gzip-9 | Brotli-11 |
| -------------------------- | --: | -----: | --------: |
| LilScript                  |  90 |    107 |        83 |
| Closure ADVANCED v20260803 | 203 |    182 |       161 |

This case exercises class constructor/method devirtualization, scalar
replacement, array map callbacks, integer lowering, a counted loop, branch
folding, and template output.

### `conformance`

| Compiler                   | Raw | Gzip-9 | Brotli-11 |
| -------------------------- | --: | -----: | --------: |
| LilScript                  | 266 |    207 |       178 |
| Closure ADVANCED v20260803 | 378 |    258 |       214 |

This case exercises mutable array operations, filter/reduce/forEach callbacks,
string predicates, case conversion, templates, and constant propagation.

### `full_conformance`

| Compiler                   | Raw | Gzip-9 | Brotli-11 |
| -------------------------- | --: | -----: | --------: |
| LilScript                  | 323 |    259 |       213 |
| Closure ADVANCED v20260803 | 520 |    355 |       296 |

This broader case combines structs, mutable classes, direct functions, global
and local closures, while/for control flow, break/continue, short-circuit
logic, compound arithmetic, arrays, strings, and templates.

### `optimizer_stress`

| Compiler                   | Raw | Gzip-9 | Brotli-11 |
| -------------------------- | --: | -----: | --------: |
| LilScript                  | 128 |     87 |        68 |
| Closure ADVANCED v20260803 | 239 |    126 |        90 |

This case isolates shared-constant propagation, deep inlining, local value
numbering, class dissolution, repeated-string aliasing, and dead function and
branch removal.

### `algorithms`

| Compiler                   | Raw | Gzip-9 | Brotli-11 |
| -------------------------- | --: | -----: | --------: |
| LilScript                  | 190 |    156 |       128 |
| Closure ADVANCED v20260803 | 196 |    161 |       135 |

This case combines recursion with single-use multi-block CFG inlining, two
iterative algorithms, loop-phi coalescing, conditional returns, and integer
normalization. Scalar and tuple parallel-copy schedules are both emitted as
bounded candidates; Brotli-11 selects the scalar schedule for this case.

### `data_model`

| Compiler                   | Raw | Gzip-9 | Brotli-11 |
| -------------------------- | --: | -----: | --------: |
| LilScript                  |  85 |     83 |        67 |
| Closure ADVANCED v20260803 | 239 |    164 |       149 |

This case measures nested value structs, class construction, mutable methods,
field-index lowering, devirtualization, and scalar replacement.

### `higher_order`

| Compiler                   | Raw | Gzip-9 | Brotli-11 |
| -------------------------- | --: | -----: | --------: |
| LilScript                  | 126 |    127 |       107 |
| Closure ADVANCED v20260803 | 132 |    133 |       111 |

This case measures map/filter fusion at emission time, reduce, block-arrow
inlining, callback effects, and captured immutable globals.

### `string_optimization`

| Compiler                   | Raw | Gzip-9 | Brotli-11 |
| -------------------------- | --: | -----: | --------: |
| LilScript                  | 119 |     97 |        65 |
| Closure ADVANCED v20260803 | 265 |    132 |        97 |

This case measures compile-time string predicates and case conversion,
short-circuit folding, templates, and repeated long-string reuse.

### `alias_optimization`

| Compiler                   | Raw | Gzip-9 | Brotli-11 |
| -------------------------- | --: | -----: | --------: |
| LilScript                  |  29 |     40 |        33 |
| Closure ADVANCED v20260803 | 161 |    133 |       114 |

This case creates and mutates local arrays, maps, and sets whose states are
never observed, alongside two observable results. Allocation-root alias
analysis removes the complete unobserved mutation graph while preserving the
observable collection behavior.

### `modules`

| Compiler                   | Raw | Gzip-9 | Brotli-11 |
| -------------------------- | --: | -----: | --------: |
| LilScript                  | 101 |    117 |        90 |
| Closure ADVANCED v20260803 | 117 |    127 |       104 |

This case gives each compiler three real source modules. It measures relative
import resolution, transitive linking, aliases, exported and private bindings,
cross-module multi-block inlining, purity-guided DCE, loop-phi copy ordering,
and complete removal of module syntax and unused exports.

### Corpus total

| Compiler                   |   Raw | Gzip-9 | Brotli-11 |
| -------------------------- | ----: | -----: | --------: |
| LilScript                  | 1,457 |  1,280 |     1,032 |
| Closure ADVANCED v20260803 | 2,450 |  1,771 |     1,471 |

## Finite-value pass ablation

All ablation sections below compare enabled/disabled compiler variants, not
LilScript against JavaScript. Where one configured artifact is checked across
multiple metrics, that deliberately stricter same-artifact rule isolates the
pass and must not be read as a general cross-objective superiority claim.

This separate optimizer ablation uses
`tests/cases/interprocedural_finite_values.lil`. Inlining and scalar replacement
are disabled in both variants; only `finite_value_propagation` changes. Both
artifacts must execute the checked output contract before sizes are accepted.

| Variant                | Raw | Gzip-9 | Brotli-11 |
| ---------------------- | --: | -----: | --------: |
| Finite values enabled  | 143 |    108 |        77 |
| Finite values disabled | 216 |    155 |       118 |

Run it with `node benchmarks/finite-values/run.mjs`.

## Identical private-function folding ablation

`benchmarks/function-folding/fixture.lil` keeps two directly called private
functions identical after specialization and inlining decisions. Both builds
disable inlining and execute the same `95660` output contract. The enabled
build redirects calls to one implementation; the disabled build retains both.
Exported and address-taken function identities are excluded from the pass.

| Variant          | Raw | Gzip-9 | Brotli-11 |
| ---------------- | --: | -----: | --------: |
| Folding enabled  | 123 |    129 |       105 |
| Folding disabled | 177 |    139 |       111 |

Run it with `node benchmarks/function-folding/run.mjs`. This is a 54-byte raw,
10-byte gzip, and 6-byte Brotli win on the isolated duplicate-body workload.

## Private-function subsumption ablation

`benchmarks/function-subsumption/fixture.lil` contains specialized scalar and
direct-call implementations plus broader scalar and higher-order
implementations that can reproduce them. The proof specializes a temporary IR
clone, canonicalizes typed constants, known callbacks, and local metadata, and
requires exact normalized SSA/CFG equality before redirecting calls with
explicit arguments. Both builds disable inlining and execute the same
`3940336` output contract. Exported and address-taken identities remain
ineligible.

| Variant              | Raw | Gzip-9 | Brotli-11 |
| -------------------- | --: | -----: | --------: |
| Subsumption enabled  | 351 |    201 |       172 |
| Subsumption disabled | 445 |    217 |       179 |

Run it with `node benchmarks/function-subsumption/run.mjs`. This isolated case
saves 94 raw, 16 gzip, and 7 Brotli bytes. The pass is a size-first IR candidate,
not an unconditional rewrite; the complete untouched artifact competes under
the configured codec.

## Function declaration layout ablation

`benchmarks/function-layout/fixture.lil` interleaves two pairs of structurally
similar functions. The candidate emitter groups declarations by repeated
eight-byte token runs; a maximum-weight dynamic program handles this four-node
case. Both artifacts execute the same `-1393288640` output contract. The raw
length is unchanged, and the complete Brotli artifact selects whether source or
similarity order survives.

| Variant               |   Raw | Gzip-9 | Brotli-11 |
| --------------------- | ----: | -----: | --------: |
| Layout search enabled | 1,133 |    454 |       362 |
| Source order          | 1,133 |    460 |       369 |

Run it with `node benchmarks/function-layout/run.mjs`. This is evidence for the
checked fixture, not a claim that reordering always helps; source order remains
an exact-scored candidate.

## Profile-guided higher-order specialization ablation

`benchmarks/profile-guided/fixture.lil` passes one known callback through a
shared higher-order function inside a 10,000-iteration loop. Both builds disable
ordinary inlining, execute the same `50005000` output contract, and use the
same performance-shape analysis. The enabled build adds a versioned hot-loop
counter and permits call-site specialization; the disabled build omits that
specialization. Final artifacts are measured only after execution succeeds.

| Variant                       | Raw | Gzip-9 | Brotli-11 |
| ----------------------------- | --: | -----: | --------: |
| Profile-guided specialization | 107 |    104 |        77 |
| Static higher-order call      | 111 |    107 |        80 |

Run it with `node benchmarks/profile-guided/run.mjs`. This is a focused
four-byte raw, three-byte gzip, and three-byte Brotli result, not evidence that
profiles improve every program.

## Inlining IR pass ablation

`tests/cases/ir_inlining_variant.lil` holds the final emission search and every
optimizer setting constant while omitting only `ir-inlining-variants`. Both
artifacts execute the JavaScript/native contract before measurement.

| Variant                       | Raw | Gzip-9 | Brotli-11 |
| ----------------------------- | --: | -----: | --------: |
| Inlining IR variants enabled  | 219 |    113 |        83 |
| Inlining IR variants disabled | 267 |    144 |       109 |

Run it with `node benchmarks/ir-variants/run.mjs`. The complete-library lab also
shows an independent production-sized win: Emotion hash improves from
`866/535/456` to `816/532/452` raw/gzip-9/Brotli-11.

## Closure factory IR ablation

`tests/cases/closure_factory_variant.lil` creates twelve closures from one
factory with distinct capture signatures. Both builds retain ordinary inlining,
the fully outlined IR candidate, and identical final-emission search; the
disabled build omits only `ir-closure-factory-variants`. Every JavaScript, C,
and native result must match before size measurement.

| Variant                      | Raw | Gzip-9 | Brotli-11 |
| ---------------------------- | --: | -----: | --------: |
| Factory IR variants enabled  | 648 |    255 |       176 |
| Factory IR variants disabled | 677 |    244 |       177 |

The project default optimizes Brotli-11, so this ablation gates only the Brotli
win. Raw and gzip are diagnostics; the 11-byte gzip loss is reported rather
than treated as a failure. A gzip-configured build would score the fully
outlined baseline independently rather than forcing the factory-preserving
candidate.

Run it with `node benchmarks/closure-factory-variants/run.mjs`.

## Loop spelling ablation

The checked-in order-sensitive control-flow fixture places condition-only loops
before update-bearing loops. Both builds use the same optimizer and emitter
policy; the disabled build omits only `loop-spelling-selection`, and both must
print `137`. This isolates the case where a frequency heuristic cannot see
future token context. The Brotli objective accepts a large raw-size trade only
because both compressed artifacts are one byte smaller.

| Variant                 | Raw | Gzip-9 | Brotli-11 |
| ----------------------- | --: | -----: | --------: |
| Codec-selected spelling | 527 |    243 |       178 |
| Frequency heuristic     | 527 |    243 |       178 |

At the historical checkpoint, the broader structural search reached the same
artifact from both sides.
This fixture is retained as a behavior and Brotli-objective non-regression gate;
raw and gzip are diagnostics. It is no longer claimed as an isolated size win.

Run it with `node benchmarks/loop-spelling/run.mjs`.

The historical checkpoint emitted `1734/831/733` for the complete MurmurHash port in
both modes. It remains part of the complete-library corpus, but that pre-canonical
byte-identical result is not an optimization win or a current byte claim.

## Mutation spelling ablation

The complete Levenshtein port holds every optimizer and emitter choice
constant while omitting only `mutation-spelling-selection`. Prefix or postfix
increment is considered only for a one-use SSA add feeding its own phi and
only when range analysis proves no signed-i32 coercion is required. Both
artifacts execute the same package contract before measurement.

| Variant                    |  Raw | Gzip-9 | Brotli-11 |
| -------------------------- | ---: | -----: | --------: |
| Mutation spelling selected | 1582 |    900 |       777 |
| Assignment spelling only   | 1583 |    899 |       780 |

The default Brotli objective selects a one-byte raw and three-byte Brotli win
while spending one gzip byte. Assignment spelling remains in the candidate set
when gzip is the configured cost model.

Run it with `node benchmarks/mutation-spelling/run.mjs`.

## Method

Each LilScript workload has a behaviorally equivalent JavaScript input under
`benchmarks/`. Integer operations in the JavaScript references explicitly use
32-bit normalization so Closure is measured against the same semantics.

For every case, `benchmarks/run.sh`:

1. builds LilScript in Cargo release mode;
2. compiles LilScript independently with raw, gzip, and Brotli cost models;
3. invokes Closure with `--compilation_level ADVANCED`;
4. executes both outputs under Node and rejects any stdout difference;
5. measures the exact emitted UTF-8 bytes without rewriting the artifact;
6. batches every exact artifact through `lilscript-codec`, using statically bundled
   upstream stock zlib C 1.3.1 at gzip level 9 with deterministic `mtime: 0`;
7. measures Brotli through that same scorer's bundled official Google Brotli C
   1.1.0 encoder in generic mode, quality 11, `lgwin = 22`.

The scorer records both library and Cargo package identities. Compiler candidate
ranking and publication gates call the same measurement functions. Node's built-in
codec sizes may be reported diagnostically, but they do not select a winner; even
Node 24's patched zlib build produced different lengths from upstream stock 1.3.1.

Run the complete comparison with:

```sh
benchmarks/run.sh
```

The historical artifacts happened to be smaller in all 30 displayed cells under the
then-current measurement. That observation is not a current gate result: a fresh
three-objective run through `lilscript-codec` is required before any raw, gzip, or
Brotli pass total is quoted. The suite is scoped evidence for these features, not
proof of universal
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
The prose tables there may predate the canonical scorer/objective-artifact contract;
their historical five-of-six Brotli observation, including the Emotion loss, is not
a current library pass claim until its generated report is rebuilt. No universal
size claim follows.
