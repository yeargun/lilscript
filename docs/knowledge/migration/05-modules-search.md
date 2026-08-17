# Phase 05 — modules, search, and compiler bugs

Parent: [migration](README.md). Compilation:
[global optima](../compilation/global-optima.md),
[candidate search](../compilation/candidate-search.md).

## Objective

When a canonical case fails, it becomes a compiler investigation: missing proof,
missing transform, search picking a locally short spelling that loses Brotli, or
emission. Keep the failing folder. Add the smallest extra folder that isolates the
bug.

Module and lazy cases join here once a fair JS bundler baseline exists for that
boundary (`single` vs `split`). Until then, do not compare a closed script to an ESM
library.

## Exit

Every red canonical row has an owner note in the case README or a compiler test.
Search still scores complete artifacts under the configured codec.
