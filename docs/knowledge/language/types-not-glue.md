# Types are not glue

Parent: [Language](README.md). Related: [boundaries](boundaries-escape.md), [aggregates](aggregates.md), [compressor surface](compressor-surface.md). Contract table: [`docs/language-v0.1.md`](../../language-v0.1.md) § Types.

## The TypeScript problem this language refuses

TypeScript checks, then **erases**. The leftover JavaScript still has:

- objects whose fields are string names a minifier must preserve or infer from JSDoc/`externs`;
- `any` / structural holes at npm and DOM boundaries;
- `enum` emit strategies (const object vs numeric) that are comments to the optimizer;
- `Promise<T>` with untyped rejection;
- module interop wrappers.

Closure `ADVANCED` then tries to recover what the type layer threw away. LilScript never throws it away. `Type` in `src/check.rs` drives elaboration into the Program IR, formation, and both targets' representations. Architecture law L13 states the other half: a typed form must never cost more bytes than its untyped equivalent, or authors learn to hide facts ([future architecture §12](../../future-architecture.md#12-the-language-designed-for-size)).

## Each type has two representations

| LilScript | Meaning for optimization | JS | Native |
|---|---|---|---|
| `int` | signed i32; generated `\|0` only when range analysis cannot prove safety; source `value \| 0` stays explicit while live | number | `i32` |
| `number` / `float` | IEEE binary64; no i32 wrapping | number | `f64` |
| `bool` | compact literals are a codec candidate (`!0`/`!1` vs `true`/`false`) | boolean | C11 `bool` |
| `string` | UTF-16 code units; pooling/packing are codec-scored | string | length + UTF-16 code units |
| `T[]` | homogeneous; callback methods snapshot length | array | handle |
| `Record<T>` | open keys are observable **data**; never mangled | null-prototype object when materialized | string map |
| `struct S` | positional; scalar-replace when local | scalars / tuple / boundary object | C value record |
| `class C` | nominal ref; methods devirtualize; no vtables | dissolve or class at escape | pointer record |
| `extern class C` | host ABI names, never renamed; never `new` | existing host object | rejected |
| `enum E` | declaration-order discriminant; no metadata object | integer | `int32_t` |
| `T?` | `null` or `T`; JS keeps raw `null` | `T` or `null` | tagged optional |
| `A \| B` | JS erases after check; native tags only at union boundaries | member value | `LilScriptValue` at boundary |
| `func(...)->R` | direct calls specialize; an unknown value call escapes | function | fn + env |
| `Task<T>` | native Promise, no LilScript scheduler | `Promise` | rejected |
| `Generator<T>` | direct `function*` | generator | rejected |
| `Regex` | constructor vs literal is a compression decision | `RegExp` | rejected |
| `JsValue` | explicit ops only; not `any` | host value | rejected |
| `void` | no value | none | none |

`auto` is inference at a declaration with an initializer. It is not a runtime type and not `any`.

## Splits that exist so the compressor can be aggressive

**`int` vs `number`.** Bitwise/shift stay `int`-only because JS itself i32-coerces them. Ordinary `int` multiply is `(a*b)|0`, never silently `Math.imul`. Source `Math.imul` stays exact low-32. The compiler may drop compiler-generated, proven-redundant normalization; a live source-written `value | 0` remains explicit.

**`struct` vs `Record<T>` vs `extern class`.** Closed layout, open data keys, and host names are three different things. Collapsing them into `object` would force property-name preservation everywhere. See [aggregates](aggregates.md).

`Record<T>` materializes as **null-prototype**. The v0.1 type table’s “plain object”
wording is the ordinary-`{}` hole jQuery hits. Do not infer `Object.prototype`
backing for a record.

A record allocation can disappear only when the compiler proves it never escapes
and replaces it with scalars (the record family today). A record that remains
materialized keeps the null-prototype language contract.

**`JsValue` vs untyped JS.** Implemented operations: `truthy()`, `isArray()`, `isObject()`, `length`, index, `for-in`, `is string|float|bool`. No arbitrary member dispatch. It is meant as the hatch for genuinely dynamic APIs (jQuery’s public bags, JSON.parse). In practice it is the main road: the 27 ports hold 42,936 `JsValue` mentions and 38,354 `JS.*` calls (architecture §12), because typed forms cost bytes on today's compiler; plan phase M10 removes that cost. Operations that can invoke coercion hooks, proxy traps, or dynamic throws are observable and invalidate `pure`; the type checker does not erase that boundary into an apparently scalar expression. Overuse is the main size tax on JS-shaped ports — see [compressor surface](compressor-surface.md) and [jQuery](../evidence/jquery.md).

**Closed enums.** Exhaustive `match`, nominal (no implicit `int`), zero-based discriminant. Enables switch lowering and constant folding without an emit object.

## Generics

Type arguments are inferred and checked. JS erases them after checking. Native boxes at polymorphic boundaries (`LilScriptValue`). Polymorphic functions are not inlined until call-site types can be substituted. That is a size/compile-time tradeoff encoded in the compiler, not a TS-style “leave it to the bundler”.

## Standard library as intrinsics

Array/string/map/set/typed-array/Math operations are builtin calls and intrinsics with a checked identity (plan M4.6 merges them into one operation catalog), not `obj["map"]`. Non-mutating typed operations are pure language operations; mutators carry precise receiver effects rather than an arbitrary host-call effect. The compiler can snapshot lengths and remove unobserved mutation graphs. Adding a “just call JS” convenience method without an intrinsic is a glue regression.
