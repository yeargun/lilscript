# Types are not glue

Parent: [Language](README.md). Related: [boundaries](boundaries-escape.md), [aggregates](aggregates.md). Contract table: [`docs/language-v0.1.md`](../../language-v0.1.md) § Types.

## The TypeScript problem this language refuses

TypeScript checks, then **erases**. The leftover JavaScript still has:

- objects whose fields are string names a minifier must preserve or infer from JSDoc/`externs`;
- `any` / structural holes at npm and DOM boundaries;
- `enum` emit strategies (const object vs numeric) that are comments to the optimizer;
- `Promise<T>` with untyped rejection;
- module interop wrappers.

Closure `ADVANCED` then tries to recover what the type layer threw away. LilScript never throws it away. `Type` in `src/semantic.rs` drives lowering, escape, range/finite-value analysis, and emission.

## Each type has two representations

| LilScript | Meaning for optimization | JS | Native |
|---|---|---|---|
| `int` | signed i32; `|0` only when range analysis cannot prove safety | number | `i32` |
| `number` / `float` | IEEE binary64; no i32 wrapping | number | `f64` |
| `bool` | compact literals are a codec candidate (`!0`/`!1` vs `true`/`false`) | boolean | C11 `bool` |
| `string` | internable; pooling/packing are codec-scored | string | handle |
| `T[]` | homogeneous; callback methods snapshot length | array | handle |
| `Record<T>` | open keys are observable **data**; never mangled | null-prototype object when materialized | string map |
| `struct S` | positional; scalar-replace when `LocalOnly` | scalars / tuple / boundary object | C value record |
| `class C` | nominal ref; methods devirtualize; no vtables | dissolve or class at escape | pointer record |
| `extern class C` | host ABI names, never mangled, never `new` | existing host object | rejected |
| `enum E` | declaration-order discriminant; no metadata object | integer | `int32_t` |
| `T?` | `null` or `T`; JS keeps raw `null` | `T` or `null` | tagged optional |
| `A \| B` | JS erases after check; native tags only at union boundaries | member value | `LilScriptValue` at boundary |
| `func(...)->R` | direct calls specialize; unknown `CallValue` escapes | function | fn + env |
| `Task<T>` | native Promise, no LilScript scheduler | `Promise` | rejected |
| `Generator<T>` | direct `function*` | generator | rejected |
| `Regex` | constructor vs literal is a compression decision | `RegExp` | rejected |
| `JsValue` | explicit ops only; not `any` | host value | rejected |
| `void` | no value | none | none |

`auto` is inference at a declaration with an initializer. It is not a runtime type and not `any`.

## Splits that exist so the compressor can be aggressive

**`int` vs `number`.** Bitwise/shift stay `int`-only because JS itself i32-coerces them. Ordinary `int` multiply is `(a*b)|0`, never silently `Math.imul`. Source `Math.imul` stays exact low-32. Size-first may elide `|0` when `src/value_analysis.rs` proves the result is already signed i32. Performance-first keeps eager normalization. That is a **config-visible** ABI of the numeric type, not a comment.

**`struct` vs `Record<T>` vs `extern class`.** Closed layout, open data keys, and host names are three different things. Collapsing them into `object` would force property-name preservation everywhere. See [aggregates](aggregates.md).

Closed record observations can disappear only when a JS-only candidate proves and
substitutes their exact results. A record that remains materialized keeps the
null-prototype language contract; production does not infer ordinary-object safety
from already projected IR. See
[aggregate lowering](../compilation/aggregate-lowering.md#closed-record-observation-projection).

**`JsValue` vs untyped JS.** Implemented operations: `truthy()`, `isArray()`, `isObject()`, `length`, index, `for-in`, `is string|float|bool`. No arbitrary member dispatch. This is the hatch for genuinely dynamic APIs (jQuery’s public bags, JSON.parse). Operations that can invoke coercion hooks, proxy traps, or dynamic throws are observable and invalidate `pure`; the type checker does not erase that boundary into an apparently scalar expression. Overuse is the main size tax on the jQuery port — see [evidence](../evidence/jquery.md).

**Closed enums.** Exhaustive `match`, nominal (no implicit `int`), zero-based discriminant. Enables switch lowering and constant folding without an emit object.

## Generics

Type arguments are inferred and checked. JS erases them after checking. Native boxes at polymorphic boundaries (`LilScriptValue`). Polymorphic functions are not inlined until call-site types can be substituted. That is a size/compile-time tradeoff encoded in the optimizer, not a TS-style “leave it to the bundler”.

## Standard library as intrinsics

Array/string/map/set/typed-array/Math operations are `BuiltinCall` / `ControlFlowOp::Intrinsic`, not `obj["map"]`. Non-mutating typed operations are pure language operations; mutators carry precise receiver effects rather than an arbitrary host-call effect. The optimizer can fuse pipelines, snapshot lengths, and DCE unobserved mutation graphs. Adding a “just call JS” convenience method without an intrinsic is a glue regression.
