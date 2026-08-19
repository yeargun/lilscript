# ident-01 — a delayed member read must not outlive its receiver

Parent: [ledger](../LEDGER.md). Status: landed.

## Question

When a fold defers or rematerializes `obj.prop`, does the emitted JS still read the
property of the object the source meant — after `obj` has been rebound in between?

## The shape

```js
obj = obj.title;   // receiver rebound
…
f(obj.href);       // rematerialized member now reads title.href → undefined
```

`obj.prop` names whatever `obj` currently is. This is not a marked bug and not a
parser bug: it is every `extern class` and every DOM field spelled the same way. A
fix scoped to one library, or to one call site, is not a fix.

## Current hypothesis

The compression search is allowed to overwrite a dead receiver — that is a legitimate
byte win and must stay reachable. The invariant is narrower and harder:

> A delayed member read may not outlive the name it reads through.

So the fold refuses to *rematerialize* after a rebinding, rather than the emitter
refusing to *reuse* the name.

## Constraints specific to this task

- Do not forbid receiver reuse globally. That was tried; see the REJECTED line below.
- Do not special-case a host object, a library, or a property name.
- The `.lil` side is not on trial here. This is a JS-emission invariant.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-19 | Guard exists and is reachable | `grep -n "source_receiver_overwritten_between" src/js_peephole/folds/*.rs` | defined `copies.rs:781`, **one** caller `copies.rs:2044` | diag |
| 2026-08-19 | Ad-hoc sibling scans | `grep -n "name_rebound" src/js_peephole/folds/copies.rs` | separate rebinding logic at `copies.rs:117–238` | diag |
| 2026-08-19 | Build health | `cargo check --quiet` (cargo 1.97.1) | exit 0, no warnings | diag |

## Log

- 2026-08-19 — Stronger approach: color the receiver so a rebound name is **never**
  reused. Backed off. It perturbed unrelated spelling and produced invalid `?:break`
  output (a statement in expression position). — **REJECTED**, but the invalid
  emission it exposed is real and moved to [emit-01](emit-01.md); it is not caused by
  the coloring, only revealed by it.
- 2026-08-19 — Narrower approach: the fold refuses to rematerialize a member after the
  receiver is rebound or that property is written — `source_receiver_overwritten_between`
  in `copies.rs:781`. Semantics-correct at its one call site. — **OPEN**, incomplete:
  one caller is not a class. Continues as [ident-02](ident-02.md).
- 2026-08-19 — The loop-carried half of the same family fixed in
  `safe_two_address_phi_pairs`; see "What actually landed". Regression test
  `keeps_a_saved_previous_value_readable_across_its_own_update` in `src/compiler.rs`
  was checked *against the unfixed compiler first* and fails there. Suite unchanged at
  57 pre-existing failures, 0 new. marked went 659/660 → 660/660. — **LANDED**
- 2026-08-19 — Rejected as a category: grouping helpers, host-name special cases, and
  any per-library patch around this. — **REJECTED**

## What actually landed

The live bug was one step earlier than the fold: **SSA two-address coalescing**, in
`safe_two_address_phi_pairs` (`src/codegen_ir_js.rs`). A loop phi was allowed to share
a JavaScript name with its own incoming value even when the phi's result was copied
into a second phi on the same edge — the textbook lost copy. `prev = cur; cur = …`
became one name, the header compared the new value against itself, and the body ran
exactly once.

Three changes, all restrictions:

1. `parallel_copy_phi_results` — a phi result that is itself copied on one of its
   incoming edges can never be two-address coalesced. Applied once over the finished
   pair set, so every site that proposes a pair is covered rather than the one that
   was noticed.
2. The phi-to-phi case now also proves the phi result is dead once the incoming value
   exists; it previously proved only that the incoming was fresh.
3. `target_is_unused_until_phi_redefinition` takes the first index to scan, so a phi
   definition (which has no instruction index) can use the same check.

marked's GFM autolink backpedal is exactly this loop: it trimmed one `)` instead of
looping to a fixpoint, and `gfm.0.29.json#625` was the single failing spec case.

## Next step

None for this note. The unfinished half is [ident-02](ident-02.md) — the receiver
rematerialization guard is still a single call site — and the new blocker is
[ident-05](ident-05.md).
