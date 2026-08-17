# Phase 02 — scalar language core

Parent: [migration](README.md). Coverage ledger:
[coverage matrix](../verification/coverage-matrix.md).

## Objective

Build dense, independently reviewable cases for the scalar semantics every later
feature depends on. Favor small counterexamples that reveal codegen drift immediately.

## Required families

- integer overflow, division/remainder by zero, masked shifts, bitwise operators,
  prefix/postfix updates, compound assignments, and coercion elision;
- `number`, `int`, boolean, string, nullable, union narrowing, equality, relational
  ordering, `NaN`, `-0`, and truthiness where the language exposes them;
- literals, templates, regex literals, defaults, constant propagation, numeric/string
  pooling, and quote/grammar choices;
- lexical scopes, shadowing, immutable/mutable bindings, globals, dead stores, and
  unused values;
- short-circuit and conditional evaluation with side effects on both branches;
- syntax boundary cases where token concatenation can change parsing.

Each operator needs ordinary values plus boundary values. Parameterized generators
may expand value matrices, but each generated ID must encode the semantic family and
seed/value tuple.

## Win cases

Add explicit strict-win fixtures where LilScript’s type facts should remove JS glue:
signed-int normalization already proven redundant, closed union tags folded away,
pure unused work removed, and constants propagated through typed calls. Pair them
against honest JavaScript that preserves the same semantics; do not hand-sabotage the
JS source.

## Exit criteria

- Every scalar row in the coverage matrix owns positive, boundary, and effect-order
  cases.
- Optimized JS, optimizer-disabled JS, and the independent evaluator agree where the
  evaluator supports the construct.
- JS/native/C agree for portable cases.
- Every family is at or below metric-specific baseline minima; strict-win families
  have at least one stable, explained win.
