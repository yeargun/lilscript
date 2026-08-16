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

`Point{10, 20}` is positional struct construction. `new Vector(3, 4)` is class construction. `record { key: value }` is an open record with a null-prototype semantic contract. A JS-only whole-artifact candidate may project proven closed record observations and eliminate the materialized record, but a surviving record never changes to ordinary-object backing. See [aggregate lowering](../compilation/aggregate-lowering.md#closed-record-observation-projection). jQuery cannot generally use `record{}` for plain objects because its object model does observe ordinary-object behavior; the port uses `createEmptyObject()` which the optimizer lowers to `{}`. Language choice here is ABI, not style.

## Config that changes aggregate emission

| Knob | Effect |
|---|---|
| `public_aggregate_abi` | Public JS field names vs opaque arrays |
| `aggregate_layout` | Instance backing (array vs named object) |
| `property-mangling` / `mangle.properties` | Owned field names |
| `export-mangling` / `mangle.exports` | Public names + public fields |
| `joint-representation-search` | Compete layouts under the codec |
| `struct_method_shorthand` | Always on in `js_options()` today |

Size-first enables property mangling by default; other priorities do not, unless the compression allowlist or `[mangle]` opts in.
