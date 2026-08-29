# Aggregate lowering and scalar replacement

Parent: [compilation](README.md). Language model:
[aggregates](../language/aggregates.md). Source anchors: aggregate/field IR in
`src/ir.rs`, escape and scalar replacement in `src/optimizer.rs`, layout options in
`src/config.rs`, and both backend emitters.

Nominal fields enter IR as owner plus numeric field index. Open records and host
objects use separate string-keyed operations. That distinction prevents a dynamic
key from silently disabling optimization of every typed aggregate.

Which of those representations is **searched** versus configured is
[decision registry — aggregates](decision-registry.md#aggregates-class-struct-object-record).
Scalar replacement is the configured incumbent; size-first library search can
admit a scored `keep-object` IR clone. Joint array/object search is gated and is
omitted from root `lilscript.toml`.

Lowering is chosen from proof and boundary:

1. A `LocalOnly` struct/class can scalar-replace into SSA values. Construction and
   fields vanish; overwritten stores and dead fields disappear.
2. A value that remains inside typed LilScript may use compact positional storage.
3. A reusable named JS boundary materializes the declared field ABI. Owned internal
   names may mangle, but host names and protected public fields do not.
4. `public_aggregate_abi = positional` permits an opaque public array handle only
   when the consumer contract does not inspect fields.
5. `Record<T>` has an open null-prototype string-map contract; every surviving JS
   record representation preserves that prototype. A JS-only projection candidate
   may instead eliminate closed record construction and observations when the
   complete operation is proven equivalent. `extern class` uses exact host names
   by default. The current explicitly configured closed-key mode is legacy ABI
   policy and must not be inferred for arbitrary host objects.

Class calls statically devirtualize because overriding is rejected. Base fields are
flattened. Native currently rejects inheritance until its subtype pointer ABI is
fixed. A constructor that is itself a JS-observable value is a different
representation — named ES `class`, not an instance literal. See
[class identity](class-identity.md).

Partial escape sinking and joint representation search may propose alternatives, but
alias, identity, exception, export, and host observations constrain legality first.
Named-object vs positional-array instance layout is both a transfer and runtime-memory
choice; the selected codec alone does not measure per-instance heap cost.

The general CFG scalar replacer remains deliberately linear, but loop headers have a
narrow phi-aware path. A `LocalOnly` struct carried by a structured loop is exploded
atomically into one SSA phi per field when every incoming value is a direct constructor
of the same nominal type and the complete use set consists only of that loop phi and
direct field reads. The transform removes the constructors, aggregate phi, and reads
as one unit; it never deletes only part of the representation. Mutation, calls,
returns, captures, another phi (including branch merges), aliases/shared inputs,
exceptions, type/layout disagreement, or any non-field observation retains the
original aggregate path. Phi inputs are real SSA edge uses, so all rejected cases
continue through the conservative dangling-reference-safe fallback.

Tests compare local dissolution, loop-carried field phis, mutation, typed and unknown
escape, branch merges, shared phi inputs, public named/opaque export, identity,
inheritance rejection, and record/host non-mangling. The escalating
`aggregate-ledger` algorithm is the complete-artifact proof: its independently
compiled raw, gzip, and Brotli targets each execute the fixed runtime/host-trace
oracle before their matching metric is compared with the JavaScript frontier.

## Closed-record observation projection

`project_closed_record_observations_for_javascript` runs only after the neutral IR
optimizer and only as a JavaScript candidate-search alternative. It can replace a
proven present/missing static read, a complete constant JSON serialization, or a
joined one-use `Object.keys` observation with its exact value. An immutable record
snapshot may cross CFG blocks only when its allocation dominates the observer and
every use of that identity is read-only. Mutable facts remain block-local; a write,
phi, terminator, unknown/host call, or exception region stops the cross-block proof.
JSON folding accepts only complete portable integer/boolean/string/null shapes and
uses ECMAScript own-key order.

Projection never replaces the configured candidate. When it changes the IR and a
finalist has at least two candidate slots and two budget bytes, the compiler splits
that finalist's count and byte budgets between projected and unprojected artifacts.
The unprojected artifact is mandatory; complete-artifact codec ranking chooses.

`IrJsOptions::ordinary_record_literals` still exists as an internal experimental
emitter option, but production candidate search deliberately keeps it disabled.
Once projection has erased an observation, reconstructing prototype safety from the
projected IR would be unsound: ambient `Object.prototype`, an inherited `toJSON`, or
a missing-key read can distinguish `{}` from a null-prototype record. Enabling
ordinary backing requires carrying a pre-projection proof as provenance; until then,
every surviving record uses the contract-preserving null-prototype spelling. See
[`src/compiler.rs`](../../../src/compiler.rs) and
[`src/optimizer.rs`](../../../src/optimizer.rs).
