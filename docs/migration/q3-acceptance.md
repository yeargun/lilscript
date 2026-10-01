# Q3 objective policy and useful effort contract

Q3 is complete after Q2 (`e18adb46`); its evidence is
[recorded here](../../benchmarks/migration-results/2026-10-01-q3-search/README.md).
The C3 training and evaluation receipts are evidence, not an optimization oracle.
Do not use an external library name as a policy feature.

1. [x] Bound and share terminal assignment evidence across compatible starts.
   Keys include the candidate, output permissions/contract, naming and complete
   assignments. Reuse a negative verdict only when its measured score cannot
   improve the current incumbent; a smaller incumbent or another source state
   does not justify reusing an ineligible artifact. Report avoided formations.
2. [x] Protect completed effort checkpoints when the structural frontier or
   automatic tactic gates change. Preserve each objective independently, fresh
   qualification, explicit vetoes, runtime permission and deterministic work
   limits. Higher effort may spend more compilation work to retain smaller bytes.
   Hard limits and explicit decision locks must have honest replay boundaries.
3. [x] Give each objective an explicit policy row. Keep unsupported tuning
   changes out of defaults; use frozen generic training, independent evaluation
   and exact artifact costs to calibrate ordering/proxy decisions. Gzip is not a
   synonym for Brotli. Preserve the ordinary effort-13 incumbent before costly
   marginal strategies at 14/15.
4. [x] Replace raw-only data ranking with objective-aware estimates, preserving
   every legal alternative even when its estimate is negative. Estimates order
   work and never establish legality or the final compressed score. Include
   decoder/helper overhead and state the applicable codec regime.
5. [x] Implement deterministic bounded worker scoring through TOML and `-j`.
   Consume independent results in declared order; workers do not change bytes,
   logical admission or permissions. Document capacity and memory/work tradeoffs.
6. [x] Batch focused policy/permission/effort/worker tests after implementation,
   then qualify affected generic all-objective artifacts and counted cost. Use
   current markedlil evidence to explain judgments, not as a tuning identity.
   Full maintained/held-out qualification stays V1. Update controls, receipts,
   coverage/history/checklist, commit and push the completed milestone.

Initial batch prediction: compatible starts avoid re-forming already losing
assignments; exact selected bytes remain eligible and may improve because the
same bounded search can reach more useful moves. Protected checkpoints add work
and retained incumbents, especially above effort 13. No CPU improvement or
universal optimality is presumed. Existing C3 results do not justify disabling
all pruning: immediate audit misses and losses of later paths remain separate.

Implementation, 25 focused checks, 48 generic and six affected data release
builds, independent oracles/remeasurement, fast-stop replay and CLI controls
are complete. The data fixture has a documented +6 Brotli search-path regression;
there is no universal size or compile-time improvement claim.
