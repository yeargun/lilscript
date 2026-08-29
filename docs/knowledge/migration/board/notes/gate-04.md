# gate-04 — final-artifact admission

Parent: [ledger](../LEDGER.md). Status: active.

## Question

Can every incumbent and challenger be rejected before codec scoring when its
printed JavaScript has invalid syntax, unresolved or changed bindings, an
unclassified property, a broken module edge or ABI element, or a dropped lowering
obligation?

## Current hypothesis

The existing parser, binding resolver, compilation contract, and source ABI
manifest can supply a first mandatory admission path without introducing the
planned target-JS tree early. Missing witnesses must fail closed.

## Constraints specific to this task

Validation runs before every exact codec call and applies equally to incumbents
and challengers. Final-byte parsing is an independent check, not a new source of
semantic identity. Do not weaken a rejection to preserve bytes.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-29 | Predecessor G2 | `node comparison/large-libraries/run.mjs --run --compiler migration,candidate ...` | every current candidate boundary passed; the invalid incumbent Marked raw/gzip artifacts were rejected by fresh semantics | gate |

## Log

- 2026-08-29 — Gate-02, V-02, and V-03 landed. V-01 is now the first open canonical phase-0.5 unit. — **OPEN**

## Next step

Inventory every codec call and existing final-JavaScript parser/resolver entry,
then add a single fail-closed admission primitive with a malformed/unresolved
candidate rejection test.
