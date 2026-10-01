# S4 shared target storage facts and rule ownership

1. Derive physical binding writes, calls, uses, array origins and closure factories
   once in the existing reachable-target walk. Consume these facts in target
   inlining, alias/namespace cleanup, initializer cleanup and array-call lowering;
   delete their separate write/origin proofs. Retain source initialization stamps
   and conservative physical transfer for representation-created storage.
2. Move primitive host-result categories into the operation catalog, and route
   generated-value queries and frame-elision checks through the shared target
   transfer instead of separate spelling/effect proofs.
3. Classify retained normalization rules with their actual legality, fact owner
   and convergence measure. Keep source semantic optimization and target spelling
   choices distinct, including every existing hard TOML veto. Remove stale
   transition labels only after their proof consumers are integrated.
4. Exercise fact invalidation after edits, generated bindings, imports/exports,
   loop/catch bindings, and the affected normalization paths in one focused batch.
   Compare matched artifacts under each objective after a release build.

Prediction: shared scans remove redundant derivation and conservatively cover
implicit writes. Existing transformations and optional choices remain eligible;
no runtime change is intended. A missed implicit write may require a soundness
fix even if a formerly smaller artifact grows. No fleet or timing claim.

## Evidence

The completed batch passes **135 focused checks**: the JavaScript target tests,
public callback/product-union boundaries, hard scalar-policy vetoes and product
call snapshots. This includes admitted fact ownership, refusal/unwind release,
strictness, captures, and generated storage mutation. The release CLI build
passes. No full-library or fleet rerun was performed.

All five matched independent oracles pass under all three objectives. Every
artifact and the recorded candidate/search counts are byte-for-byte identical
to the product-union pin. Selected totals remain **14,378 raw / 3,359 gzip /
2,524 Brotli**. The 292 spellings and 588 records-128 Brotli judgments remain
negative search-cost evidence for Q2/Q3; no compilation-speed claim is made.
Scored and delivered bytes agree.

Pin `s4-shared-storage-1`, SHA-256
`7659ea2a498c298552dbb13b6e07c67c017f55d7ec5a6c3c3e0ef1c6c4f08ab8`.
Algorithm 37 / search 32 / walk 10 / local facts 14. Exact compiler/source,
configuration, artifact and work identities are in [comparison.json](comparison.json)
and [identity.json](identity.json). The [owner audit](../../../docs/migration/s4-target-facts.md)
explains retained target normalization and later Q2/Q4 removals. S4 remains
active; no ratchet baseline or port artifact changed.
