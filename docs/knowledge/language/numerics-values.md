# Numerics and value semantics

Parent: [language](README.md). Contract: [types](../../language-v0.1.md#types).
Compiler anchors: `Type` checking in `src/check.rs`, primitive semantics shared by
both targets and the interpreter in `src/primitive.rs`, primitive domains in
`src/program/raw_domains.rs`, and integer normalization in
`src/program/javascript_int32.rs`.

LilScript distinguishes values whose JavaScript spelling looks similar because the
distinction supplies optimization proofs.

Owner decision [D1](../../future-architecture.md#appendix-a-owner-decisions-d1d5-verbatim-from-the-2026-09-18-design-git-show-d362338fdocscompiler-designmd)
selects **value structs with explicit mutable references**. Caller-mutating helpers
require an explicit `ref` to the original place
([language contract](../../language-v0.1.md#mutable-references-ref)); flattening
cannot change assignment semantics. Nested/generic/nullable copies, reference
fields and capture/lifetime details are specified and tested in
[step 002](../../migration/record-2026-09.md#002-language-and-public-boundaries).
The architecture proposes revising D1 to immutable value structs with functional
update (L6, plan M10.9, needs an owner ruling).

| Surface type | Required semantics | Compression consequence |
|---|---|---|
| `int` | signed i32; wrapping add/sub/negation/bitwise, shifts mask by 31 | the compiler may drop a proven-redundant generated `\|0`; a live source `value \| 0` stays explicit |
| `number` / `float` | IEEE-754 binary64 | avoids artificial i32 normalization on ordinary web numerics |
| `bool` | exactly `true`/`false` | branch and finite-value propagation can erase tags |
| `string` | JS-compatible string operations over UTF-16 code units, on both targets | literals, templates, pooling, quote style, and repeated contexts are searchable |
| `enum E` | nominal closed discriminant in declaration order | emits an integer, not a metadata object |
| `T?` | raw `T` or `null` | narrowing/nullish branches need no wrapper in JS |
| `A \| B` | one statically declared member | JS erases the union; native tags only at union boundaries |
| `Symbol` | unique identity | cannot be folded by description |
| `JsValue` | explicit dynamic JS boundary | blocks type-dependent rewrites and is rejected by native |

Ordinary integer multiplication follows JavaScript binary64 multiplication followed
by signed-i32 normalization. `Math.imul` is a separate exact-low-32-bit intrinsic;
the optimizer does not substitute one for the other. Integer division truncates
toward zero, and division or remainder by zero returns `0` on every target. An `int`
widens to `number`; other conversions are explicit.

Narrowing is flow-sensitive for null checks and supported `is` categories. Assignment
invalidates a fact. A union guard is rejected when two members share the same runtime
category, such as `int | float`, because JavaScript cannot distinguish them without
inventing reflection.

## What config may change

The compiler drops a generated `|0` only where a value fact proves it redundant
(plan M6.4 moves those proofs onto one value lattice). Removing every `|0` was
measured at only −4 to −79 Brotli, so `|0` is a raw-size lever more than a Brotli
one. The old `priority` values other than `size-first` and
`javascript.integer_coercions` are retired. Spelling choices may change only after
legality is proved; no configuration can change overflow, `NaN`, `-0`, evaluation
order, or conversion semantics.

## Evidence expectations

Own edge cases for i32 extrema, imprecise large products vs `Math.imul`, zero
division/remainder, every shift family, `NaN`/`-0`/infinity, nullable falsy values,
union narrowing, enum exhaustiveness, UTF-16 strings, and Symbol identity. Portable
cases must agree in JS, generated C, and native execution.
