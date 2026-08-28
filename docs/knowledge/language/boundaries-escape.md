# Boundaries and escape

Parent: [Language](README.md). Related: [types](types-not-glue.md), [aggregates](aggregates.md), [effects](effects-purity.md). Host ABI: [`docs/web-platform.md`](../../web-platform.md).

## The only ways out

1. **`extern` function** — typed call to JS or C.
2. **`extern class` + `extern` global** — typed host object (`document`, `window`). Names are ABI. No `new` on extern classes. Methods must be called on a receiver (`this` cannot be lost).
3. **`import extern`** — foreign ESM specifier + matching `extern` contract. JS-only.
4. **Root `js-module` runtime exports** — reusable library ABI.
5. **`print`** — portable observable output (treated as untyped boundary for escape).
6. **`JsValue`** — raw host value with a **closed** operation set.

Anything that reaches these is **escaping**. Representation-changing optimizations (scalar replacement, field-name deletion, some range facts) stop being legal.

## Escape states (`src/semantic.rs`)

| State | Meaning | Typical lowering |
|---|---|---|
| `LocalOnly` | Never leaves the function as a distinguishable object | Scalar replacement, dissolve |
| `EscapesToTypedCode` | Returned/captured inside LilScript | Positional arrays / typed records; still not a host ABI |
| `EscapesToUntypedBoundary` | Host, export, `JsValue`, `print`, indirect call | Named or configured public ABI; host names exact |

`src/optimizer.rs::analyze_escapes` iterates a graph of values and globals to a fixpoint. Conservative rules: `CallValue` (unknown callee), unresolved indirect calls, and host ops mark untyped escape.

## What escape blocks

- Scalar replacement of structs/classes (`value_escapes == LocalOnly`).
- Finite-value / integer-range facts on aggregate fields (`unsafe_aggregate_owners` in `src/value_analysis.rs`).
- Property mangling of names that are host ABI or public named aggregate fields (unless `mangle.exports` / `export-mangling`).
- Treating a host getter as pure unless `pure` is declared.

## Zero wrappers

JS lowering of host ops is direct:

```js
document.createElement("button")
element.textContent="Run"
```

JS lowering of host ops is direct:

```js
document.createElement("button")
element.textContent="Run"
```

No registries, proxies, or runtime type checks. Known host factories used by ports may still lower in the optimizer (`createEmptyObject()` → `{}`, `callN(f, null, …)` → direct call). That is whole-program knowledge, not a wrapper. See [jQuery](../evidence/jquery.md).

`assume_pure_property_reads` is an explicit unsafe ABI opt-in (Terser `pure_getters`, default off). It is not a type proof. Language replacement is [07.7](../migration/07-global-compressor.md#077--language-proofs-and-explicit-lowering-contracts) / [compressor surface](compressor-surface.md).

## `pure extern`

A trusted host promise. Unused calls may be DCE’d. Violating it is an integration bug. Trusted names must appear in `[lint].pure_extern_allowlist` because the compiler cannot verify host source.

## Config

- `[mangle].properties` / `property-mangling` — owned fields only; `extern class` members never rename
- `[mangle].exports` / `export-mangling` — public ESM names and public aggregate fields
- `javascript.public_aggregate_abi` — named vs positional **public** handles
- Host reads stay effectful unless `pure`
