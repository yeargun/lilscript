# Migration handoff — Q2 active

The owner requests the remaining migration, one milestone at a time. Commit
and push are authorized. Implement substantial coherent batches, then focused
checks. Do not resume the deferred S1 full-library/CLI or CPU study. One heavy
job at a time. No separate PR is requested.

Checkout `/home/azureuser/lilscript`, branch `finer/059-idiom-directed-naming`.
C1/C2/C3/S1/S2/S3/Q1/G1/G2/G3/S4 are implemented. **Q2 is the sole active
milestone.** [History](history.md) retains earlier pins, measurements and
negative results; [coverage](coverage.md) keeps their remaining owners.

The [Q2 use-index batch](../../benchmarks/migration-results/2026-10-01-q2-use-index/README.md)
pins `/home/azureuser/lilscript-work/bin/q2-use-index-1/lilscript`, SHA-256
`563df22dfa771db23006916ada15442460f9a6ed93a219a0babdc9a780a66bad`.
Algorithm 43 / search 32 / walk 10 / local facts 15. The old four-pass function
pruner is deleted; one admitted rooted use graph closes chains/recursive groups.
Shared module graph routines admit entry projection and cycle bookkeeping,
reuse static orders for lazy roots and release projection storage after placement.
Delivery re-proves print forms after inserting setters; the previous release's
panic is retained separately from matched size evidence. 43 distinct focused
checks and 21 release artifact oracles pass. Matched controls remain byte-identical.

The following [bounded journal batch](../../benchmarks/migration-results/2026-10-01-q2-journals/README.md)
is algorithm 44 (not release-pinned). Admitted slot bitsets replace repeated edit
lists, preserve exact counts/remapping and release on success/refusal. Its 26
focused checks pass. The 10,000-edit control stays at 16 bytes. Dirty scheduling
and other resource owners remain open; do not mistake journal completion for Q2.

The [persistent codec batch](../../benchmarks/migration-results/2026-10-01-q2-codec-cache/README.md)
pins `q2-codec-cache-1`, SHA-256
`549214afa580fd5c348e0680135b9faf61eecf59224306d9292da2c141bb9bb1`;
algorithm 45 / search 32 / walk 10 / local facts 15. Eight focused checks and
18 release artifact/runtime oracles pass. Strict `[cache]` and `--cache DIR|off`
control optional bounded disk codec receipts; cold/warm/disabled outputs and
logical bills match. Physical encodes fall, but tiny gzip IO can cost more.
The final release checks cover lazy disk setup and per-output configuration
added after unit checks. Build/elaboration caches and decision locks remain.

The [normalization batch](../../benchmarks/migration-results/2026-10-01-q2-normalization/README.md)
pins `q2-normalization-1`, SHA-256
`c514eca8a6cb6392a54512d489a20d571f031e2d60dd764a4b1bfa2ee11b3e15`;
algorithm 46 / search 32 / walk 10 / local facts 15. 35 focused tests and
36 release artifact/runtime checks pass; one native test needed the installed
Clang path and was rerun alone. Artifacts are byte-identical to algorithm 45;
normalization reuse on/off has identical logical work/search. Local source
revision proofs and three target stable-pass consumers reduce visits but show
no clear timing gain. Renumbering uses admitted plans and in-place swaps; the
old clone path and escaping remap results are deleted.

The latest [target-copy batch](../../benchmarks/migration-results/2026-10-01-q2-target-copies/README.md)
pins `q2-target-copies-1`, SHA-256
`f32d84840ba334f8d9560ddb7d8d74b54be56017af9eae62b298fa15323b6a86`,
algorithm 47. Typed copies admit every nested payload; behavior proof replacement
and removal release their backing. 23 focused tests and 30 release oracles pass;
all files/judgment counts match algorithm 46, with increased logical copy work.
Initial placement and remaining source/target producers still need admission.

The [effect-component batch](../../benchmarks/migration-results/2026-10-01-q2-effect-components/README.md)
pins `q2-effect-components-2`, SHA-256
`4cd627217bd2d5a5208fe65aee4bca6d56f74a1e9fd3afdcdeee8afbd507eac8`,
algorithm 48. Exact SCC invalidation and dirty recursion reduce physical effect
summary executions 1,278 → 390 on 21 case/objective pairs. 41 distinct focused
tests and 61 successful release oracles pass. Reused/disabled files, logical
bills and search counts match; matched prior outputs are byte-identical. Two
prior gzip/Brotli failures are repaired separately: conditional spelling
eligibility no longer mutates an unselected condition. No clear CPU gain or
complete source-analysis memory claim. Previous/current effect analyses are
phase-local; the remaining source resource owner must admit their overlap.

