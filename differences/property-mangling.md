# Property mangling

Parent: [comparison index](index.md). Related: [pipeline and safety](pipeline-and-safety.md)
and [advanced opportunities](advanced-opportunities.md).

## Why Closure has several property passes

Closure must optimize arbitrary JavaScript, where one spelling may refer to unrelated runtime
properties and different spellings may never coexist on one receiver. When type-based property
optimizations are enabled, its property pipeline can use four distinct transformations:

1. collapse qualified namespace paths into variables;
2. disambiguate one source spelling into type-specific identities;
3. ambiguate different identities when receiver sets do not overlap;
4. assign final short spellings by occurrence count.

These steps are more important than the final `RenameProperties` allocator.

## Ordinary Closure renaming

[`RenameProperties`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/RenameProperties.java)
is spelling-based. It counts dot access, optional-chain access, fields, methods, accessors,
unquoted keys, and destructuring keys. It sorts by descending occurrence count and then by old
name, assigning shortest names first. See
[`RenameProperties.java` lines 101-111](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/RenameProperties.java#L101-L111)
and [lines 189-226](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/RenameProperties.java#L189-L226).

Safety and compatibility rules include:

- `prototype` and all extern properties are excluded;
- quoted bracket accesses and quoted keys are not rewritten and reserve their spellings against
  generated-name collisions;
- ES class `constructor` definitions are excluded;
- a coding-convention property-reflection call rewrites a string such as `"foo.bar"` consistently;
- a previous property map can be reused;
- an optional eligibility filter is all-or-nothing per source spelling.

Collection and reflection handling are in
[`RenameProperties.java` lines 337-472](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/RenameProperties.java#L337-L472).

This pass is not type-aware. Type-aware disambiguation/ambiguation may have happened before it when
their options and type checking are enabled; standard ADVANCED CLI defaults enable them.

## Disambiguation: one spelling becomes several

Closure's `DisambiguateProperties` associates each property occurrence with a receiver `Color`,
builds a subtype graph, and gives each source spelling a union-find of compatible receiver
clusters. Overrides, interfaces, unions, and common descendants force clusters together;
unrelated receiver families may get different intermediate names.

For example, unrelated `Foo.id` and `Bar.id` can become two independent identities. Ambiguation may
later merge nonconflicting identities; ordinary renaming then assigns final names by occurrence
count.

The fixed-point cluster propagation and renaming are wired in
[`DisambiguateProperties.java` lines 120-160](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/disambiguate/DisambiguateProperties.java#L120-L160).
Intermediate names are produced by
[`UseSiteRenamer.java` lines 44-106](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/disambiguate/UseSiteRenamer.java#L44-L106).

Conservative backoff is substantial:

- `prototype`, `constructor`, and `then` are always invalidated;
- unknown, top, structural, mismatch-affected, or otherwise invalidating types can invalidate a
  complete source spelling;
- an access on a type that does not declare or inherit the property also invalidates that spelling;
- extern, enum, and boxed-scalar receivers join an original-name cluster.

See
[`DisambiguateProperties.java` lines 215-299](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/disambiguate/DisambiguateProperties.java#L215-L299).

## Ambiguation: several spellings become one

`AmbiguateProperties` performs the opposite transformation. For every property it builds a bitset
of receiver colors and their transitive subtypes. Two different properties may share one generated
name when those bitsets do not intersect.

The pass:

- computes transitive subtype sets to a fixed point;
- orders properties by descending occurrence count, then spelling;
- greedily colors the implicit conflict graph;
- emits one short name per color class.

The subtype closure and coloring are in
[`AmbiguateProperties.java` lines 149-243](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/disambiguate/AmbiguateProperties.java#L149-L243).
The bitset nonintersection test is in
[lines 318-343](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/disambiguate/AmbiguateProperties.java#L318-L343).

This is a genuinely advanced Closure feature. It is weighted register allocation for property
names over a receiver-type interference graph.

## LilScript's property model

LilScript carries typed property provenance into IR:

- owned struct/class fields are `FieldGet`/`FieldSet` with owner and field index;
- open-record keys remain record operations;
- external JavaScript accesses are host operations.

See [`ir.rs` lines 195-212](../src/ir.rs#L195-L212) and
[`ir.rs` lines 624-671](../src/ir.rs#L624-L671). This avoids reconstructing ownership from emitted
JavaScript.

Property names often disappear before mangling. Internal aggregates may be scalar-replaced or use
positional arrays. Named property mangling is mainly relevant when a named aggregate
representation survives.

When global property naming is selected, LilScript:

- reserves `__proto__`, `constructor`, `prototype`, host names, boundary-visible fields, and extern
  fields under the default preserve-extern policy; closed-world config may release eligible externs;
- adds weight when an access is directly in a block designated as an ordinary loop body or update;
- sorts by descending weight and then source spelling;
- optionally uses the selected entropy-aware identifier alphabet.

See [`codegen_ir_js.rs` lines 6255-6432](../src/codegen_ir_js.rs#L6255-L6432).

With `owner_scoped_property_names`, LilScript allocates names independently per inheritance
component. A field identity is `(canonical owner, field index)`, inherited slots canonicalize to
their base owner, and unrelated components can reuse the same spelling. See
[`codegen_ir_js.rs` lines 6435-6572](../src/codegen_ir_js.rs#L6435-L6572).

## Direct comparison

| Question | Closure | LilScript |
|---|---|---|
| What identifies an owned property? | Spelling plus inferred receiver-type cluster | Explicit owner and field index in IR |
| Can same source spelling split? | Yes, through disambiguation | Yes for owned fields under owner-scoped naming |
| Can different source spellings merge? | Yes, if receiver subtype sets do not intersect | Across independent owner components; not a general receiver-use conflict coloring |
| Handles arbitrary JS property syntax | Broad AST coverage | Only compiler-understood owned/static/host forms |
| Hotness | Occurrence count | Static count with extra weight for designated loop body/update blocks |
| Final compression choice | Shortest names by count | Entropy alphabet and whole-artifact codec candidates |
| Reflection | Recognized string-name intrinsics | Typed/static-key paths, but no equivalent general rename intrinsic |
| Unknown receiver | Often invalidates whole spelling | Ownership categories prevent many unknowns; untyped boundaries pin names |
| Extern/public boundary | Extern AST, exports, coding conventions | Language-level externs, escape state, public ABI options |
| Incremental map | Yes for final spelling map | No persisted map |

## What Closure does better

Closure's receiver-set ambiguation is more general than owner-component reuse. Inside one
inheritance/type component, two properties that provably occur on disjoint receiver subsets can
still share one name. LilScript currently gives every retained field identity in that component a
distinct allocator slot.

Closure also supports a source-level property-reflection contract and emits diagnostic cluster
information. This makes aggressive mangling usable in code that must materialize a renamed key.

## What LilScript does better

LilScript's owned-property identity is explicit and proof-carrying. It does not need Closure's
large invalidating-type machinery for ordinary LilScript fields. Escape analysis can choose a
positional or scalar representation, eliminating the name instead of merely shortening it.

Its loop-block weight and exact compressed-size search provide additional cost signals beyond
Closure's raw occurrence count, although their practical advantage requires equivalent benchmarks.
Closure's ambiguation uses greedy coloring, not a globally optimal allocator, and final property
naming is not chunk weighted.

## Closure limitations to retain as tests, not features

- Mixed `obj.x` and `obj['x']` access is unsafe unless the boundary is modeled consistently.
- Computed property names are mostly opaque even when a constant could be inferred.
- A single unknown use can invalidate a whole spelling.
- Property reflection blocks ambiguation even when a receiver is supplied.
- The current test suite documents an unsound `Object.setPrototypeOf()` case in ambiguation.

LilScript should preserve its typed boundary instead of adopting Closure's whole-program
assumptions wholesale.
