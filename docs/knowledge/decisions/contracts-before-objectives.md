# Contracts constrain objectives

Status: accepted direction. Parent: [design decisions](README.md).

## Intent

Raw, gzip, and Brotli builds may choose different internal programs without
changing what the source, host, or unknown library consumer can observe.

## Decision

Normalize language semantics, compilation world, boundary roots, artifact
format, public/host ABI, explicit source obligations, unsafe assumptions, and
effect-removal policy before profitability search. Keep codec, priority, enabled
families, guards, and budgets in a separate objective.

An objective may rank only artifacts legal under the same immutable contract.
It may not change exports, callable behavior, field names promised by the ABI,
host names, evaluation order, or explicit target operations.

## Tradeoff

This rejects some smaller artifacts and requires boundary tests. It avoids a
more expensive and less reliable system where every optimization reinterprets
raw config or where API compatibility is inferred after selection.

## Refusal

- Do not infer application versus library world from ESM versus script format.
- Do not treat unsafe host facts as profitable search choices.
- Do not claim the current source-derived ABI manifest validates final bytes.

Implementation: [`src/compilation_contract.rs`](../../../src/compilation_contract.rs).
Plan: [planned architecture](../compilation/planned-architecture.md#1-contract-boundary).
