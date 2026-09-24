# Language

LilScript is an independent statically typed language. `.lil` is never parsed as JavaScript or TypeScript. JavaScript and native object code are targets of one typed whole-program IR.

Parent: [Mission](../mission.md). Contract: [`docs/language-v0.1.md`](../../language-v0.1.md).
Durable rationale: [typed proofs, not glue](../decisions/typed-proofs-not-glue.md),
[representation and ABI](../decisions/representation-and-abi.md).

## Why the language looks this way

Every surface construct is judged by whether it gives the compiler a **proof** it can use for DCE, layout, mangling, or codec search — without introducing TypeScript-style glue (erased types, implicit `any`, structural holes at npm boundaries, runtime wrappers).

| Construct | Proof it gives the compiler |
|---|---|
| Nominal `struct` / `class` | Field indexes; scalar replacement; no property names internally |
| Named `object` | ABI keys on one singleton identity; private method bodies nest/mangle (does not compile today; plan M10.10) |
| `object { ... }` | Ordinary-prototype JS dictionary; inherited hooks remain observable |
| `Record<T>` | Open string keys are **data**, never mangled |
| Closed `enum` + `match` | Integer discriminant, no metadata object; exhaustive DCE of arms |
| `extern` / `extern class` | Exact host ABI by default; everything else may dissolve |
| `T?` and `A \| B` | Narrowing without wrappers; native tags only at boundaries |
| `pure` / inferred effects | Unused calls are removable (from plan M6–M7) |
| Static imports | One program across files; module syntax is not emitted |
| `import("./x")` | Typed lazy chunk; lazy modules are init-free |
| `int` vs `number` | Proven-safe `|0` elision vs binary64 hot paths |
| `JsValue` | Dynamic hatch; native rejects it. Meant to be narrow, used everywhere today (see [types are not glue](types-not-glue.md)) |

## Pages

### Types and values

- [Types are not glue](types-not-glue.md)
- [Compressor surface](compressor-surface.md) — write proofs so Terser/Oxc/Closure can lose
- [Numerics and value semantics](numerics-values.md)

### Programs

- [Functions, closures, and generics](functions-closures-generics.md)
- [Control flow and exceptions](control-flow-errors.md)
- [Effects and purity](effects-purity.md)

### Data

- [Aggregates](aggregates.md) — struct / class / record
- [Collections and typed intrinsics](collections-intrinsics.md)
- [Async, generators, and regex](async-generators-regex.md)

### World

- [Closed world](closed-world.md)
- [Packages, exports, and ABI](packages-exports-abi.md)
- [Boundaries and escape](boundaries-escape.md)
- [Modules, lazy loading, progressive enhancement](modules-lazy.md)
- [JavaScript vs native](js-vs-native.md)

## Compilation consequence

Language design is upstream of [compilation](../compilation/README.md). If a feature cannot be checked, escaped, and represented, the compressor cannot legally rewrite it. Prefer a smaller, explicit surface over a JS convenience that would force conservative lowering. Which legal representations are rules and which are codec-judged choices is the architecture's [§8–§9](../../future-architecture.md#8-edits-and-the-rule-scheduler); the language additions designed for size are [§12](../../future-architecture.md#12-the-language-designed-for-size). How to write a port so Terser/Oxc/Closure can lose: [compressor surface](compressor-surface.md).

Config that changes **language-visible ABI** is contract, not a codec preference
(architecture law L12):

- the target: `--target js-module` keeps the root module's exports as the API
- `keep_published_function_names` / `keep_function_names` — reflected `name`
- `[mangle] preserve_properties` — property names callers read in code the compiler never sees
- `[bundle] mode` — artifact/module layout, distinct from application vs library world

`public_aggregate_abi`, `aggregate_layout`, `function_spelling` and `[mangle] exports`
are retired: public aggregates are plain objects with named fields (D2), an exported
function keeps its declared callable kind, and a library keeps its export names.
