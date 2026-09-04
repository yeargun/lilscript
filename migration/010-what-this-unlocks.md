# 010 — What the tree unlocks

Parent: [index](index.md).

> "We need the absolute best — both compilation faster, and more room, more understanding in the
> compiler for optimization chances." — owner

This document is the answer to the second half. Faster compilation is a consequence of the
representation ([006](006-candidate-derivation.md)); *more room* is the reason to do it at all.

---

## The one number

**The IR proves 21 distinct classes of fact at the emission boundary. Exactly one crosses it.**

`IntegerValueAnalysis` is delivered, `Arc`-shared across all ~300 candidate emissions. Four complete
analyses are computed in the optimizer and **delivered nowhere**:

| Analysis | State | What it would license |
|---|---|---|
| `FunctionEffectSummary` | completed interprocedural fixpoint — `inherent`, `mutated_parameters`, `retained_parameters` | motion across calls, dead-store elimination through a call, safe reordering |
| `FiniteValueAnalysis` | built behind a flag, dies inside two optimizer passes | membership tests, total-ternary default elimination, switch shaping |
| Array-parameter lengths | computed, discarded | bounds-free indexing, loop-shape choices |
| Escape points-to graph | monotone lattice with a complexity proof | allocation sinking and literal fusion at the target level |

Everything else dies at the `String` boundary, and then 47,000 lines rebuild it from tokens — **worse**.

Three concrete symptoms of "worse", each measured:

- **Three purity oracles now exist.** Two are exact over typed IR; four token-kind guesses answer the
  same question in the text layer.
- **Obligation checking has degenerated into pattern counting.** `PreserveJavaScriptBitOrZero` — the
  mechanism that guarantees a source-written `value | 0` survives optimization — is verified by
  counting `|` followed by `0` token pairs and comparing `observed >= expected`. It cannot
  distinguish a source obligation from a compiler-generated normalization. A load-bearing correctness
  mechanism is implemented as a substring census.
- **A whole proof bundle is built and thrown away 310 times per compile.** `LocalNames` has 38
  fields, 28 of them `ValueId`-keyed fact tables, re-derived by ~24 independent whole-function scans,
  used to pick a spelling, then discarded — once per candidate emission on cnlil.

---

## What is currently refused for lack of proof

These are not hypotheticals. They are transforms the compiler already tries and declines, with the
decline measured:

**`fold_dead_pure_identifier_assigns`: 161 invocations, 0 fires.** It cannot prove a member read is a
data slot rather than a getter or a Proxy trap, so it declines every time. The IR states the fact
outright — `FieldGet { owner, index }` versus `RecordFieldGet` / `HostFieldGet` / `IndexGet`. All
four print as `a.b`, so the text layer must assume the worst about all of them.

**`rename_ambiguous` on 14 of 14 candidates on cnlil.** The rename pass cannot proceed because the
emitter's own duplicate `var t` bindings are indistinguishable in text. Two distinct bindings that
happen to share a spelling defeat the largest remaining Brotli lever — and the emitter *knows* they
are distinct, because it created them.

That second one is worth dwelling on: **the identifier stream is where katexlil's remaining gap lives
(+2,113 bytes)**, and the pass that would close it is being blocked by an ambiguity that exists only
because identity was discarded at the print boundary.

---

## The annotation design

Cheap, and bounded on purpose. Two tiers.

