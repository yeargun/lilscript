# Migration handoff — D1 active

Implement the entire remaining migration, one milestone at a time. Commit/push
are authorized. Continue after each milestone. Use coherent implementation
batches then focused checks; no repeated full library/fleet runs. One heavy job
at a time; no PR/subagents requested.

Q4 is complete under [its acceptance contract](q4-acceptance.md), algorithm65 /
search33. [Evidence](../../benchmarks/migration-results/2026-10-01-q4-data/README.md)
records 36 focused checks, 30 release JS artifacts with independent oracles and
canonical codec scores, two native compilers and paired reconstruction costs.
The final public-product guard has a separate focused check. No full fleet rerun.

D1 is the sole active milestone: finish CJS/library-IIFE/UMD/bare delivery,
manifest naming/escaping/stale cleanup and complete-tree scoring. Inspect the
existing `~/lilscript-work/portwork/m3.3b-wip.patch`; it is old config groundwork,
not an implementation to apply blindly. Application IIFE default is already in.
Remaining order: **D1 → D2 → D3 → N1 → N2 → V1 → V2**.
D2 also owns graph-preserving public const product publication, currently diagnosed;
N2 owns static native const storage and native public const ABI.

Per-module elaboration is required in D3 with the shared graph/session owner.
Q2 arbitrary target-fragment relocation and S4 unrestricted public erased/
identity/mutable ABIs are explicitly scoped future proposals, not claimed work.
Keep independent objectives, size-focused effort13, expensive marginal tactics
at14/15 with explicit overrides, and hard TOML vetoes. Each new control documents
situations, gates and compile/size/runtime tradeoffs.

Workspace `/home/azureuser/lilscript`, branch `finer/059-idiom-directed-naming`.
Native Clang: `/home/azureuser/lilscript-work/toolchains/clang-18/root/usr/bin/clang-18`.
Do not touch the stopped remarklil rebuild owned by the separate task.
