# Pipeline, boundaries, and safety

Parent: [comparison index](index.md). Related: [property mangling](property-mangling.md)
and [source reference](source-reference.md).

## Renaming is a pipeline effect

Closure's final manglers are simple because many difficult decisions happen earlier. Its current
production sequence includes:

1. normalization and extern-property gathering;
2. alias inlining and qualified-property collapse;
3. dead-code removal before optional type disambiguation;
4. type-based property disambiguation when enabled;
5. repeated function, variable, and property inlining plus DCE;
6. flow-sensitive local inlining and cross-chunk motion;
7. optional property ambiguation and ordinary property renaming;
8. raw-global export gathering;
9. live-range coalescing;
10. variable renaming.

The wiring is spread across
[`DefaultPassConfig.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/DefaultPassConfig.java),
especially
[lines 519-699](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/DefaultPassConfig.java#L519-L699)
and [lines 920-1143](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/DefaultPassConfig.java#L920-L1143).

LilScript's analogous pipeline moves much of the work earlier than JavaScript:

1. semantic ownership and nominal field indices;
2. typed lowering and escape propagation;
3. SSA optimization, specialization, inlining, and DCE;
4. scalar, positional, or named aggregate representation;
5. liveness-aware emission and mangling;
6. whole-artifact structural and naming candidates;
7. exact codec ranking;
8. bounded terminal JavaScript cleanup and remapping.

The configured path is summarized in
[`compiler.rs` lines 1405-1519](../src/compiler.rs#L1405-L1519).

## Namespace and property collapse

Closure's `InlineAndCollapseProperties` turns a path such as
`goog.events.handleEvent` into a flat variable such as `goog$events$handleEvent`. That variable can
then be dead-code eliminated, inlined, coalesced, or renamed by ordinary variable machinery.
Object-literal members can also be split into independent variables.

The current implementation is
[`InlineAndCollapseProperties.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/InlineAndCollapseProperties.java),
with collapse rationale at
[lines 1257-1315](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/InlineAndCollapseProperties.java#L1257-L1315).

Important details include:

- alias inlining runs to a fixed point over an incrementally updated global namespace;
- one global write, no local writes, nonescaping ancestors, dominance, and chunk dependencies gate
  alias replacement;
- flattening a method call marks it as a free call so removing the receiver does not change `this`;
- object spreads, deletes, getters/setters, `hasOwnProperty`, loops, conditional definitions,
  aliases, and `@nocollapse` trigger conservative backoff;
- collapsed declarations are placed in the deepest common ancestor chunk that dominates uses.

LilScript already gets analogous wins when module namespaces disappear during linking or an
aggregate scalarizes/uses positional layout. A direct JavaScript-shaped collapse pass is most
useful only for retained named records, host-adapter facades, or imported JavaScript that still
survives as a namespace object.

## Externs, exports, and reflection

Closure uses several overlapping boundary mechanisms:

- extern source files reserve globals and properties;
- coding conventions identify exported names and property-reflection functions;
- `@export` can synthesize an `Object.prototype` extern and an export call;
- global `window.foo`, `globalThis.foo`, and similar accesses are gathered after property renaming
  so variable renaming does not break the public root;
- quoted/computed keys generally remain literal boundaries.

LilScript instead starts with language-level categories:

- imported/extern functions and globals retain ABI identities;
- owned fields, record keys, and host properties are distinct IR operations;
- escape state and public aggregate sets decide whether a named field must remain stable;
- ESM exports may retain a public alias while the internal binding is mangled;
- export and extern-field policy are ABI decisions that candidate search does not silently flip.

LilScript's boundary intent is explicit for values represented by its language and IR. Closure's
broader JavaScript interop and reflection surface requires its overlapping boundary mechanisms.

## Chunk behavior

Closure's final property allocator is whole-program and frequency based; it does not weight startup
chunks. Its collapse and alias passes are nevertheless chunk-aware. They check dependency order,
find a common ancestor chunk for declarations, and avoid moving an initialization into a chunk
that some use does not depend on.

LilScript already emits chunks separately and sums a deployment objective containing raw, gzip,
and Brotli weights, request overhead, dependency depth, preload discount, and cache-reuse discount.
Optional joint chunk symbol search currently varies function layout and local-name reservation. See
[`compiler.rs` lines 1049-1136](../src/compiler.rs#L1049-L1136) and
[lines 1311-1348](../src/compiler.rs#L1311-L1348). It does not yet search per-symbol
startup/load-probability weights, identifier/property alphabets, or safe chunk-local namespaces.
The checked-in root config uses [`mode = "single"`](../lilscript.toml#L90-L93), so this machinery is
not active there.

## Stability and diagnostics

Closure provides:

- previous variable and property maps as inputs;
- deterministic map serialization sorted by original key;
- pseudo names such as `$original$$` and `$property$`;
- forward and reverse map lookup;
- disambiguation cluster and invalidation logs;
- summaries for property ambiguation and disambiguation.

LilScript is deterministic but not incrementally stable by contract. Fixed-seed maps, explicit
tie-breaks, and a fixed permutation seed make the same input reproducible. An inserted declaration
can still move many names, and no persisted many-to-many property provenance is emitted.

These are different properties:

- deterministic: identical inputs produce identical output;
- stable: a small input edit preserves most old output names;
- explainable: a build can report why a name changed or stayed pinned.

Closure is ahead on the latter two.

## Source maps

Closure performs these transformations on its AST and keeps compiler source information through
the pipeline. LilScript's terminal naming passes rewrite final JavaScript by byte offsets and return
a new string. No native compiler source-map composition was found for those transformations.

This is not just a debugging feature. It limits how aggressively late lexical search can evolve:
every additional rewrite increases the cost of reconstructing provenance after semantic identity
has been erased.

## Safety comparison

### Closure's conservative mechanisms

- unknown/mismatched receiver types invalidate property optimizations;
- extern, quoted, and reflected names are reserved;
- alias/collapse analysis checks writes, escapes, dominance, receiver semantics, and chunks;
- generated names filter keywords and configured reserved characters;
- normalized AST scopes provide broad JavaScript syntax coverage.

### Closure's known hazards

- ADVANCED assumes whole-world visibility and consistent property-access conventions;
- mixed dot and quoted access can be broken when not modeled as extern/reflection;
- some constructor/enum collapse behavior is retained despite explicit unsafe legacy notes;
- `Object.setPrototypeOf()` is a documented ambiguation gap;
- a global-prefix configuration has a noted possible local/global collision;
- one unknown property use often disables optimization for an entire spelling.

### LilScript's conservative mechanisms

- typed IR preserves property ownership and host boundaries;
- escape analysis protects dynamic/public representations;
- unsupported generated-JS scopes are marked unsound, while duplicate names are marked ambiguous
  and their affected resolutions become unresolved;
- exact-score alternatives preserve the incumbent; terminal textual candidates receive partial
  generated-JS token/declaration validation, while structural emitter candidates are measured
  directly;
- local convergence blocks captures and ambient names that its resolver successfully observes.

### LilScript's current hazards

- generated-JS scope reconstruction is function/catch oriented, not a complete lexical-scope model;
- raw token remappers are safe only when their callers have performed complete eligibility checks;
- post-emission renames do not compose source maps.

The 2026-08-29 correctness migration now makes whole-artifact convergence and terminal rename
admission require total declaration resolution, rejects template-bearing rename candidates,
reserves fixed descendant names, makes bounded name exhaustion explicit, and gates fresh-object
collection on the pristine-builtins contract. Those changes still require the pinned five-fork G2
evidence checkpoint before their migration units close.
