# Phase 03 — aggregates and typed wins

Parent: [migration](README.md). Language: [aggregates](../language/aggregates.md).

## Objective

Show where LilScript should beat ordinary JS objects: nominal structs, nested
structs, closed classes, integer enums. These are the "win scenarios" that make the
language purpose visible.

## Families

`canonical/aggregates/`, plus `canonical/wins/` for mixed programs that combine
several proofs.

If a case loses, the LilScript was written as TypeScript glue, or scalar replacement
/ layout search failed. Fix the compiler or rewrite the `.lil`; do not inflate the
JS.

## Exit

Named `lt` cases are strict in all three objective lanes. `le` aggregate cases still
must not lose.
