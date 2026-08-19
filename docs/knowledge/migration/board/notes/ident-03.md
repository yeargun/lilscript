# ident-03 — catch identity bugs without a library

Parent: [ledger](../LEDGER.md). Status: todo.

## Question

Can the differential oracle produce receiver-rebinding shapes on its own, so this class
is caught by the test suite instead of by a 660-case parser port?

## Current hypothesis

Yes, and cheaply: the shapes are small — copy a member, rebind the receiver, read a
second member; vary with property writes, computed access, `delete`, closures capturing
the receiver, and rebinding inside a nested function (the guard walks past nested
functions today, `copies.rs:790`-ish — confirm that is sound, not merely convenient).

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-19 | Oracle binary present | `ls target/debug/lilscript-differential` | present | diag |

## Log

- 2026-08-19 — Opened. Motivation: marked is currently the only thing that finds these,
  which makes the feedback loop a whole port long. — **OPEN**

## Next step

Read `src/bin/lilscript-differential.rs` and `docs/differential-testing.md`, then add
the smallest generator that emits receiver-rebinding programs. Record whether it
reproduces the `ident-01` shape before any fix is applied — a fuzz lane that cannot
find the known bug proves nothing.
