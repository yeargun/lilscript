# 005 — The printer and naming

Parent: [index](index.md). Consumes fold groups G1, G2 and G14 from [007](007-fold-disposition.md).

Two subsystems, one document, because they are the same insight seen twice: **a decision made before
the information exists is a decision made badly, and then repaired.**

---

## Part 1 — Naming

### The defect, in the code's own words

`src/js_peephole/rename.rs:5-9`:

> "Names assigned in the IR backend **cannot converge**: they are chosen per function before the
> nesting layout is known … This pass runs on the final laid-out text, where `BindingResolution`
> knows exactly which declaration every identifier refers to."

That is a complete and correct diagnosis of an **ordering** bug, and the response was to build a
second naming system on the far side of the print boundary — with its own tokenizer, scope model and
binding resolver — because that is where the information finally exists.

The fix is not a better second system. It is to assign names **after layout, on the tree**, where
binding identity was never lost in the first place.

### What that deletes

- `rename.rs` entirely, and the third `Mangler` in the tree (after `codegen_ir_js.rs` and
  `codegen_js.rs`), forked deliberately so the pass "depends only on the token stream"
- the token-level `BindingResolution` — replaced, not reimplemented
- `rename_ambiguous`, which fired on **14 of 14 candidates on cnlil** because the emitter's own
  duplicate `var t` bindings are indistinguishable in text

That last item is the lever, not a cleanup. The identifier stream is where **katexlil's remaining
+2,113 bytes** live, and the pass that would close it is currently blocked by an ambiguity that
exists only because identity was discarded at the print boundary.

### The determinism trap

`Mangler::next_name` is a bare counter. Identifiers are therefore a function of **request order**,
and a tree-then-print architecture naturally reorders requests. A migration that changes name request
order silently changes every identifier in the artifact — which then reorders function layout under
`FunctionLayout::CompressionSimilarity`, because that ranks functions by an 8-gram profile of their
*printed text*. One reordered request can cascade into a completely different artifact.

So naming carries a gate stronger than byte equality:

> **The name-request-order trace is a phase gate.** Emit the `(order, name)` sequence under the
> witness build and treat a diff in it as a failure **even when the final bytes match.**

Bytes matching with a different request order means the two systems disagreed and coincidentally
landed in the same place. That is not evidence; it is luck, and it will stop holding on the next port.

### Ordering within the migration

Naming moves **after** statements, functions and module structure are on the tree — not before.

This corrects the sequencing error the design review caught in one of the candidate plans: a
post-layout naming pass needs a finished module with a complete scope tree, and that does not exist
while 63 functions still emit statements into a shared `String` buffer. All three independent
designs converged on the same order once the flaw was pointed out.

### And it is a byte-changing phase, so it is split

Post-layout naming is the one phase in this migration that **deliberately** changes bytes. Against a
semantically-empty perturbation band of roughly −125..+30, a single knob flip measured on the fleet
proves nothing.

So it lands as **separately scored decisions, each defaulted off, each with its own registry row and
its own fleet A/B**:

- `NameOrdering::EmissionWalk` — reproduce today's order exactly (the byte-identity anchor)
- `NameOrdering::FrequencyDesc`
- `NameOrdering::IdiomConverged`
- `ReservationMode::Precise` — separate axis, separate measurement

The first must reproduce the incumbent byte-for-byte. The others are scored against it like any other
alternative, which is what
[D3](001-directives.md#d3--compression-is-a-hard-constraint-and-byte-identity-is-the-only-clean-proof)
requires and what the existing search machinery already knows how to do.

---

## Part 2 — The printer

### A pure total function

```rust
fn print(tree: &TargetTree, names: &NameAssignment) -> String
```

No decisions left. Every choice that could be made — grouping, spelling, separator, terminator — was
made when the node was built or when the name was assigned. The printer walks and appends.

This is the property that makes byte-identity checkable at every commit, because it makes the
artifact a function of the tree rather than of the order in which the emitter happened to visit it.

### What "by construction" removes

**Token separation.** `repair_fused_keyword_identifiers` exists because some rewrite fuses a keyword
into an identifier and produces `returna`. A printer that emits tokens knows whether two adjacent
tokens need a separator; the failure is unreachable. This deletes the repair *and* the 3,904 lines
of `keyword_space_tests.rs` that exist to police it.

**Grouping.** `fold_redundant_and_parens` drops parentheses the printer put there. It fires **68 of
75 times on markedlil** — the single clearest symptom of the half-AST: `JsExpression` carries
`precedence` and `ungrouped`, yet the authoritative `String` already has the parens baked in. A
printer that computes grouping from child precedence at print time never emits a redundant paren.

**ASI.** `elide_asi_safe_semicolons` fires **77 of 77 times** — every single emission. The printer
knows whether a statement needs a terminator by construction.

**The nine repairs-for-repairs (G2).** They repair states *a tree cannot enter*: a sequence with a
hole, a ternary with an empty arm, `async async`, a declarator another fold ate. Not deleted —
unreachable.

Together G1 and G2 are 22 folds that stop existing rather than being ported.

### Decisions must stop being read back out of text

The printer's discipline only holds if the emitter stops making semantic decisions by re-reading its
own output. The confirmed worst case is loop spelling:

```rust
// src/codegen_ir_js.rs — inside LoopSpelling::Auto, per loop emitted
out.matches("for(").count() > out.matches("while(").count()
```

Two full scans of the entire accumulating buffer, per loop, counting matches **inside string
literals** too. This is both the O(n²) and a correctness smell: a decision about program shape taken
from a substring census of unrelated text.

It generalizes. Roughly **1,487 substring-scan sites** exist in the emitter; the ones that make a
*semantic* decision from rendered text — `without_explicit_tostring` stripping a `+""` suffix,
`is_constant_literal` matching on a rendered string, `host_wrapper_inline_is_profitable` testing
`code.contains("typeof")`, `arrow_block_as_object_method` using `code.find("=>{")` — are each a place
where a string literal, template or regex in user code can make the test fire wrongly.

Every one becomes a field test on the tree. The migration tracks them as a closing count, per phase.

**And `take_trailing_expression_statements`** rescans the buffer from byte 0 for every statement it
peels — O(k·n) where the writer already knew the answer. The tree fix is structural; until then it is
a running counter, and it is scheduled in Phase 0 rather than at the end, because it is byte-neutral
and pure win.

### Dense ids delete the determinism scaffolding

`src/codegen_ir_js.rs` carries **57 explicit sorts whose only job is making hash-map iteration
deterministic**, and `StableHashMap` exists because iteration order is observable in the generated
program.

Make node, binding, atom and scope ids **dense and `Ord`**, and key every emission-path map by them —
a `Vec` indexed by id, or a sorted `Vec<(id, T)>`. The sorts and the stable-hash types then delete,
and determinism stops being a discipline that 57 call sites have to remember.

### The print memo

Function layout under `CompressionSimilarity` prints each function separately, profiles it and orders
by DP, so **per-function text is the natural memo unit**.

The key must be `(node identity, digest of the spellings of the bindings reachable from that
subtree)` — **not** a global name revision counter. Naming is the dominant candidate axis, so a
global revision invalidates every function on every rename and the memo does nothing. Keyed properly,
a scope-local rename invalidates only the functions containing that binding, which is what makes
candidate derivation cheap ([006](006-candidate-derivation.md)).

### The printer stays inside the ordering relation

Candidate ordering terminates in a lexicographic comparison of artifact **text**. The tree does not
replace that — it must serialize to exact bytes on demand, and those bytes remain the final tiebreak
key. Printing is part of the comparator, not an output step, and it must stay cheap enough to be
called there.
