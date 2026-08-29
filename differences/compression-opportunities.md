# Closure-inspired compression opportunities

Parent: [comparison index](index.md). Naming-only opportunities remain in
[advanced opportunities](advanced-opportunities.md).

This list favors additions that complement LilScript's typed IR and exact raw/gzip/Brotli search.
It does not rank work by implementation effort.

## 0. Graduate the current correctness firewall

The working tree now fixes the binding-resolution/name-generation issues listed in
[advanced opportunities](advanced-opportunities.md#priority-0-harden-current-terminal-renaming) and
gates fresh-empty-object property collection on the explicit pristine-builtins contract. Complete
the pinned five-fork G2 evidence checkpoint before marking those migration units landed or
expanding terminal rewrite coverage.

## 1. Add a general condition-form candidate generator

Port the idea behind Closure's `MinimizedCondition`, not its tiny punctuation cost function:

1. represent both positive and negated forms of Boolean trees;
2. include De Morgan, comparison inversion, branch swap, and precedence-aware parentheses;
3. preserve NaN, BigInt, coercion, and side-effect semantics using typed facts;
4. emit a small Pareto set of forms;
5. score the complete artifact under raw/gzip/Brotli.

This generalizes many isolated Boolean folds and can discover that a locally longer negated form
improves surrounding branch syntax or compressed repetition.

Evidence: [peephole condition minimization](peephole-and-syntax.md#condition-minimization).

## 2. Replace fixed peephole round counts with dirty-region scheduling

Closure's peephole driver revisits only changed function/script roots until stable. LilScript can
adopt the worklist while retaining stricter controls:

- mark the containing target-JS function/block dirty after a rewrite;
- rerun only families whose input facts may have changed;
- charge every visit/rewrite/codec probe to existing budgets;
- stop at no change, budget exhaustion, or a hard per-region cap;
- keep the pre-pass artifact as incumbent.

This should expose long rewrite chains without repeatedly rescanning a complete artifact.

## 3. Complete typed known-method folding

Implement the string intrinsics that already reach LilScript's constant-fold dispatcher but lack
fold logic:

- `trim`, `trimStart`, `trimEnd`;
- `slice`;
- bounded `split`;
- constant/literal `ArrayJoin` with partial adjacent-run folding.

Add bounded literal `replace` to the dispatcher and then implement it separately.

Use exact JavaScript UTF-16 and replacement semantics, cap output expansion, and submit folded and
unfolded forms to scoring when folding can increase raw size.

Closure's implementation and negative tests provide an extensive semantic corpus. See
[constant and known-method folding](peephole-and-syntax.md#constant-and-known-method-folding).

## 4. Extend literal construction recovery

Generalize current fresh-object and array-push folds to:

- direct indexed writes into fresh arrays;
- writes following a nonempty initial literal;
- complete closed-record `keys`, `values`, `entries`, and `hasOwn` observations.

The fresh-empty-object fold handles static dot/string keys and duplicate-key order and is now gated
by pristine builtins. New typed literal construction should instead use ownership/prototype and
escape proof. Explicit blockers should include holes and inherited numeric setters,
object-prototype setters, getters/setters, spread, computed effects, self-reference, and immediately
invoked closures.

## 5. Add a true outer fixed point or genuine pass-order candidates

LilScript already implements unused returns/parameters, constant-parameter specialization,
inlining, DCE, and reachability, and repeats them in a fixed schedule. Its currently named
phase-order variants adjust CSE/specialization/inlining settings rather than permuting pass order.
Test either a budgeted outer fixed point or genuine order candidates around Closure's productive
cycle:

```text
unused returns -> parameter optimization -> function inline -> variable/value inline -> DCE -> peephole
```

Compare at least the current schedule, one extra complete cycle, and a no-duplication variant. Do
not make Closure's uncosted argument/constant duplication unconditional. LilScript already has
inlining-off, specialization-off, call-site-specialization-off, and reusable-helper variants; reuse
those as the no-/reduced-duplication baselines rather than adding duplicates.

## 6. Add build-time define and packed-toggle domains

Closure processes configurable primitive defines before its main optimizer and can hard-code or
bit-pack named runtime toggles. A LilScript equivalent is useful when source packages expose
feature flags that should disappear before specialization and DCE.

Keep compile-time constants distinct from runtime toggle packs. For runtime packs, compare direct
booleans, object/array tables, and bitsets under exact artifact and access-site cost.

Treat assertion/debug stripping as a separate explicit semantic build mode. Closure's configured
`StripCode` is powerful, but LilScript should prefer declared domains over type/name suffix
conventions.

Evidence: [build constants and packed toggles](data-modules-delivery.md#build-constants-and-packed-toggles).

## 7. Broaden scored structural-prefix extraction

LilScript already scores shared versus direct `Array.prototype` aliases and a repeated window root.
Broaden that mechanism to class prototype tables, host-adapter paths, or retained namespace roots.
Charge:

- helper declaration once per artifact/chunk;
- assignment/setup once per contiguous cluster;
- each shortened use;
- chunk import/export consequences.

Closure supplies a grouping and cost model, but its implementation explicitly assumes RHS code does
not replace the prototype object. Do not inherit that unsafe assumption. LilScript's exact scorer
can decide whether repetition should remain inline for gzip/Brotli after semantic eligibility is
proved.

## 8. Add semantic replaceable-string domains

Generic string pooling preserves content. Some applications can explicitly permit stronger
compression:

- diagnostic/error messages replaced by short codes plus a sidecar map;
- localization placeholders substituted before or after optimization;
- CSS/DOM/protocol-local IDs generated consistently or per occurrence;
- stable hashes for cache compatibility versus sequential names for minimum size.

Keep these as typed/configured domains. Never infer that arbitrary user strings may be replaced.
Measure runtime decoder/helper costs in every shipped artifact.

Evidence: [generated IDs and messages](data-modules-delivery.md#generated-ids-and-messages).

## 9. Strengthen switch and exit candidates

Use Closure's switch pruning, fallthrough merging, duplicate return/throw, and follow-node tests as
a differential corpus. Implement general forms in CFG IR where exception regions, finally edges,
loop targets, and lexical declarations are explicit. LilScript already lets state-machine,
structured switch/condition, dense-table, and late-exit alternatives compete; extend their switch
pruning and fallthrough analysis rather than introducing another parallel family.

## 10. Add receiver-set property coloring

Extend owner-component reuse with actual receiver conflict sets so fields within one inheritance
component can share spellings when they cannot occur on the same runtime receiver. Use greedy,
DSATUR, recoloring, and exact small-component variants as bounded candidates.

Details: [advanced opportunities](advanced-opportunities.md#priority-2-receiver-set-property-coloring).

## 11. Add provenance, stable maps, and required-optimization assertions

Emit many-to-many symbol/property maps, source ranges, pinned reasons, and candidate decisions.
Allow selected internal properties or functions to assert that expected disambiguation/inlining
remains legal, converting silent size drift into a build diagnostic.

This is infrastructure for safely iterating on all other compression work.

## 12. Extend split-build search

Current joint chunk search varies function layout and local-name reserve. Extend it with:

- identifier/property alphabets;
- per-symbol startup and lazy-load probability;
- dependency-safe declaration SCC movement;
- safe chunk-local namespaces;
- method motion only when total and startup objectives justify stubs.

The existing deployment objective should remain authoritative.

## 13. Move terminal rewrites onto a hygienic target-JS tree

Closure's mature scope and AST infrastructure is a major enabler for its breadth. LilScript should
not duplicate that breadth with more token-level special cases. A target-JS tree carrying binding,
property, effect, module, source-map, and lowering-obligation identity would make the condition,
literal, declaration, and naming proposals above composable and auditable.

## High-value differential test imports

Even before implementing new passes, import/adapt Closure test cases for:

- dead assignments with computed-key and RHS effects;
- overwritten property stores across blocks, accessors, and exception boundaries;
- unused arguments with defaults, spread, rest, and `arguments`;
- array holes/spread, object getters, duplicate keys, and `super`;
- template raw/cooked escape boundaries;
- NaN/BigInt identity and comparison traps;
- branch suffix movement through lexical declarations and `finally`;
- switch fallthrough and labeled exit behavior;
- devirtualized call receiver and argument evaluation order;
- dynamic imports and chunk dependency placement;
- property reflection and prototype mutation.

## Already stronger or better suited in LilScript

Do not replace these with Closure's simpler mechanisms:

- exact complete-artifact raw/gzip/Brotli scoring;
- incumbent-preserving bounded candidate search;
- typed SSA plus effect, range, ownership, and escape analyses;
- scalar/positional aggregate representations;
- CFG liveness and phi-aware local allocation;
- n-gram and codec-window-aware function layout;
- explicit chunk deployment objective.

## Do not copy

- semantic deviations accepted for historical compatibility;
- `GETELEM` getter-purity assumptions without a language guarantee;
- constructor/enum property collapse explicitly described as unsafe;
- raw-size-only string aliasing as a default;
- uncosted duplication of long constants;
- dynamic-import roots taken from unreachable importers without checking linked reachability;
- cross-chunk method stubs without separately measuring startup and aggregate bytes;
- runtime-regressing function factories unless a size-only profile explicitly selects them.
