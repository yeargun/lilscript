# ident-02 — make the receiver invariant a class, not a call site

Parent: [ledger](../LEDGER.md). Status: todo. Depends on [ident-01](ident-01.md).

## Question

Which folds can delay, copy, or rematerialize a member read — and does every one of
them consult the same receiver-liveness check?

## Current hypothesis

They do not. `source_receiver_overwritten_between` (`copies.rs:781`) has one caller.
`copies.rs` also carries an independent `name_rebound` scan (`:117–238`), and
`members.rs` has its own narrower guard for `this`/`arguments` (`members.rs:238–239`).
Three spellings of one invariant means at least one of them is weaker than the others,
and the weakest one is the bug the next port will hit.

## Constraints specific to this task

- One shared check. Not three that agree today.
- Each rematerialization site gets its own test, so a later refactor cannot silently
  drop the call.
- Keep the byte wins: the check refuses *rematerialization*, never receiver reuse.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-19 | Fold inventory (candidate sites) | `grep -n "^pub(crate) fn fold_" src/js_peephole/folds/{copies,members,declarations}.rs` | 17 in `members.rs`, 16 in `copies.rs`, 6 in `declarations.rs` | diag |

## Log

- 2026-08-19 — Opened from [ident-01](ident-01.md) once the guard proved to be a single
  call site rather than a shared rule. — **OPEN**

## Next step

Enumerate, for each fold in `copies.rs`, `members.rs`, `declarations.rs`, and
`calls.rs`, whether it can move a member read across a statement boundary. Write the
list into this note **before** editing anything — that inventory is the deliverable
that survives the next context, even if the refactor does not land in one sitting.
