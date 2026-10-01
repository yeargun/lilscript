# Migration handoff — Q4 active

Implement the entire remaining migration, one milestone at a time. Commit/push
are authorized. Continue after each milestone. Use coherent implementation
batches then focused checks; no repeated full library/fleet runs. One heavy job
at a time; no PR/subagents requested.

Q3 is complete under [its acceptance contract](q3-acceptance.md), algorithm64 /
search33. [Evidence](../../benchmarks/migration-results/2026-10-01-q3-search/README.md)
contains 25 focused checks, 54 release artifacts/oracles/remeasurements and CLI
controls. The data fixture's +6 Brotli search-path regression and extra checkpoint
cost are explicit. Do not repeat these checks without a new change or failure.

Q4 is the sole active milestone. Read R9 in `docs/language.md`, M8.5/M9.8/M10.3/
M10.19 in the archived plan and Q4 coverage. It owns const data/exact bounded
evaluation, schema specialization, helper ownership/placement, pooling and
remaining encoding alternatives including lazy legality/runtime gates.
Remaining order: **Q4 → D1 → D2 → D3 → N1 → N2 → V1 → V2**.

Per-module elaboration is required in D3 with the shared graph/session owner.
Q2 arbitrary target-fragment relocation and S4 unrestricted public erased/
identity/mutable ABIs are explicitly scoped future proposals, not claimed work.
Keep independent objectives, size-focused effort13, expensive marginal tactics
at14/15 with explicit overrides, and hard TOML vetoes. Each new control documents
situations, gates and compile/size/runtime tradeoffs.

Workspace `/home/azureuser/lilscript`, branch `finer/059-idiom-directed-naming`.
Native Clang: `/home/azureuser/lilscript-work/toolchains/clang-18/root/usr/bin/clang-18`.
Do not touch the stopped remarklil rebuild owned by the separate task.