The [whole-build cache batch](../../benchmarks/migration-results/2026-10-01-q2-build-cache/README.md)
pins `q2-build-cache-1`, SHA-256
`8776c5e0224e0a6b1ef2e5d94cb83bc18bf7bac26906eae7f5ce3d7da6268a93`,
algorithm 49. Optional `cache.build_reuse` stores complete handoffs and cold
logical receipts in 64 bounded slots. Fresh graph/host identities and complete
configuration/compiler/limits gate hits. 12 focused tests, schema/release CLI
controls and 72 release oracles pass; all prior outputs and current logical
bills match. Warm generic samples drop to .00–.01 CPU seconds; cold cache IO
adds .01–.02 seconds and tiny gzip has no clear benefit. No broader claim.
Per-module elaboration and (at that pin) decision-lock replay remained, as did formation and
remaining resource owners. Low-effort handoffs can carry unmeasured codec
scores: preserve None, never invent a score. Native public exports still refuse.

The [decision-lock batch](../../benchmarks/migration-results/2026-10-01-q2-decisions/README.md)
pins `q2-decisions-1`, SHA-256
`a0c3d68135f3b50f12c5485880b0583c6fdfb73af9ca41de3fdb240d753dd8d3`,
algorithm 50. `[decisions] read/write`, `--choices FILE|off` and
`--write-choices [FILE]` save separate objective assignments. Full current
identities gate reads; current proofs, permissions, limits and artifact admission
qualify the first terminal move. Fast tiers pin the baseline through replay.
Ten focused checks and 36 release oracles pass; all prior/control bytes match.
No general speed benefit: one Brotli walk drops 50→42 judgments, another grows
65→84. Keep explicit opt-in and negative evidence. Whole-build hits are bypassed
for explicit lock IO. Do not reuse a local-site compression verdict across source
changes; complete bytes/encoder identity are necessary. Per-module elaboration,
formation, global dependencies and remaining resource owners stay open.

S4 is complete under its [acceptance record](s4-acceptance.md) and
[explicit ABI scope decision](s4-public-abi.md). Unrestricted public erased
schemas, ambiguous unions, observable wrappers, mutable aliased product
collections and erased optional presence remain unimplemented future ABI
proposals. No hidden runtime witness/proxy/public convention was invented.
This corrects an unspecified plan gate under the owner's scope-change authority;
it does not claim universal language or native support. `inline for`, `@pool`
and all twelve documented regional `@choose` families are complete, including
hard TOML/replay permissions. The shared fact/removal audit is complete.

1. Finish [Q2's acceptance contract](q2-acceptance.md): dirty scheduling,
   shared/incremental formation, transparent bounded persistent reuse and the
   remaining resource owners. Existing source-effect caches, fixed-point suffix
   reuse, retained target heads and exact codec memoization are starting points.
   The use-index consumer, module graph, local dirty maps and renumbering plans
   are complete. Remaining global rule dependencies, placement storage,
   source analysis/edit owners, cross-candidate formation,
   per-module elaboration reuse remain. Explicit full-identity decision locks are now implemented.

2. Finish Q2 before **Q3 → Q4 → D1 → D2 → D3 → N1 → N2 → V1 → V2**.
   Q3 owns markedlil candidate growth and G3's 48 → 292 spelling judgments for
   six Brotli bytes; records-128's 588 judgments are retained cost evidence.
   Default effort 13 favors size, with marginal costly exploration at 14/15 and
   explicit overrides. Raw/gzip/Brotli remain independent objectives.
3. Preserve `ref`, public constructibility and `pure` termination (Y2/Y3/Y4).
   D3/V2 own qualified language-default adoption. D1 inspects
   `/home/azureuser/lilscript-work/portwork/m3.3b-wip.patch`.
4. N1 resolves native `preserve_root_exports=false` skipping frontend checks
   while the shared root export table remains. N2 owns native shapes,
   absent-to-default arguments, exceptions/suspension/regex and full runtime/ABI.
5. Every new behavior flag documents purpose, gates, vetoes, interactions and
   compile/runtime/size tradeoffs. Keep negative results. No fleet, Closure,
   runtime-speed or native-complete claim until the corresponding qualification.

No ratchet baseline or port artifact changed in the final S4 batches.
