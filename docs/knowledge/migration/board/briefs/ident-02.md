# Brief — ident-02

For a subagent. Written before spawning. The agent reads
[mission](../../../mission.md), this brief, and [notes/ident-02.md](../notes/ident-02.md) —
nothing else unless this brief names it.

## Task

Produce the inventory that `ident-02` needs: for every fold in
`src/js_peephole/folds/{copies,members,declarations,calls}.rs`, determine whether it can
move, copy, or rematerialize a **member read** (`obj.prop`) across a point where the
receiver name could be rebound. For each such fold, record which rebinding check it
consults today — `source_receiver_overwritten_between` (`copies.rs:781`), the local
`name_rebound` scan (`copies.rs:117–238`), the `this`/`arguments` guard
(`members.rs:238`), or none. Done = the table is written into the note and every fold in
those four files appears in it exactly once, including the ones that cannot move member
reads (marked "n/a" with a one-clause reason).

This is a **read-and-record** task. Do not change behavior.

## Why this matters to the objective

`obj.prop` names whatever `obj` currently is, so a rematerialized member read after a
rebinding silently reads a different object — the failure is undefined fields at the web
surface, not a size regression. The invariant has to hold as a class across every fold,
because the next port hits whichever fold is weakest, and until it holds, candidate
search is ranking programs that may be wrong.

## Read

- `docs/knowledge/mission.md`
- `docs/knowledge/migration/board/notes/ident-02.md` and `notes/ident-01.md` — including
  every REJECTED line; the "never reuse the receiver name" coloring is already rejected
- `src/js_peephole/folds/copies.rs` — especially `:117–238`, `:781–900`, `:2044`
- `src/js_peephole/folds/members.rs` — especially `:190–330`
- `src/js_peephole/folds/declarations.rs`, `src/js_peephole/folds/calls.rs`

## May touch

- `docs/knowledge/migration/board/notes/ident-02.md` — append only.

Everything else is read-only for this agent. No source edits in this task.

## Must not

- The [standing refusals](../README.md#standing-refusals) — no glue, no post-minify, no
  weakened gate, no `diag` number in a claim, semantics before size.
- Do not propose forbidding receiver reuse globally; that is REJECTED in `ident-01`
  (it perturbed unrelated spelling and produced invalid `?:break`).
- Do not refactor toward a shared check yet. The inventory is the deliverable; a
  refactor built on a guessed inventory is the thing this task exists to prevent.

## Prove it

```sh
grep -n "^pub(crate) fn fold_\|^fn fold_" src/js_peephole/folds/{copies,members,declarations,calls}.rs | wc -l
```

Expected: the row count of the table in the note equals that number, and each row names
a check or an explicit "n/a" reason.

## Report

Append to `docs/knowledge/migration/board/notes/ident-02.md`: the inventory table, one
Evidence row per command run (with the `gate`/`diag` tag), and one Log line ending in
OPEN, LANDED, or REJECTED. Then return at most 20 lines: how many folds can move member
reads, how many are unguarded, and which single fold looks weakest. Do not edit
`LEDGER.md`.
