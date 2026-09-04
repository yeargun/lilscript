# 006 — The candidate search

Parent: [index](index.md). Directive: [D9](001-directives.md#d9--record-cost-with-size).

The search is this compiler's crown jewel and the migration must not damage it. This document says
exactly which part is already correct, which part is the problem, what the migration must preserve
byte-for-byte, and where the compile-time win comes from.

---

## Two of the three tiers are already clean

| Tier | What it does | State |
|---|---|---|
| **IR-variant probe** | clones the module per optimizer-option tuple, runs the full SSA pipeline, emits one artifact each | **clean** — proposes `IrJsOptions`, re-emits from IR |
| **Plan / beam** | Cartesian product of emission options seeded from the decision registry, then scored families mutate the frontier along one axis | **clean** — same |
| **Terminal** | everything from `finalize_javascript_candidates_with_parallelism` onward | **100% text**, and it produces the bytes that actually ship |

So the migration's scope inside the search is precisely one tier. **33 text-operating entry points
across seven groups** were enumerated. That is the work list, and nothing above it needs to change.

This matters for risk: the part of the search that decides *which programs to consider* is untouched.
Only the part that decides *how the chosen program is spelled* moves onto the tree.

---

## Three mechanisms that must not be touched

Determinism here is engineered, not incidental, and the thread-invariance test asserts more than
output equality — it asserts identical `plan_identity`, identical optimization reports, and the
entire `selection_metrics` struct. **The amount of work done is part of the contract.**

1. **Coordinator-serial identity assignment.** `plan_identity` is `(context_id, ordinal)` where
   `ordinal` is the count of plans already registered for that context. Assigned on the coordinator
   before any parallel work. Must keep exactly this shape.
2. **Deterministic budget-prefix reservation.** Every parallel section reserves its prefix on the
   coordinator first; workers only measure. This is what makes `&mut` budget accounting race-free.
3. **Total orders that terminate in a lexicographic comparison of the artifact text.**

The third has a direct consequence for the representation:

> **The target AST must serialize to exact bytes on demand, and the serialized byte string remains
> the final tiebreak key in every candidate ordering.**

A tree that can only be compared structurally cannot replace this. Printing is not an output step
bolted on at the end — it is part of the ordering relation, and it must stay cheap enough to be
called in a comparator.

### The contract that must be versioned, not preserved

`selection_metrics` records how much work the search did. This migration **will** change that —
fewer lex calls, fewer emissions, different starvation. So the thread-invariance test's contract must
be re-baselined per phase rather than held fixed, and the re-baseline is part of the phase's gate,
declared in advance. What must never change within a phase is that two thread counts produce the
same metrics.

---

## Migration order inside the search

Fixed by dependency, not preference:

1. **The six folds fused into `emit_javascript_candidate` go first.** They are on the hot path —
   every one of ~300 candidates pays six whole-artifact re-scans — and they are the cheapest thing
   to prove byte-identical.
2. **The admission / ABI derivation goes second.** `validate_observed_javascript_artifact_allowing`
   re-derives export names, witnesses, arities and static imports *from the emitted text*. On the
   tree these come from the structure that produced them. Its `direct_source` comparison semantics
   must be preserved exactly.
3. **The naming and remap family goes last** — ~9 remap entry points plus the coordinate descent.
   It produces the shipped bytes, it is the largest remaining Brotli lever, and it may only move
   once the AST can express and score a scope-local renaming **without re-serializing**.

Two structures must survive the move intact:

- The **two-level plan → declaration-leaf** shape: a plan carries a partial `[usize; 4]` score ledger
  over its declaration spellings, its rank is the minimum, and budgets count plans, not leaves.
- **Declaration-kind selection** moves from string surgery to an AST/option axis — and must keep
  producing exactly the same ≤4 spelling family, in the same order.

One trap: `resolved_one_byte_binding_count` looks like telemetry and is not — it is a **control-flow
key**. It must be re-derived from the AST, not from text.

---

## Where the compile-time win comes from

Measured today, on a **171-byte** artifact: 371 plans registered, 366 emissions attempted, both
budgets exhausted, 1.5 s wall. CPU split across 8 threads: emit 1,176 ms / 375 calls, codec
1,493 ms / 332 calls, peephole 722 ms / 103 calls.

Three separate wins, in the order they land:

### 1. Idle work disappears

**97.4% of fold invocations rewrite nothing** (12,293 idle vs 330 active on that run). A fold whose
enabling syntax is absent still pays a full re-lex and a full bracket-table build today, because the
fold signature is `Fn(&str) -> …` and 134 folds each call `lex()` themselves.

On a tree, a transform is driven by node kind: a pass over a program containing no classes visits no
class nodes. The idle cost is not reduced — it stops existing.

### 2. Emission stops being superlinear

Emission measures 380 ms at 48 KB output and 1,301 ms at 98 KB — **3.43× for 2.05×**, roughly
O(n^1.7) in the size band the corpus lives in. At least one confirmed cause is structural rather than
incidental: loop spelling is chosen by counting `for(` versus `while(` substrings **across the whole
accumulating output buffer**, per loop emitted, matches inside string literals included.

The general defect is the emitter making decisions by re-reading text it just wrote — the same
defect as the peephole, one layer earlier, across ~1,487 substring-scan sites. On a tree these
decisions read a field.

### 3. Candidates share a base instead of re-emitting

Today every candidate is a full re-emission from IR. Once the tree exists, candidates that differ
only in spelling or in a local rewrite derive from a shared base. The design constraint that makes
this safe rather than merely fast is in [003](003-target-representation.md): a derived candidate
that changes nothing must cost **one walk and zero allocations**, and re-entry/caching decisions may
**never** change bytes.

### The memos stay

They are load-bearing and correct. Disabling the content-addressed memos on that run took lex calls
from 1,256 to **34,367** (27.4×) and wall from 1,507 ms to 4,021 ms — with **byte-identical output**.
Hit rates: analysis 95%, codec 42%.

Nothing in this migration removes them. The tree makes them *less necessary*, which is the point —
but they are proven selection-neutral and they stay until the tree has demonstrably subsumed them.

---

## Budgets must be re-derived, not inherited

Today's budget policy is calibrated against today's cost model: full proposals through 16 KiB, ¼ at
64 KiB, 1⁄12 at 256 KiB. On a 171-byte artifact the search **exhausts both budgets** and reports
**32 of 40 admitted families starved**.

That is the real headline for the owner's "more room to optimize": the search is not budget-limited
by ambition, it is budget-limited by how expensive a candidate is. If emission gets several times
cheaper, the same wall-clock buys a wider frontier — but only if the ladder is re-measured rather
than assumed.

So: the budget curve is **re-derived from measurement after the emission cost changes**, as its own
phase with its own fleet A/B. Carrying today's constants forward onto a cheaper emitter would leave
most of the win unclaimed, and raising them by guess would spend it badly. Both are failures.

---

## The admission gate keeps an independent parser

Per accepted candidate the gate performs **7 independent whole-artifact re-derivations** (syntax
floor, export names, export witnesses ×2, static imports, `|0` count, static property names ×2) —
494 whole-artifact contract checks on that 171-byte run.

On the tree, most of these become structural facts rather than re-derivations, and the cost largely
disappears. But **an independent standards-grade parser stays in the pipeline** — at two fixed
positions rather than as a per-leaf filter.

This is deliberate defence in depth and it is not redundancy. The tree proves the compiler built what
it intended; oxc proves the bytes actually say that. A printer bug is exactly the failure the tree
cannot catch about itself, and this compiler has already shipped three programs that were valid
JavaScript meaning the wrong thing.
