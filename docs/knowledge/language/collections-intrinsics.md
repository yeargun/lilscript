# Collections and typed intrinsics

Parent: [language](README.md). Contract: [collections](../../language-v0.1.md#collection-literals-destructuring-and-iteration)
and [standard library](../../language-v0.1.md#standard-library-surface). Compiler
anchors: receiver and contextual-call checking in `src/check.rs`, primitive
resolution and shared operation identity in `src/primitive.rs`, and intrinsic
lowering in JavaScript formation and the native C writer (`src/program/`). Plan
M4.6 merges builtins and intrinsics into one operation catalog.

| Family | Semantic boundary | Typical JS representation |
|---|---|---|
| `T[]` | mutable, homogeneous, dense language array | native array, often scalarized/fused internally |
| `Record<T>` | open string keys, homogeneous values, observable key order | null-prototype object when materialized; keys never mangled |
| `Map<K,V>` / `Set<T>` | insertion order; SameValueZero scalar keys, identity references | native `Map` / `Set` |
| buffers/views | fixed bytes and typed coercion; `slice` copies, `subarray` aliases | native buffers and typed arrays |
| string methods | UTF-16-compatible indexing, JS `trim`/`search`/`slice`/`replace`/`split`, Unicode `codePointLength` | direct strings/known built-ins |

Array/record spread and destructuring are shallow, left-to-right, and evaluate the
source once. Missing array or record elements become nullable results. `for...of`
over arrays and typed arrays uses a direct indexed loop with live array length; it
does not allocate a language iterator. String iteration is rejected because its
code-point behavior would conflict with the portable indexing contract.

Array callbacks snapshot their starting length and preserve mutation/short-circuit
behavior. `indexOf` and `includes` intentionally differ (`===` vs SameValueZero).
Map/Set are invariant; struct keys are rejected until a portable value-identity rule
exists. Typed-array stores preserve each view's wrap/clamp/float conversion, and
overlapping `set`/`copyWithin` use snapshot/memmove semantics.

## Compression consequences

Typed operations are IR intrinsics rather than property strings. This enables direct
spelling, range reasoning, callback fusion, allocation sinking, and DCE. Those
rewrites must stop at alias, mutation, escape, callback-effect, or host boundaries.
Non-mutating typed array/string/`Math` intrinsics are pure language operations even
when the JavaScript emitter selects a built-in spelling. Mutators retain a precise
receiver-mutation effect. An explicit `JsValue` coercion or proxy-sensitive access
is a different semantic boundary and cannot inherit these purity facts.
`Record<T>` keys remain observable data even when internal nominal fields mangle.
The record family can replace a captured `Record<int>` that never escapes with
scalars. It never permits key renaming or ordinary-object backing for a surviving
record. The deleted route's record projection is in
[history](../history/compilation/aggregate-lowering.md#closed-record-observation-projection).

Collection tests need aliasing and mutation, callback evaluation order, absent keys,
prototype-sensitive record keys, JSON order, `NaN` membership, negative indexes,
typed-array overlap/coercion, buffer identity, and empty/default cases.
