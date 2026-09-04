# 009 — Phases, gates and rollback

Parent: [index](index.md). Directives: [D3](001-directives.md#d3--compression-is-a-hard-constraint-and-byte-identity-is-the-only-clean-proof),
[D7](001-directives.md#d7--every-phase-ships).

Every phase below ships to `main`, green, with the ports building. Each states the **invariant it
establishes**, the **gate that proves it**, and the **rollback**. The order is fixed by dependency,
not preference — where three independent designs and three independent reviews converged on an
ordering, that is noted.

---

## The gate vocabulary

Three kinds of gate, and a phase declares which one it is claiming **before** it runs:

| Gate | Means | Used by |
|---|---|---|
| **IDENTICAL** | every artifact in the sweep matrix is byte-identical to the incumbent | phases 1–4, 6 |
| **DECLARED** | named configs may move bytes; every other config is IDENTICAL | phase 5, 7 |
| **BEHAVIOUR** | the failing-test set per port is unchanged or smaller | every phase |

There is no "within the noise floor" gate. The perturbation band for a semantically empty change is
roughly −125..+30 Brotli and the whole text layer is worth 189 bytes, so a neutral phase that moves
bytes has changed the program, not its spelling. Treat it as a correctness alarm.

**Canaries** (fast, clean, typed, high signal): cnlil, markedlil, posthoglil.
**Must-pass before a phase is done**: jquerylil, mobxlil, zodlil (at its shipped config), katexlil.

---

## Phase 0 — Repair the instrument

**Blocking. Nothing starts until this is green.** Full detail: [002](002-the-instrument.md).

- Make assertions real (`[profile.release-assert]`, env-gated release witness)
- Make the ports a gate (`portgate.mjs`, 61 configs, failing-set diff, under-10s = failure)
- Make the differential harness generative (random seed, classes/closures/prototypes in domain)
- Freeze the baseline on idle pool workers; record which ports are blind to which defect classes
- Fix Live-2 (one line); ship the Live-1 collision guard
- Close fleet gaps F1–F5 ([008](008-fleet.md))

**Gate:** reverting a known-good compiler commit turns the corresponding ports red. The harness
reproduces Live-2 without being told about it.
**Rollback:** n/a — all of it is additive and useful regardless.

---

## Phase 1 — The tree exists, and is proved against the incumbent

Build the target tree **alongside** the existing `String`, for expressions first. Every constructor
ends with a witness comparing `print(&node)` to the `code` it built. The output path is unchanged:
release still emits from `code`.

**Invariant:** the tree can reproduce the incumbent expression-for-expression.
**Why first:** a printer bug in this phase cannot reach an artifact. It can only fail an assertion.
That is a stronger guarantee than any measurement, and it is available for free at the start.

Two things the review caught and this phase must honour:

- The witness **cannot be a bare `debug_assert!`** — those are compiled out of every fleet build
  ([002 G5](002-the-instrument.md#what-is-broken-verified)). It is env-gated and runs in release,
  and it also rides `release-assert` so the 1,715 existing tests exercise it for free.
- Several current spellings are **not pure functions of the options** — they read back rendered text
  (`without_explicit_tostring` strips a `+""` suffix, `is_constant_literal` matches a rendered
  string). The witness asserts against a **quirk-preserving** printer, and each quirk is a named
  ledger entry that retires later as its own commit with its own A/B. Otherwise every quirk surfaces
  mid-migration as unexplained drift with no owner.

**Gate:** IDENTICAL (trivially — the output path did not move) + the witness passes on all 61 configs
+ BEHAVIOUR.
**Rollback:** delete the tree constructors. Nothing depended on them.

---

## Phase 2 — Statements, functions and the module

The 63 `out: &mut String` signatures move onto a block type. **Split into two commits**, per
[D7](001-directives.md#d7--every-phase-ships):

- **2a** introduces `type JsBlock = String` and changes all 63 signatures. A no-op diff that cannot
  conflict semantically and rebases trivially against concurrent sessions.
- **2b** replaces the alias with the real type, and deletes each escape method (`push_str`, `pop`,
  `truncate`, `insert_str`) in its own named commit as the last caller goes.

This is deliberately unglamorous. It is also where a single 38,325-line sweep would collide with the
other sessions editing this file, so the mechanical/semantic split is the whole point.

**Gate:** IDENTICAL + witness + BEHAVIOUR.
**Rollback:** 2b reverts to the alias; 2a is a pure rename.

---

## Phase 3 — The tree becomes authoritative

Delete `code`. The printer is now the only producer of bytes, and it is a pure total function.

**Consumes:** G1 (13 printer-hygiene folds) and G2 (9 repairs-for-repairs) — 22 folds that become
*unreachable*, not merely unnecessary ([005](005-printer-and-naming.md), [007](007-fold-disposition.md)).
Also deletes `repair_fused_keyword_identifiers` and the 3,904 lines of `keyword_space_tests.rs`.

**Also here:** the loop-spelling substring census and `take_trailing_expression_statements` become
running counters / recorded boundaries.

> **Corrected 2026-09-04, by measurement.** This paragraph called both of them "the confirmed
> superlinearity". Only one of them is worth anything. The census landed in phase 2b and is real —
> it rescanned the whole artifact twice per loop. `take_trailing_expression_statements` is **not**:
> instrumented as `trailing_scan` and measured on posthoglil, it is **5.1 ms across 165 calls and
> 0.72 MB rescanned**, against 31.2 s of wall time — 0.016%. It is quadratic in shape and irrelevant
> in size, because the blocks it rescans are function bodies, not the artifact.
>
> It still moves in phase 3, but for the invariant below (no production path re-reads emitted text),
> **not** for speed, and it should not be prioritised as if it were a performance fix.
>
> The same run says where compile time actually goes, per compile of posthoglil (CPU across threads,
> against 31.2 s wall): **codec 87.7 s**, **emit 60.1 s over 1,246 emissions**, peephole 8.3 s,
> analyze 8.3 s, lex 3.1 s over 28,801 tokenizations, optimize 1.3 s. The candidate search's Brotli
> encodes dominate everything else, which is [phase 7](#phase-7--candidate-derivation-and-budgets),
> not phase 3.

**Invariant:** no production path re-reads emitted text to make a decision.
**Constraint:** no `Raw` / escape-hatch variant survives in the node enum. A construct that cannot be
modelled is a design finding, not a variant. Keep it behind a `cfg` feature until the phase ends so a
concurrent merge cannot reintroduce one, then delete it.

**Gate:** IDENTICAL + BEHAVIOUR + the fold-deletion protocol
([007](007-fold-disposition.md#what-delete-has-to-prove)) for all 22.
**Rollback:** this is the first irreversible phase. Rollback is a revert of the phase branch, which
is why 1 and 2 must be fully green first.

---

## Phase 4 — Deliver the facts

Annotations land: the 16-bit inline flag word, `Arc`'d side tables keyed by origin, and provenance
made load-bearing (`NodeId` as the origin key; the 17 `node_id: None` sites become build failures).
`FunctionEffectSummary`, `FiniteValueAnalysis`, array-parameter lengths and the escape graph cross
the emission boundary for the first time. Detail: [010](010-what-this-unlocks.md).

**Invariant:** a target pass reads a proven fact rather than guessing one.
**Watch:** a property of a *use* never lives on a shared node. `BOOL_CTX` and `SINGLE_USE` are
threaded through the traversal context, not cached.

**Consumes:** G6 (int32 coercions — 1,554 lines become a field test) and unblocks G4, G5, G10, G11.

**Gate:** IDENTICAL — delivering a fact must not change what is emitted. A byte change here means a
pass started using the fact, which belongs in phase 6 with its own measurement.
**Rollback:** annotations are additive; passes that consume them land later.

---

## Phase 5 — Naming moves post-layout

**The first deliberately byte-changing phase**, and the largest single lever
(katexlil's identifier stream, +2,113).

Ordered **after** phases 2–4, not before: a post-layout naming pass needs a finished module with a
complete scope tree. All three candidate designs converged on this once the sequencing error was
caught.

Lands as separately scored decisions, each defaulted off, each its own registry row and its own fleet
A/B: `NameOrdering::{EmissionWalk, FrequencyDesc, IdiomConverged}` and `ReservationMode::Precise`.
`EmissionWalk` must reproduce the incumbent byte-for-byte and is the anchor.

**Deletes:** `rename.rs`, the token-level `BindingResolution`, the third `Mangler`, and
`rename_ambiguous` (14/14 on cnlil).

**Gate:** DECLARED — `EmissionWalk` is IDENTICAL on all 61 configs; each other ordering is a separate
A/B. Plus the **name-request-order trace** gate: a diff in the `(order, name)` sequence fails the
phase *even when bytes match* ([005](005-printer-and-naming.md#the-determinism-trap)).
**Rollback:** every new ordering is off by default; reverting is a config default, not a code revert.

---

## Phase 6 — The fold groups

The remaining eleven groups, in the order and by the two-commit protocol in
[007](007-fold-disposition.md). Before the scored families (G9, G10, G13) open, the
`ShapeTransform { sites, apply(subset), polarities }` type must exist — otherwise the backend becomes
always-emit-best-shape and **will** regress ports.

Within the search, the internal order is fixed: the **six folds fused into
`emit_javascript_candidate`** first (hot path, cheapest to prove), then the **admission/ABI
derivation**, then everything else ([006](006-candidate-derivation.md#migration-order-inside-the-search)).

G8 (loop headers) is high value despite being hard: carrying `Latch` from
`ControlShape::Loop { update }` closes two of the three shipped wrong-program folds by construction.

**Gate:** per group — `active == 0` for every fold on every config with the fold still enabled,
IDENTICAL, unchanged scored-candidate count, then a separate deletion commit with no byte change.
**Rollback:** per group, and the two-commit protocol means the emitter change and the deletion revert
independently.

---

## Phase 7 — Candidate derivation and budgets

Candidates derive from a shared base rather than re-emitting from IR. Then, and only then, the budget
ladder is **re-derived from measurement** — today's curve is calibrated against today's cost model,
and the search currently exhausts both budgets on a 171-byte artifact with 32 of 40 families starved
([006](006-candidate-derivation.md#budgets-must-be-re-derived-not-inherited)).

**Invariant:** a derived candidate that changes nothing costs one walk and zero allocations; a
re-entry/caching decision may **never** change bytes.
**Watch:** `selection_metrics` is part of the thread-invariance contract and this phase changes it.
Re-baseline as part of the gate, declared in advance.

**Gate:** DECLARED for the budget change (its own A/B); IDENTICAL for the sharing change at fixed
budgets. Plus D9 resource reporting per port.
**Rollback:** budgets are config; sharing reverts to per-candidate emission.

---

## Phase 8 — Retire the text layer

Delete `token.rs`, `scope.rs`, `binding.rs`, `liveness.rs`, `parse.rs` and the remaining folds.
`repair_late_javascript_candidate` and its duplicate list in `emit_javascript_candidate` go with them.

**Exit criteria** (inherited from `planned-migration.md` Phase 3):

- production never reparses generated text to discover binding or owned-property identity
- final bytes are still independently parsed and checked against intended identities, ABI, syntax
  floor and live obligations before scoring — **the oxc gate stays**, at two fixed positions
- no production path calls `repair_late_javascript_candidate`
- the full phase 0 gate set is green and resource budgets are accepted

**What remains, honestly:** G12 (class recovery from prototype tables, 11 folds, 8,017 lines) is the
one group a tree does not subsume — it recovers structure from *source* idiom, not from emitter
mistakes. It ports to the tree last, stays scoped and named, and shrinks as ports become typed. Do
not let it block phases 1–7.

---

## Rollback discipline

- Phases 0–2 are fully reversible.
- Phase 3 is the commitment point. Everything before it must be green on all 61 configs first.
- Every byte-changing decision after that is a **default-off registry row**, so a regression found
  later is a config flip, not a revert.
- The two-commit fold protocol means an emitter change and its fold deletion revert independently.

If a phase cannot state its gate in the vocabulary above before it starts, it is not yet designed.

---

## What "done" means

The migration is complete when, on the full 61-config sweep on idle pool workers:

1. no production path re-parses generated JavaScript,
2. every port's Brotli is **at or below** its Phase 0 baseline,
3. every port's failing-test set is unchanged or smaller,
4. `emit_ms` and `lex_calls` are materially down and reported per port,
5. and the correctness matrix in [004](004-legality-by-construction.md) shows every known
   wrong-program class as unrepresentable or a compile error — not merely "fixed".

Point 5 is the one that matters. The rest is bookkeeping.
