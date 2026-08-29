# LilScript roadmap

Authority: [planned migration](knowledge/migration/planned-migration.md) owns
execution order; [ledger](knowledge/migration/board/LEDGER.md) owns live task
state; [`docs/current-status.md`](current-status.md) owns the current snapshot.

## North Star

LilScript is a compression-first typed web language. For every declared
supported and semantically equivalent maintained boundary, `size-first` should
eventually produce compiler output no larger than the best eligible pinned
JavaScript baseline for the selected raw, gzip, or Brotli metric. Correctness,
source intent, host behavior, and public ABI are hard constraints.

This is an expanding engineering criterion, not a theorem over arbitrary
JavaScript. See [mission](knowledge/mission.md).

## Current Priorities

1. Replayable large-library evidence and selected recipes.
2. Expected-versus-observed ABI plus pre-score final-byte validation.
3. Recovery of legal Motion/Marked/MobX incumbents that currently regress.
4. Consolidation of existing decisions and acceptance paths.
5. Minimal hygienic target-JS representation for identity-sensitive work.
6. Reusable language/analysis proofs and measured interactions that close
   maintained library gaps.

Details and exit criteria: [planned migration](knowledge/migration/planned-migration.md).

## Completion Rule

A capability is complete only when:

1. language, source-intent, host, and ABI semantics are explicit;
2. legality comes from conservative reusable proof;
3. optimized and appropriate fallback executions agree;
4. JavaScript/C/native agree where the feature is portable;
5. every declared candidate passes typed and final-artifact validation before
   exact scoring;
6. the incumbent and admitted alternatives are reachable/rejected/reported;
7. semantic/API, canonical, codec, and affected external gates pass;
8. selected-metric, compile-time, memory, and runtime effects are recorded;
9. public claims link a tracked fingerprinted report.

Durable choices: [design decisions](knowledge/decisions/README.md). Current
implementation: [current architecture](knowledge/compilation/current-architecture.md).
