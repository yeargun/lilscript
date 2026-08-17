# Phase 04 — collections and effects

Parent: [migration](README.md). Language:
[collections](../language/collections-intrinsics.md),
[async](../language/async-generators-regex.md).

## Objective

Arrays, records/JSON, Map/Set, nullish, `try/finally`, generators, `Task`. Host
touches stay `extern` / `JS.*`.

## Families

`canonical/collections/`, `canonical/effects/`, `canonical/host/`.

Parity first. Strict wins only where a typed intrinsic removes a JS shape the
minifier must keep.

## Exit

No gated loss on canonical folders in these families.
