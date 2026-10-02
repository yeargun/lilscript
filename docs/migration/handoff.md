# Migration handoff — V1 active

Implement the remaining migration in order: **V1 → V2**. Commit/push are
authorized; continue after each milestone. N2 is complete under its
[acceptance contract](n2-acceptance.md) and [profile/corpus evidence](../../benchmarks/migration-results/2026-10-02-n2-native/profiles/README.md).

Use substantial coherent implementation batches followed by focused checks.
One heavy build/test/measurement job at a time. Do not repeat the entire library
suite for small edits. No PR or subagents have been requested.

Follow the [finite V1 acceptance checklist](v1-acceptance.md). Its known semantic
gaps, compiler library/CLI/language-matrix checks and matched size matrix are
qualified. The active stage is maintained/held-out library runtime/delivery,
followed by compile-cost qualification. Candidate 5 is pinned at
`/home/azureuser/lilscript-work/bin/v1-js-candidate-5/lilscript`. Its bounded target
DAG indexing passes all 192 affected checks and preserves the 54 app/algorithm
artifact hashes. The library runner saves completed per-port results under its
work directory; cnlil is already qualified and is excluded from the resumed batch.
The resumed work directory is `lilscript-work/out/v1-ports-production-2` and its
log is `/tmp/lilscript-v1-ports-production-2.log`. Hast-util-to-html is green;
jQuery's remaining syntax-only assertion is repaired and passes against the
same output. Motion has a source-boundary repair and MobX a test-workflow repair
awaiting their affected production checks. See `port-repairs.json` in the V1
evidence. Several port checkouts also separate `npm test` from `test:build` to
avoid compiling twice. Preserve their unrelated preexisting working-tree edits
when committing. The continuing fleet predates the runner's new build-failure
skip/receipt-preservation logic; do not silently relabel its old rows.
Do not restart the broad
compiler suite: every original failure has a passing focused recheck in the
[evidence](../../benchmarks/migration-results/2026-10-02-v1-javascript/README.md).
Preserve source/oracle boundaries and compare eligible Closure
ADVANCED separately for raw, gzip and Brotli. Keep effort 13 size-focused;
expensive marginal work can default to 14/15 with explicit TOML overrides.
Every control documents its situation, gate and compile/size/runtime tradeoffs.

The dirty Vue generated package/test outputs predate N2 closure and belong to
V1/V2. Finish their production profiles, parity/upstream checks and release
receipts. Do not run the stale `/tmp/d3-vue*.py` or `/tmp/migrate-d3-vue.py` drivers.

S4's unrestricted public erased/identity/mutable ABIs and Q2's arbitrary
changed-fragment relocation are explicitly scoped future proposals. They are
not hidden completion claims. S1's deferred broad checks belong to final V1
qualification. V2 owns reproducible package/site receipts and qualified staged
default adoption.

Workspace `/home/azureuser/lilscript`, branch `finer/059-idiom-directed-naming`.
Native qualification implementation pin: `lilscript-work/bin/n2-corpus-2/lilscript`.
Clang: `/home/azureuser/lilscript-work/toolchains/clang-18/root/usr/bin/clang-18`.
Node Unicode 17: `/home/azureuser/.npm/_npx/387698761821791d/node_modules/node/bin/node`.
Do not touch the stopped remarklil rebuild owned by the separate task, or stage
the four untracked N1 executable files. Native qualification archives exclude
binaries and keep generated C/header hashes.
