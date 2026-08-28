# Aggregates

Parent: [Language](README.md). Related: [types](types-not-glue.md), [escape](boundaries-escape.md). Config: [`public_aggregate_abi`](../config/javascript-priority.md), [`[mangle]`](../config/mangle.md).

## Three layouts, three jobs

| Kind | Shape | Keys | Optimization |
|---|---|---|---|
| `struct` | Positional value | Field **indexes** in IR | Scalar-replace if `LocalOnly`; else positional array or boundary object |
| `class` | Nominal reference, `init`, methods | Indexes internally | Devirtualize methods; dissolve if `LocalOnly`; no virtual dispatch |
| `object` | Closed public singleton | **ABI keys**; bodies are private functions | Nest/anonymize/fold implementations; never mangle keys unless `mangle.exports` |
| `Record<T>` | Open homogeneous map | **String keys are data** | Never mangled; reads are `T?` |
| `extern class` | Host interface | Exact ABI names | Never mangled; never constructed |

IR uses `FieldGet`/`FieldSet` with `index` for structs/classes, `RecordFieldGet`/`Set` with a string for records, and `HostFieldGet`/`Set` for host names (`src/ir.rs`).

Positional arrays and SSA locals are the **usual** size win, not a theorem.
`LocalOnly` dissolution is an always-on IR pass (no “keep the object” clone).
Array vs named object is `aggregate_layout` unless `joint-representation-search`
is actually enabled — root `lilscript.toml` omits that decision, so repo-default
compiles do not compete layouts. ES `class` is constructor identity, not instance
backing. Full map: [decision registry](../compilation/decision-registry.md#aggregates-class-struct-object-record),
[class identity](../compilation/class-identity.md).

## Inheritance is non-virtual on purpose

Single inheritance flattens base fields first. `super(...)` must be first in derived `init`. Upcasting works. **Overriding is rejected**: silent static dispatch would be unsound; vtables would add size and memory. Native C currently rejects inheritance until the subtype pointer ABI is fixed.

This is a compression-oriented OO subset, not a TS `extends` clone.

## Named vs positional — two independent knobs

`javascript.public_aggregate_abi` (default `named`):

- `named` — structs/classes that cross a **reusable JS boundary** keep stable field names (and fields reachable through public fields).
- `positional` — compact array-backed **handles**. Legal only when JS consumers treat exports as opaque and pass handles back into compiled functions. Solid’s open-world package uses this (`labs/solid-client/config/open-world.toml`).

`javascript.aggregate_layout` (default `positional`):

- `positional` — smallest emitted instance shape (array slots).
- `named` — hidden-class objects; comment in `src/config.rs`: fewer bytes **per instance at runtime** in V8 because named properties sit inline rather than behind an elements store. That is a **runtime vs transfer** tradeoff, not a codec search by itself. Joint representation search can still compete layouts when enabled.

`[mangle].properties` renames LilScript-owned fields. Named fields on the public ESM surface stay stable unless exports are also mangled. Internal fields already lower to scalars or numeric slots regardless.

## Construction spelling

`Point{10, 20}` is positional struct construction. `new Vector(3, 4)` is class construction. `record { key: value }` is an open record with a null-prototype semantic contract. `object { key: value }` is an ordinary-prototype, JavaScript-only `JsValue` dictionary and keeps inherited hook behavior. A JS-only whole-artifact candidate may project proven closed record observations and eliminate the materialized record, but a surviving record never changes backing. See [aggregate lowering](../compilation/aggregate-lowering.md#closed-record-observation-projection).

`export class` does not produce a JS constructor. `export constructor C;`
publishes a named, constructible ES class; `as` supplies a public export alias.
Identity-free classes stay dissolved.

## Config that changes aggregate emission

| Knob | Effect |
|---|---|
| `public_aggregate_abi` | Public JS field names vs opaque arrays |
| `aggregate_layout` | Instance backing (array vs named object) |
| `property-mangling` / `mangle.properties` | Owned field names |
| `export-mangling` / `mangle.exports` | Public names + public fields |
| `joint-representation-search` | Compete named vs positional instance backing (off unless listed; omitted from root toml) |
| `struct_method_shorthand` | Default on in `js_options()`; candidate search flips it (`k(){…}` vs `k:function(){…}`) |

Size-first enables property mangling by default; other priorities do not, unless the compression allowlist or `[mangle]` opts in.
