# Phase 06 — scale and release

Parent: [migration](README.md). Gates:
[release gates](../verification/release-gates.md).

## Objective

Canonical folders, the generated catalog, and `comparison/algorithms/` all stay
green. Canonical is the reviewed "why"; the catalog is the parameterized net;
algorithms are whole-program.

500 catalog entries is not completion. Completion is: no unowned coverage-matrix
cell, no gated loss, and release-check running `--canonical-only` plus the existing
catalog/algorithm lanes.

## Exit

`scripts/release-check.sh` includes canonical discovery. Evidence pages cite
generated summaries, not copied numbers.
