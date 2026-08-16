# Numerics and value semantics

Parent: [language](README.md). Contract: [types](../../language-v0.1.md#types).
Compiler anchors: `Type` checking in `src/semantic.rs`, integer/finite analyses in
`src/value_analysis.rs`, and constant rendering in `src/codegen_ir_js.rs`.

LilScript distinguishes values whose JavaScript spelling looks similar because the
distinction supplies optimization proofs.

| Surface type | Required semantics | Compression consequence |
|---|---|---|
| `int` | signed i32; wrapping add/sub/negation/bitwise, shifts mask by 31 | range analysis may remove redundant `|0`, but never changes the result |
| `number` / `float` | IEEE-754 binary64 | avoids artificial i32 normalization on ordinary web numerics |
| `bool` | exactly `true`/`false` | branch and finite-value propagation can erase tags |
| `string` | JS-compatible string operations; UTF-16-oriented indexing contract | literals, templates, pooling, quote style, and repeated contexts are searchable |
| `enum E` | nominal closed discriminant in declaration order | emits an integer, not a metadata object |
| `T?` | raw `T` or `null` | narrowing/nullish branches need no wrapper in JS |
| `A \| B` | one statically declared member | JS erases the union; native tags only at union boundaries |
| `Symbol` | unique identity | cannot be folded by description |
| `JsValue` | explicit dynamic JS boundary | blocks type-dependent rewrites and is rejected by native |

Ordinary integer multiplication follows JavaScript binary64 multiplication followed
by signed-i32 normalization. `Math.imul` is a separate exact-low-32-bit intrinsic;
the optimizer does not substitute one for the other. Integer division truncates
toward zero, and division or remainder by zero returns `0` on every backend. An `int`
widens to `number`; other conversions are explicit.

Narrowing is flow-sensitive for null checks and supported `is` categories. Assignment
invalidates a fact. A union guard is rejected when two members share the same runtime
category, such as `int | float`, because JavaScript cannot distinguish them without
inventing reflection.

## What config may change

`safe-integer-coercion-elision`, compact booleans, numeric pooling, and expression
search may change spelling only after legality is proved. `priority` can retain more
eager normalization for runtime shape, but it cannot change overflow, `NaN`, `-0`,
evaluation order, or conversion semantics.

## Evidence expectations

Own edge cases for i32 extrema, imprecise large products vs `Math.imul`, zero
division/remainder, every shift family, `NaN`/`-0`/infinity, nullable falsy values,
union narrowing, enum exhaustiveness, UTF-16 strings, and Symbol identity. Portable
cases must agree in JS, generated C, and native execution.