**Inline, as a 16-bit flag word on the node** — exactly eight predicates, chosen because they are
what every rewriting pass tests in its inner loop ("may I delete this", "may I move this across
that"). On cnlil, **11,085 of 13,606 fold invocations** test one of these:

```
PURE  NO_THROW  OWNED_SLOT  INT32  NON_NULLISH  LOCAL_ONLY  SOURCE_ORIGIN  HAS_OBLIGATION
```

**Everything else in `Arc`'d side tables**, keyed by the node's `origin` id (a packed
`FunctionId` + `ValueId`). Never inline, never variable-size.

The design is already proven in this codebase: `Arc<IntegerValueAnalysis>` is built once behind a
`OnceLock` and shared across every candidate emission. The migration generalizes the pattern it
already validated to the other twenty facts.

Two rules that keep it sound, both learned from the design review:

- **`HAS_OBLIGATION` is checked by construction, not by counting.** The obligation *kind* lives in a
  sparse table; the flag says one exists. This replaces the `|0` token census with a structural fact.
- **A property of a *use* never lives on a shared node.** Facts like "consumed only through
  ToBoolean" and "single use" are properties of a use, not of a value — caching them on a node that
  candidates share by identity is a staleness bug waiting to happen. They are threaded through the
  traversal context instead.

Marginal cost: approximately zero. The current half-AST already pays far more than 8 bytes per node
in duplicated rendered text.

---

## Provenance becomes load-bearing

Coverage is complete at lowering — all three `lower.rs` emit paths call `alloc_node_id`. It then
decays: **17 `node_id: None` sites** in `optimizer.rs` and `compress_passes.rs` drop provenance on
constructions the optimizer makes itself.

Making `NodeId` the AST node's origin key turns those from cosmetic gaps into build failures, and
that is the point. Once every node knows where it came from:

- **Obligation checking becomes exact** — a source `value | 0` is a node with an obligation, not a
  token pattern that survived.
- **Source maps become a byproduct** rather than a separate mechanism.
- **Diagnostics point at source**, including the ones the emitter currently cannot express.
- **A twin-run divergence names the node, its origin, and its span** instead of reporting a byte
  offset into an artifact nobody has read.

That last one is what makes the migration's own gate usable ([002](002-the-instrument.md)).

---

## The transforms that become available

Ordered by how directly the delivered facts license them:

| Fact delivered | Unlocks |
|---|---|
| `OWNED_SLOT` (owner/slot identity) | Dead-store elimination and read forwarding through member access — the 161-invocation/0-fire case, and most of G10/G11 in [007](007-fold-disposition.md) |
| Binding identity | Scope-local renaming without ambiguity; the identifier-stream lever; convergence across the whole module rather than per function |
| `FunctionEffectSummary` | Motion across calls; the current text layer must assume every call is opaque |
| Escape lattice | Allocation sinking and literal fusion at the target level, where the codec can score them |
| `FiniteValueAnalysis` | Membership tests, total-ternary default elimination, switch shaping |
| `I32Range` at the node | The 1,554 lines of `fold_int32_coercions` become a field test |
| `Latch` from `ControlShape::Loop` | Correct loop-header shaping — and it closes two of the three shipped wrong-program folds by construction |
| Exact arity / `receiver_use` | Arrow spelling that cannot capture `this` — closes [Live-2](002-the-instrument.md#live-2) |

---

## And a wider search, if it is re-measured

The search currently **exhausts both budgets on a 171-byte artifact** and reports **32 of 40 admitted
families starved**. It is not limited by ambition; it is limited by how expensive a candidate is.

Cheaper candidates buy a wider frontier — but only if the budget ladder is re-derived from
measurement rather than carried forward
([006](006-candidate-derivation.md#budgets-must-be-re-derived-not-inherited)). Carrying today's
constants onto a cheaper emitter leaves the win unclaimed; raising them by guess spends it badly.

---

## The honest limit

A tree does not make the compiler smarter about JavaScript. It makes the compiler **able to use what
it already proved**.

Every fact above is one the SSA IR computes today and discards at the print boundary. None of this
is new analysis — it is the removal of a wall between analysis and use. If the migration lands and
these transforms still do not fire, the reason will be that the facts were never as strong as the
analyses claimed, and that is worth knowing too.

One group genuinely needs new capability rather than delivered facts: **G12, class recovery from
prototype tables** ([007](007-fold-disposition.md#g12-is-the-exception-and-it-must-be-named-as-one)).
Those 8,017 lines recover structure from *source* idiom the IR never modelled. The tree improves how
that recovery is done; it does not remove the need for it. Typed ports do.
