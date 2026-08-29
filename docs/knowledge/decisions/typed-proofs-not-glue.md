# Typed proofs instead of port glue

Status: accepted. Parent: [design decisions](README.md).

## Intent

LilScript should beat JavaScript tooling because its source can state facts
before JavaScript exists, not because the compiler recognizes a library or the
port imitates minifier-friendly JavaScript.

## Decision

Legality comes from conservative reusable facts: ownership, escape, aliasing,
effects, ranges, allocation and constructor identity, capture mutation, dynamic
property access, and ABI visibility. Missing proof removes an alternative.

When several representations are legal, retain the incumbent and score the
alternatives. Add language surface only when it defines reusable semantics,
optimization envelope, ABI effect, unsupported-target behavior, and paired
tests.

## Tradeoff

Ports may stay larger until the language can express a sound fact. This is
preferable to a fast win that silently changes getters, proxies, descriptors,
identity, or evaluation order.

## Refusal

- No package-name, path, or library-AST matcher.
- No default-on `pure_getters` equivalent.
- No trailing-name convention as ownership proof in the target architecture.
- No post-hoc fold whose only evidence is one port.

Inventory: [compressor surface](../language/compressor-surface.md).
