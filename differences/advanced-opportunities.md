# Advanced opportunities for LilScript

Parent: [comparison index](index.md). Evidence: [variable mangling](variable-mangling.md),
[property mangling](property-mangling.md), and [pipeline and safety](pipeline-and-safety.md).

## Priority 0: harden current terminal renaming

Implementation status: the following three corrections are implemented in the 2026-08-29 working
tree with focused and canonical tests. They remain open in the migration ledger until the pinned
five-fork G2 evidence gate can run.

### Use binding resolution for two-character shortening

The previous two-character shortening path used a weaker syntax-only predicate, so a free
host/global identifier could look eligible even though renaming it changed behavior.

The current working tree routes one- and two-character whole-artifact admission through
`BindingResolution`, requires total resolution, and conservatively rejects template-bearing
artifacts until template expressions carry binding identity.

Relevant code:
[`js_peephole/mod.rs` lines 1014-1058](../src/js_peephole/mod.rs#L1014-L1058),
[`js_peephole/mod.rs` lines 1203-1234](../src/js_peephole/mod.rs#L1203-L1234), and
[`compiler.rs` lines 8403-8452](../src/compiler.rs#L8403-L8452).

### Remove the two-character generator ceiling

`converge_local_names` still intentionally stops after two-character candidates, but its generator
now returns `None` after that finite space instead of indexing past its alphabet.

Relevant code: [`rename.rs` lines 206-225](../src/js_peephole/rename.rs#L206-L225).

### Require total resolution and reserve fixed descendant bindings

The implementation now requires total resolution, blocks free/unresolved or referenced outer
bindings, reserves fixed descendant function/class names against outer rewrites, and documents that
actual invariant.

Relevant code: [`rename.rs` lines 17-26](../src/js_peephole/rename.rs#L17-L26) and
[lines 52-105](../src/js_peephole/rename.rs#L52-L105), plus
[`binding.rs` lines 179-185](../src/js_peephole/binding.rs#L179-L185).

## Priority 1: add rename provenance and diagnostics

Closure's persisted maps and pseudo-name mode solve practical production problems that exact codec
search does not:

- stable differential delivery and cache behavior;
- deobfuscation of production stack traces;
- explanation of why a property stayed public or was invalidated;
- comparison of naming changes between compiler versions.

LilScript needs a richer format than Closure's one-to-one `VariableMap`, because owner-scoped
properties can split one source spelling and multiple identities can share one output spelling.
Record at least:

| Field | Purpose |
|---|---|
| symbol provenance | module, function, declaration/value identity |
| source spelling | human-readable origin |
| semantic property identity | owner and field index, or host/record category |
| emitted spelling | final selected name |
| candidate reason | frequency, affinity, coalescing, map reuse, codec search |
| pinned reason | extern, public ABI, escape, reflection, reserved word |
| source ranges | stack-trace and source-map integration |
| artifact/chunk | delivery-specific interpretation |

The map should be diagnostic and incremental input, not a public ABI promise.

Also expose a strict assertion analogous to Closure's `propertiesThatMustDisambiguate`: selected
properties can be declared optimization requirements, turning an unexpected escape/invalidation
into a build error rather than silent size drift. Closure's check is in
[`DisambiguateProperties.java` lines 150-156](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/disambiguate/DisambiguateProperties.java#L150-L156).

## Priority 2: receiver-set property coloring

LilScript's owner-scoped allocator already gives unrelated inheritance components independent
alphabets. Extend that idea within a component:

1. collect the possible receiver owner/type set for every retained named field identity;
2. include descendant and union reachability required by actual accesses;
3. add conflicts when two fields can be selected on the same runtime receiver;
4. color the conflict graph so nonconflicting fields can share a spelling;
5. rank colors using loop/profile weight;
6. send greedy, DSATUR, and bounded recoloring candidates to exact artifact scoring.

This is Closure's `AmbiguateProperties` idea adapted to LilScript's stronger typed ownership. It
should operate before JavaScript emission and only on named layouts. Scalar/positional layout still
wins whenever legal.

Do not copy Closure's global-spelling invalidation. Quarantine uncertainty to the affected owner
component or receiver cluster when typed provenance permits it.

## Priority 3: reflected-property intrinsic

Add a typed equivalent of Closure's property rename function for the cases where a program must
materialize a field key. The intrinsic should carry a semantic property identity, not just a raw
string:

```text
property_name(Type, field)
```

The compiler can then emit the selected string spelling and keep the use in receiver-set conflict
analysis. A raw string-only form should remain conservative because it loses owner identity.

This is useful for serializers, schema tables, event delegation, generated adapters, and
interop metadata without pinning every occurrence to its source spelling.

## Priority 4: typed generated-ID mapping

Closure's `ReplaceIdGenerators` supports consistent, occurrence-unique, stable-hash, XID, and
caller-supplied mappings for literal IDs. It can consume previous mappings, emit new maps, and use
pseudo names. See
[`ReplaceIdGenerators.java` lines 35-40](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ReplaceIdGenerators.java#L35-L40)
and [lines 82-254](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ReplaceIdGenerators.java#L82-L254).

A LilScript intrinsic could cover generated DOM IDs, CSS-module tokens, protocol-local tags, and
schema IDs. Keep each ID domain explicit and distinguish stable hashing from shortest-name
allocation; only the latter should enter codec search.

## Priority 5: cheap locality proposals

Closure assigns equal-length global names by first source occurrence, producing similar spellings
near one another. LilScript should add analogous proposals without replacing exact scoring:

- declaration/source proximity within equal-width names;
- emitted-function-layout proximity;
- n-gram similarity between nearby declarations and candidate names;
- moving the existing same-shape parameter/local convergence heuristic into the safe emitter or
  hygienic binding model, optionally grouped by actual function shape;
- separate initial-chunk and lazy-chunk proposals.

Each proposal is cheap and deterministic. The existing raw/gzip/Brotli scorer decides whether it
actually helps.

## Priority 6: extend joint chunk symbol search

Closure's collapse pass is dependency-aware, but its final property names are globally frequency
ranked. LilScript already scores emitted chunks independently and combines transfer, request,
dependency-depth, preload, and cache-reuse costs. Extend the existing joint search to:

- include identifier and property alphabet candidates;
- assign per-symbol entry/startup and expected lazy-load weight;
- preserve one semantic rename identity across chunks where required;
- allow chunk-local names only where no cross-chunk reference exists.

This belongs in `score_javascript_chunk_plan`, not in a separate single-artifact approximation.
The checked-in root config is single-bundle and does not enable joint chunk symbol search.

## Priority 7: bounded stronger coloring

Closure uses greedy graph coloring for variable coalescing and property ambiguation. LilScript can
exploit its bounded-search architecture:

- greedy baseline for all artifacts;
- DSATUR as a deterministic alternative;
- local color-class swaps and recoloring;
- exact coloring only for small connected components;
- exact codec scoring of emitted candidates, because minimum color count need not minimize Brotli.

Apply this first to retained property conflict components. The SSA local allocator already has
strong phi affinity and may offer less incremental value.

## Priority 8: selective namespace exposure

Do not add a generic JavaScript namespace-collapse pass merely because Closure has one. LilScript
already erases modules and can scalarize or positionalize aggregates before JS exists.

Target only residual shapes:

- retained named static records used as namespaces;
- generated host-adapter facades;
- imported closed-world JavaScript objects with proven single initialization;
- public facade/private implementation pairs where the public alias can remain stable.

Preserve Closure's important correctness lessons: dominance, write count, escape, delete, spread,
getter/setter, `hasOwnProperty`, chunk dependency, and method receiver semantics.

## Priority 9: source-map-aware terminal search

Represent terminal edits as mapping-preserving transformations or move them onto a hygienic target
JavaScript IR. At minimum, compose byte-range edits into the source map. Longer term, one target-JS
binding model should replace the overlapping resolver/index/remapper safety layers.

This enables stronger post-emission search without sacrificing production debugging.

## Ideas not to copy directly

- Do not replace exact codec scoring with occurrence count.
- Do not replace typed SSA allocation with positional `L n` slots.
- Do not use one unknown property access to disable every proven-safe owner cluster.
- Do not inherit Closure's mixed quoted/unquoted property-access contract.
- Do not make rename maps an ABI guarantee; use them as stability hints.
- Do not run unbounded simplify/inline loops. Keep the incumbent and bounded budgets.
- Do not introduce a generic collapse pass for representations that typed lowering can eliminate
  more cleanly.

## Suggested implementation sequence

1. Fix terminal rename admission and the convergence generator bound.
2. Add a many-to-many rename/provenance report with pinned reasons.
3. Add receiver-set conflict data for retained named owned fields.
4. Implement greedy ambiguation as an optional scored candidate.
5. Add DSATUR/recoloring candidates for small property components.
6. Add the typed reflected-property intrinsic.
7. Add typed generated-ID domains and optional required-disambiguation assertions.
8. Extend joint chunk search to symbol alphabets and safe chunk-local namespaces.
9. Move remaining lexical naming search onto a source-map-aware target-JS representation.
