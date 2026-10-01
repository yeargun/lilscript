# S2 implementation acceptance

S2 implementation is complete. S1 is checkpointed at `343a3af0`; its remaining
broad verification remains deferred by the owner. The implementation below
builds on existing record/product families and checked initialization, use,
call and effect views. [Pinned evidence](../../benchmarks/migration-results/2026-09-30-s2-complete/README.md)
records the focused verification and its limits. S3 is next.

## Shared facts and semantic edits

1. Derive bounded allocation identities through values, aliases, cells,
   structured control and complete typed calls. Classify observable identity,
   local/captured/typed/host escape and unknown uses separately. Null tests do
   not expose identity. Unsupported or exhausted analysis supplies no proof.
2. Track reads, writes and constant definitions by allocation/field identity,
   with nominal reflection and exported/host boundaries preserved. Eliminate
   redundant/overwritten stores and forward fixed field values only where
   initialization, effects, dominance and mutation permit it. Keep initializer
   effects in order even when the field itself is unused.
3. Flatten private constant namespaces and scalarize closed object/class/record
   storage, including aliases and closure captures with the same activation.
   Preserve conditional/loop initialization, independent loop activations,
   absent record keys, special property keys, getters and host observations.
   Unknown identity or dynamic observation retains the original representation.
4. Extend value-product storage through branches and loops. Prove when a
   logical copy needs no independent physical copy, and when field updates may
   mutate owned storage without rebuilding every ancestor. Retain snapshots
   whenever another source value remains live. Consume the proof in JavaScript
   and native output, retaining the existing `ref` language contract.
5. Use shared facts in place of equivalent source-level target recognizers.
   Representation-created storage stays under Q1/S4's explicit ownership;
   never delete useful cleanup before its replacement can see its subjects.

## Controls and optimization decisions

- `constant-folding` owns field-value forwarding and constant namespace reads;
  `dead-code-elimination` owns unread/overwritten stores; `scalar-replacement`
  owns scalar storage and copy-elision alternatives. Each remains independently
  vetoable in direct, searched and reused output.
- Keep actual representation alternatives in the existing family mechanism
  where profitable layout is uncertain; add no parallel optimizer or per-port
  heuristics. Raw, gzip and Brotli retain independent exact judgments.
- Register required facts, invalidation and runtime effects. Explain every new
  behavior in the existing TOML reference. Receipts count allocations, fields,
  stores and copies actually changed, separately from eligibility facts.

## Focused completion checks

Implement coherent batches before compiling or testing. Use one focused suite
covering the new facts and their consumers, with independent JavaScript/native
oracles for alias mutation, snapshots, reentry, throws, loop captures, dynamic
observations and negative eligibility. Check tactic vetoes and representative
structural/size effects under raw/gzip/Brotli. Measure relevant runtime work
only for a specific changed allocation/copy path; make no unsupported fleet
speed claim. Do not rerun the entire library suite or resume S1's stopped
qualification chain. Expand verification only for an observed failure or an
unresolved concern in the changed code.

## Completion and dependency boundaries

- [x] Shared `ProgramAggregates` carries bounded origins, aliases, typed-call
  inputs, escape/identity observations and field facts. Unknown returns,
  dynamic or host observation, unsupported initialization and analysis limits
  retain storage. A static allocation site alone never proves one runtime
  object or activation.
- [x] Shared rules expose object allocations, collect fresh stores, flatten
  immutable lexical namespaces, forward fixed fields and remove overwritten
  stores. Scalar banks preserve aliases, captures, activation and declared
  field types; record aliases feed the existing exact-judged family. Record
  absence and nullable-array normalization remain intact.
- [x] Complete-use physical proofs allow direct updates of owned JavaScript
  products across branches/loops and native single-use ownership transfers.
  Whole-value snapshots retain independent values. Native captures keep
  packed storage when splitting would increase the number of allocated boxes.
- [x] Existing TOML tactics independently control these producers; registry,
  invalidation, native availability, generated reference and actual-change
  receipts are updated. Default effort remains 13, with no new costly tail.
- [x] Twenty-two final S2 checks and 75 adjacent checks across implementation
  batches pass. Three native C variants run under seven compiler/sanitizer
  profiles each. Fifteen programs under independently selected raw/gzip/Brotli
  objectives pass 90 old/new behavior observations; totals shrink in each
  objective. These are focused development checks, not full-fleet qualification.

The contract does not require unsupported storage to become optimizable.
Opaque/host/generic boundaries, arbitrary constructor calls and returned
allocation identities stay conservative. S3 owns call-shape expansion and
constructor/method inlining; S4 owns missing language/host contracts; N2 owns
native Record support.

The two historical M7.9 call-storage growths belong with S3's call transport
and Q1's representation recipes: inline parameter cells and waiting struct
argument operands arise during target formation, after S2's source edits.
Removing them needs call-specific evaluation/snapshot proofs, not a broader
claim that an allocation is unique. They remain explicit open work in
[coverage.md](coverage.md); they are not counted as fixed here.

Likewise, existing target scalar/namespace cleanup remains until Q1/S4 can
replace proofs for representation-created subjects without losing coverage.
S2 supplies shared source facts and their new consumers; it does not delete
useful later cleanup that those source facts cannot address. Q2 retains the
frontend-wide resource-accounting and dirty-scheduling work. No full library,
port, ratchet or paired timing rerun was required for this checkpoint, and
S1's deferred broad verification was not resumed.
