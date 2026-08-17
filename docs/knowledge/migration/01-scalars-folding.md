# Phase 01 — scalars and folding

Parent: [migration](README.md). Coverage:
[coverage matrix](../verification/coverage-matrix.md).

## Objective

Prove local arithmetic, bitwise, boolean, and string rules. These are the cases that
catch encoder/fold drift immediately.

## Families

`canonical/scalars/`, `canonical/strings/` (concat/index/search that do not yet
depend on pooling).

JS must use `|0` after ordinary LilScript `int *`. Do not pair `Math.imul` unless
the LilScript source calls `Math.imul`.

## Exit

Every scalar canonical case is `le` in raw, gzip-9, and Brotli-11. Constant-fold
pairs may be `le` even when both sides print a literal; the point is the compiler
does not emit a worse wrapper than Terser.
