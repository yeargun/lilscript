# Phase 04 — aggregates, collections, and object models

Parent: [migration](README.md). Representation:
[aggregates](../language/aggregates.md). Boundaries:
[escape](../language/boundaries-escape.md).

## Objective

Prove the language-level advantage: internal objects have known layout and may
disappear, while open string-key data and public JavaScript objects retain their
observable contracts.

## Required families

- structs/classes: construction, defaults, nested fields, mutation, methods,
  inheritance where supported, identity, escape, scalar replacement, named and
  positional layouts;
- public aggregate ABI: named reusable-library values vs explicitly opaque positional
  handles;
- records/open objects: dynamic keys, enumeration order, JSON, prototype assumptions,
  and the distinction between plain `{}` and null-prototype storage;
- arrays/typed arrays/buffers: aliasing, length snapshots, indexing, slices/subarrays,
  callback mutation, and binary coercion;
- maps/sets: SameValueZero/reference identity, chaining, nullable lookup, mutation,
  size, and DCE; add insertion-order/mutation-during-iteration cases when the language
  exposes an iteration surface;
- property/export mangling boundaries, quoted host properties, and escaped objects.

## Strict-win expectations

Typed fixed shapes, enums, scalar-replaced locals, and closed collections should own
strict-win cases. Dynamic records and public named facades may only reach parity;
their purpose is to prove the compiler does not buy bytes by changing an ABI.

## Exit criteria

- Every representation mode in the config matrix has a semantic and size fixture.
- Escape transitions (`LocalOnly`, return/export, host) are independently exercised.
- Browser-visible keys and JSON shapes match the paired JS contract.
- jQuery-relevant plain-object vs null-prototype cases are pinned before facade
  refactors continue.
