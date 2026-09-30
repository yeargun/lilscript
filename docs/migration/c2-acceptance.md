# C2 acceptance and implementation batch

C2 is the only active milestone. This checklist fixes its acceptance before
implementation; [index.md](index.md) remains the progress board.

1. Extend the checked-AST interpreter to nominal structs, ordinary classes,
   enums/generic calls and ordered Map/Set identity operations. Measure actual
   interpretation of the conformance corpus. Provide an oracle command that
   refuses existing-output disagreement and never blesses compiler output.
   Unsupported host/language effects stay explicit in the coverage receipt;
   each optimization must have an independent oracle for the feature it changes.
2. Remove pinned regression prologues from random generation into conformance
   cases. Generate by result type and feature mask, retain seeds and sources,
   and run native/C lanes on the supported mask instead of disabling them for
   every generated program. Keep JavaScript-only features in a separate lane.
3. Provide one executable benchmark-contract runner: explicit boundary and
   semantics, tool/input content locks, complete delivered artifacts, common
   behavior oracle for our artifacts and every competitor, independent minima
   for raw/gzip/Brotli, codec robustness measurements, retained failures and
   compile costs. Include Terser, SWC, Oxc, Rolldown, esbuild, Closure ADVANCED
   and upstream recipes; unsupported boundaries are reasoned inapplicability.
   Qualify open-library, closed-application and consumer-bundle contracts.
4. Complete the paired runtime protocol with reproducible no-op controls,
   warm-up samples, bootstrap intervals, noise-aware judgments, static
   operation inventories and a runtime regression ledger. Qualify representative
   compiler-written ports and generic allocation/call/startup micro workloads.
   Preserve startup, steady CPU/wall, retained memory, host load and all samples.
5. Resolve C2's carried test-ledger entries, document the public commands and
   oracle/performance requirements, then run the combined tool/interpreter/
   differential tests and the appropriate release conformance gates once.

Predicted effects: no production compiler-byte or search-policy changes.
Oracle and measurement runs cost more; ordinary compilation does not acquire
runtime benchmarking. New test tooling has its own explicit work bounds.

The archived requirement to run fixed 150-round/30-warm-up, multi-engine and
KaTeX tiering investigations for every compiler change is replaced by the
current plan's affected-workload rule. The runner retains configurable warm-up
and sample counts, controls and intervals. Small/noisy samples remain
inconclusive; a finite measurement cannot certify all workloads or engines.
Browser-specific qualification belongs to affected browser workloads in V1,
using the same oracle and contract requirements. A host API without a declared
independent model is reported uncovered rather than silently approximated.
This keeps C2 finite without weakening later optimization acceptance.
