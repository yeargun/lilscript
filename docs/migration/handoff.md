# Migration handoff — D3 active

Implement the entire remaining migration, one milestone at a time. Commit/push
are authorized. Continue after each milestone. Coherent implementation batches
then focused checks; no repeated full library/fleet runs. One heavy job at a time;
no PR/subagents requested.

D2 is complete under [its contract](d2-acceptance.md), algorithm67/search33.
[Evidence](../../benchmarks/migration-results/2026-10-01-d2-consumers/README.md)
records consumer/host/cycle/lazy/map checks, esbuild consumer measurements and six
independent raw/gzip/Brotli release artifact/runtime/manifest checks. The release
pin is `lilscript-work/bin/d2-consumers-1/lilscript`. D2's source-map files count in
exact output costs; unlowered foreign single-file order/laziness limits are explicit.

D3 is the sole active milestone: shared CLI/LSP/lint graph/session ownership,
per-module elaboration with dependency invalidation and cold/reused diagnostic
parity, shared multi-format workflows, compiler-written port/package entries.
Audit motionlil's ten entries/nominal renames, remaining post-minifiers, mobxlil's
development host globals, katexlil's two font-metric scripts and source/config
ownership. Independent objective-set TOML/API/CLI results already exist.
Remaining order: **D3 → N1 → N2 → V1 → V2**.

Q2 arbitrary target-fragment relocation and S4 unrestricted public erased/
identity/mutable ABIs are explicitly scoped future proposals, not claimed work.
Keep independent objectives, size-focused effort13, expensive marginal tactics
at14/15 with explicit overrides, and hard TOML vetoes. Each new control documents
situations, gates and compile/size/runtime tradeoffs. N2 owns static native const
storage and native public const ABI.

Workspace `/home/azureuser/lilscript`, branch `finer/059-idiom-directed-naming`.
Native Clang: `/home/azureuser/lilscript-work/toolchains/clang-18/root/usr/bin/clang-18`.
Do not touch the stopped remarklil rebuild owned by the separate task.
