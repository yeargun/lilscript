# Boundaries and escape

Parent: [Language](README.md). Related: [types](types-not-glue.md), [aggregates](aggregates.md), [effects](effects-purity.md). Host ABI: [`docs/web-platform.md`](../../web-platform.md).

## The only ways out

1. **`extern` function** — typed call to JS or C.
2. **`extern class` + `extern` global** — typed host object (`document`, `window`). Names are ABI. No `new` on extern classes. Methods must be called on a receiver (`this` cannot be lost). An internal class may `extend` an extern class (for example `Error`); it then always emits as a real `class ... extends` and calls the host constructor declared with `init(...)` through `super(...)`.
3. **`import extern`** — foreign ESM specifier + matching `extern` contract. JS-only.
4. **Root `js-module` runtime exports** — reusable library ABI. An exported function that takes or returns a value struct is published through a D2 adapter: a plain object with the fields in declaration order, read once per field on the way in, and a fresh object on the way out. Internally the struct stays in its private positional form. See [D2 for value structs](../../future-architecture.md#d2-for-value-structs).
5. **`print`** — portable observable output (treated as untyped boundary for escape).
6. **`JsValue`** — raw host value with a **closed** operation set.

Anything that reaches these is **escaping**. Representation-changing optimizations (scalar replacement, field-name deletion, some range facts) stop being legal.

## Escape states

| State | Meaning | Typical lowering |
|---|---|---|
| local | Never leaves the function as a distinguishable object | Scalar replacement, dissolve |
| typed | Returned or captured inside LilScript | Positional arrays / typed records; still not a host ABI |
| host | Host, export, `JsValue`, `print`, indirect call | The declared public ABI; host names exact |

This lattice is the escape fact of plan task M6.6, computed per allocation site
on the Program IR; a compare with `null` is not an escape. **Today** the compiler
has no escape fact: the JavaScript tree uses a syntactic member-only test to
scalarize objects, and the record family carries its own narrow non-escape proof.
The deleted route's `analyze_escapes` is described in
[history](../history/compilation/analyses.md).

## What escape blocks

- Scalar replacement of structs/classes (only local allocations).
- Value facts on aggregate fields (plan M6.7 keys them by `(nominal, slot)`).
- Renaming of names that are host ABI or public named aggregate fields. Nothing
  renames properties today; typed property renaming is plan M9.6.
- Treating a host getter as pure unless `pure` is declared.

## Zero wrappers

JS lowering of host ops is direct:

```js
document.createElement("button")
element.textContent="Run"
```

No registries, proxies, or runtime type checks. An `extern` means nothing by its name: the deleted route's table of host helpers recognized by spelling (`createEmptyObject()` → `{}` and the like) was dropped by design. Operations the compiler understands are language operations, `JS.*` operations or delivered host modules; plan M10.2 turns them into a typed host catalog.

`assume_pure_property_reads` is an explicit unsafe ABI opt-in (Terser
`pure_getters`, default off). It is not a type proof. Reusable typed replacements
are tracked in [compressor surface](compressor-surface.md) and the
[migration plan, step 009](../../migration/record-2026-09.md#009-reusable-compression-families).

## `pure extern`

A trusted host promise. Unused calls may be removed (from plan M7.2). Violating it is an integration bug. Trusted names must appear in `[lint].pure_extern_allowlist` because the compiler cannot verify host source.

## Config

- `[mangle].properties` / `property-mangling` — owned fields only, and no producer yet (plan M9.6); host `extern` names always stay exact
- `[mangle].preserve_properties` — names the port's callers read in code the compiler never sees
- Public aggregates are plain objects with named fields (D2); `public_aggregate_abi = "positional"` and `mangle.exports` are retired
- Host reads stay effectful unless `pure`
