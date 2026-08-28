# ident-02 — make the receiver invariant a class, not a call site

Parent: [ledger](../LEDGER.md). Status: landed. Depends on [ident-01](ident-01.md).
[ident-05](ident-05.md) has landed.

## Question

Which folds can delay, copy, or rematerialize a member read — and does every one of
them consult the same receiver-liveness check?

## Current hypothesis

Rematerialization of a member read is illegal after the receiver is rebound **or**
after that property is written, without forbidding receiver-name reuse. Peephole
sites that cross a gap share `source_receiver_overwritten_between` in
`src/js_peephole/liveness.rs`. The JS emitter's expression cache is the same class:
a cached `obj.prop` must be snapshotted before `obj.prop =` (not dropped, and not
held across the store).

Sibling writes (`obj.src =` after a snapshot of `obj.href`) still rematerialize.
Identifier copies of the object (`d = b; b.href = x; return d`) still fold.

An invoked closure that *rebinds* a captured receiver is not this task; the
expression cache does not flush across that call. That is [ident-03](ident-03.md).

## Inventory (2026-08-28)

Folds that **can rematerialize a member or identifier-rooted expression** across
a gap:

| fold | file | crosses statements? | shared check |
|---|---|---|---|
| `fold_single_use_temporaries` | `returns.rs` | next statement | yes |
| `fold_single_use_if_assigns` | `control.rs` | rest of `if` body | yes |
| `fold_identifier_copies` | `copies.rs` | until copy rebound | yes (source name) |
| `fold_typeof_identifier_caches` | `copies.rs` | until last use | yes |
| `fold_statement_assignments_into_first_use` | `copies.rs` | next statement | yes |
| `fold_returned_temporaries` | `returns.rs` | adjacent only | no (gap empty) |
| `fold_sequence_assignments_into_first_use` | `copies.rs` | same paren seq | yes |
| `fold_copied_member_presence` | `boolean.rs` | same statement | no (cond and copy are the same tokens; receiver is a fresh temp) |
| `fold_single_use_literals` / regex | `copies.rs` | uses of a literal | n/a (not a member) |
| expression cache | `codegen_ir_js.rs` | until use | `materialize_cache_before_object_member_write` |

`fold_single_use_function_values`, `fold_identity_arrow_iife`, and inlining move
function literals (ident-05), not `obj.prop`.

## Constraints specific to this task

- One shared check. Not three that agree today.
- Each rematerialization site gets its own test, so a later refactor cannot silently
  drop the call.
- Keep the byte wins: the check refuses *rematerialization*, never receiver reuse.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-19 | Fold inventory (candidate sites) | `grep -n "^pub(crate) fn fold_" src/js_peephole/folds/{copies,members,declarations}.rs` | 17 in `members.rs`, 16 in `copies.rs`, 6 in `declarations.rs` | diag |
| 2026-08-28 | Re-inventory after members.rs collapse | inspect `copies.rs`, `returns.rs`, `control.rs`, `boolean.rs`, `members.rs` | `source_receiver_overwritten_between` gone; `members.rs` is coercions only | diag |
| 2026-08-28 | Shared check at rematerialization folds | `cargo test --lib rematerialization_folds_refuse` | 2 passed | gate |
| 2026-08-28 | Property-write liveness | `cargo test --lib js_peephole::liveness` | 3 passed | gate |
| 2026-08-28 | Record field snapshot across a store | `cargo test --lib snapshot_of_a_record_field_survives_a_later_write` | passed (`77` then `75`) | gate |
| 2026-08-28 | Existing emitter rematerialization guards | `cargo test --lib does_not_rematerialize` | 2 passed | gate |

## Log

- 2026-08-19 — Opened from [ident-01](ident-01.md) once the guard proved to be a single
  call site rather than a shared rule. — **OPEN**
- 2026-08-28 — Inventory written before further refactors. The named helper from
  ident-01 is gone; rematerialization folds that cross a gap now call `liveness.rs`.
  Remaining: `fold_copied_member_presence` (same-statement temp), property writes
  without receiver assign, then ident-03. — **OPEN**
- 2026-08-28 — Property writes are in the shared peephole check (`obj.prop =`,
  `+=`, `++`, `delete`, prefix of a nested read, computed). Sibling / nested
  writes stay legal. The live production hole was the expression cache replaying
  `a.href??0` after `a.href=`; snapshots now bind before the store.
  `fold_copied_member_presence` stays n/a. Invoked captured rebind is ident-03.
  — **LANDED**

## Next step

None for this note. Continue as [ident-03](ident-03.md): seed receiver-rebinding
and property-write shapes in `lilscript-differential`, including the invoked
closure that rebinds a captured receiver (expression cache does not flush across
that call today).
