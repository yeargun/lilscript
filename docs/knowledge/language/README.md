# Language

LilScript is an independent statically typed language. `.lil` is never parsed as JavaScript or TypeScript. JavaScript and native object code are backends of one typed whole-program IR.

Parent: [Mission](../mission.md). Contract: [`docs/language-v0.1.md`](../../language-v0.1.md).

## Why the language looks this way

Every surface construct is judged by whether it gives the compiler a **proof** it can use for DCE, layout, mangling, or codec search — without introducing TypeScript-style glue (erased types, implicit `any`, structural holes at npm boundaries, runtime wrappers).

| Construct | Proof it gives the compiler |
|---|---|
| Nominal `struct` / `class` | Field indexes; scalar replacement; no property names internally |
| Closed `object` | ABI keys on one identity; private method bodies nest/mangle |
| `Record<T>` | Open string keys are **data**, never mangled |
| Closed `enum` | Integer discriminant, no metadata object |
| `extern` / `extern class` | Exact host ABI; everything else may dissolve |
| `T?` and `A \| B` | Narrowing without wrappers; native tags only at boundaries |
| `pure` / inferred effects | Unused calls are removable |
| Static imports | Cross-file SSA; module syntax is not emitted |
| `import("./x")` | Typed lazy chunk; lazy modules are init-free |
| `int` vs `number` | Proven-safe `|0` elision vs binary64 hot paths |
| `JsValue` | Narrow dynamic hatch; native rejects it |

## Pages

- [Types are not glue](types-not-glue.md) — vs TypeScript; ABI per type
- [Numerics and value semantics](numerics-values.md) — i32 vs binary64, unions,
  nullability, enums, strings, identity
- [Functions, closures, and generics](functions-closures-generics.md) — callable ABI,
  captures, defaults, generic erasure/boxing
- [Control flow and exceptions](control-flow-errors.md) — evaluation order, phis,
  nullish flow, structured completion
- [Collections and typed intrinsics](collections-intrinsics.md) — arrays, records,
  maps/sets, buffers/views, strings
- [Async, generators, and regex](async-generators-regex.md) — direct JS platform
  lowering and native rejection
- [Closed world](closed-world.md) — compilation unit, exports as accessibility vs retention
- [Packages, exports, and ABI](packages-exports-abi.md) — lock/resolution and public
  boundaries
- [Boundaries and escape](boundaries-escape.md) — `extern`, escape states, invalidation
- [Aggregates](aggregates.md) — struct / class / record; named vs positional ABI
- [Effects and purity](effects-purity.md) — inference, `pure`, host contracts
- [Modules, lazy loading, progressive enhancement](modules-lazy.md) — graph, chunks, PE
- [JavaScript vs native](js-vs-native.md) — shared IR, rejected features

## Compilation consequence

Language design is upstream of [compilation](../compilation/README.md). If a feature cannot be checked, escaped, and represented, the compressor cannot legally rewrite it. Prefer a smaller, explicit surface over a JS convenience that would force conservative lowering.

Config that changes **language-visible ABI** (not just spelling):

- `javascript.public_aggregate_abi` — named fields vs positional handles at the reusable JS boundary
- `javascript.aggregate_layout` — named objects vs positional arrays for instances
- `javascript.function_spelling` — constructible functions vs public arrows
- `[mangle] exports` / `properties` — public names vs owned fields
- `[bundle] mode` — one artifact vs real ESM chunks vs preserve-modules
