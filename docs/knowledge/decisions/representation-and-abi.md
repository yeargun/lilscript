# Representation is private; ABI is explicit

Status: accepted direction. Parent: [design decisions](README.md).

## Intent

Internal classes, structs, closures, and properties should use whichever legal
representation best serves the selected objective. Unknown consumers must see
the declared API regardless of objective.

## Decision

Compilation world, artifact format, and boundary roots are separate axes. A
reusable library freezes an expected ABI for its public roots; private entities
remain eligible for DCE, inlining, scalar replacement, layout search, and
mangling. A closed application may still use ESM internally without making every
chunk export public.

Expected ABI is derived before optimization. Final artifacts provide an observed
ABI witness for exported names, callable kind/arity/constructibility, constructor
and prototype identity, promised fields/descriptors/order, module linkage, and
host names. Behavioral suites remain necessary; bytes cannot prove general
semantic equivalence.

## Tradeoff

The ABI model grows only for maintained observable boundaries. It is not a
universal JavaScript reflection schema.

## Refusal

- No “library mode means optimization off.”
- No output-format shortcut for compilation world.
- No objective-dependent public API.
- No extern-key mangling unless a separately declared coordinated foreign ABI
  owns both sides; ordinary host `extern` names stay exact.

Details: [packages and exports](../language/packages-exports-abi.md).
