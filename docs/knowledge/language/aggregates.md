# Aggregates

Parent: [Language](README.md). Related: [types](types-not-glue.md), [escape](boundaries-escape.md). Contract: [language v0.1](../../language-v0.1.md#aggregates-and-classes). Keys: [configuration.md](../../configuration.md).

## Three layouts, three jobs

| Kind | Shape | Keys | Optimization |
|---|---|---|---|
| `struct` | Positional value | Fields by position | Scalar-replace when local; else positional storage or a boundary object |
| `class` | Nominal reference, `init`, methods | Fields internally; name-keyed members today (plan M4.1 gives them `FieldRef{nominal, slot}`) | Static dispatch; dissolve when identity is unobserved; no virtual dispatch |
| `object` | Closed public singleton | **ABI keys**; bodies are private functions | Does not compile today; plan M10.10 deletes or implements it |
| `Record<T>` | Open homogeneous map | **String keys are data** | Never renamed; reads are `T?` |
| `extern class` | Host interface | Exact ABI names | Never constructed; never renamed |

Positional storage and plain locals are common size wins, not a theorem. Which
layout an allocation gets is a representation choice: today the product family
(value-struct scalars) and the record family (captured `Record<int>` scalars) make
it per candidate, and plan M9.7 turns layout into one codec-judged choice per
nominal or allocation (scalars, positional, named object, real class). ES `class`
is constructor identity, not instance backing. The deleted route's layout
machinery (`aggregate_layout`, `joint-representation-search`, the decision
registry) is in [history](../history/compilation/aggregate-lowering.md).

## Inheritance is non-virtual on purpose

Single inheritance flattens base fields first. `super(...)` must be first in derived `init`. Upcasting works. **Overriding is rejected**: silent static dispatch would be unsound; vtables would add size and memory. Native C compiles internal inheritance through pointer records. Plan M10.5 proposes sealed hierarchies with virtual methods, where each call site gets a static call, a tag switch or a prototype method.

This is a compression-oriented OO subset, not a TS `extends` clone.

## Public shape

At a declared public boundary a struct is one documented shape (D2): a plain
object whose own enumerable data properties are its fields in declaration
order. That is the only public aggregate shape; the old
`public_aggregate_abi = "positional"` is refused. Inside the program the
compiler chooses the representation.

`[mangle].properties` would rename LilScript-owned fields, but nothing renames
properties yet (plan M9.6). Internal fields already lower to scalars or numeric
slots where the representation choice allows.

## Construction spelling

`Point{10, 20}` is positional struct construction. `new Vector(3, 4)` is class construction. `record { key: value }` is an open record with a null-prototype semantic contract. `object { key: value }` is an ordinary-prototype, JavaScript-only `JsValue` dictionary and keeps inherited hook behavior. A surviving record never changes backing.

`export class` does not produce a JS constructor. `export constructor C;`
publishes a named, constructible ES class; `as` supplies a public export alias.
Identity-free classes stay dissolved. **Until plan M4.1** `export constructor` is
refused.

## Config that changes aggregate emission

| Knob | Effect |
|---|---|
| `[policy.tactics] scalar-replacement` | `off` vetoes scalar replacement |
| `mangle.preserve_properties` | Names the port's callers read; kept whatever renaming does |
| `assume_pure_property_reads` | Contract assumption about foreign values (Terser's `pure_getters`), off by default |

`public_aggregate_abi`, `aggregate_layout`, `struct_method_shorthand`,
`mangle.exports` and the `joint-representation-search` list entry are retired:
each warns "no effect in this compiler" or is refused.
