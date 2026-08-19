# ident-04 — freeze identity as paired canonical folders

Parent: [ledger](../LEDGER.md). Status: todo. Depends on [ident-02](ident-02.md).

## Question

What is the permanent, reviewed record that this class stays fixed?

## Current hypothesis

A `canonical/identity/` family under `comparison/cases/`, one folder per shape, mostly
`expect = "le"`. These are correctness-first cases: the point is that stdout matches
and the compressed size does not regress, not that LilScript wins. A `lt` here would
have to name a typed advantage, per
[the migration working rules](../../README.md#working-rules).

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| — | — | — | not started | — |

## Log

- 2026-08-19 — Opened as the durable half of the identity lane. — **OPEN**

## Next step

After `ident-02` lands, write one folder per shape from the `ident-03` generator,
following [case layout](../../../verification/case-layout.md), and run
`node comparison/cases/run.mjs --only identity/`.
