# Use a narrow hygienic target-JS representation

Status: planned. Parent: [design decisions](README.md).

## Intent

Stop reparsing generated strings to rediscover bindings, owned properties,
precedence, and source obligations while preserving the useful target-level
contractions already measured.

## Decision

Introduce a target representation only for constructs emitted by LilScript. It
carries resolved bindings, external/global references, owned property identity,
function/call kind, allocation identity, ordered effect barriers, module edges,
syntax-floor requirements, and lowering obligations. It prints JavaScript and a
witness that maps semantic identities and obligations to final nodes/ranges.

Semantic transforms stay in typed IR or in narrow proof-backed target
operations. An independent standards-grade parser checks final syntax, bindings,
module links, and observed ABI; it does not claim to prove program equivalence.

## Tradeoff

The current parsed peephole remains a scored fallback during migration. Move a
family only for correctness, obligation handling, or measured value. Do not
build a full JavaScript frontend or a third general optimizer.

Plan: [planned migration](../migration/planned-migration.md).
