# Superseded goal architecture

Status: historical design snapshot. Canonical target:
[planned architecture](planned-architecture.md). Current implementation:
[current architecture](current-architecture.md). Execution:
[planned migration](../migration/planned-migration.md).

The former document explored a universal `ChoiceGraph`, incremental `ProofDb`,
Pareto archive, exact islands, and solver-style guarantees. That exploration
established durable principles now promoted into
[design decisions](../decisions/README.md): contracts constrain objectives,
incumbents survive, legal alternatives are explicit, exact codecs select final
size winners, bounded work is reported honestly, and source/ABI identity reaches
the target.

The canonical plan intentionally chooses less machinery. It keeps the working
typed CFG/SSA compiler and bounded coordinator, consolidates decision recipes,
adds expected-versus-observed ABI validation, and introduces only the hygienic
target-JS representation needed by measured correctness and compression work.
Universal solvers, persistent proof databases, and dynamically expanding choice
graphs remain deferred until a minimized case proves the smaller design cannot
express or reach a required legal alternative.

The removed detailed proposal remains available in Git history before the
documentation consolidation on 2026-08-29.
