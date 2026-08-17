# Phase 02 — control flow and functions

Parent: [migration](README.md). Language:
[control flow](../language/control-flow-errors.md),
[functions](../language/functions-closures-generics.md).

## Objective

Branches, loops, early exit, defaults, closures, identical-helper folding, and dead
pure callees.

## Families

`canonical/control/`, `canonical/functions/`.

`lt` is reserved for proven DCE and identical-function folding. A loop that both
sides must actually run is `le`.

## Exit

No gated loss. At least one `lt` for unused-pure-helper DCE and one `lt` for
identical helpers.
