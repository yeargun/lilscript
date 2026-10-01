# Migration handoff — D2 active

Implement the entire remaining migration, one milestone at a time. Commit/push
are authorized. Continue after each milestone. Coherent implementation batches
then focused checks; no repeated full library/fleet runs. One heavy job at a time;
no PR/subagents requested.

D1 is complete under [its contract](d1-acceptance.md), algorithm66/search33.
[Evidence](../../benchmarks/migration-results/2026-10-01-d1-formats/README.md)
records 34 focused checks, the final namespace-template guard, 21 independent
production codec/format builds and the targeted CJS/bare case-runner matrix.
D1 supports CJS, library IIFE/UMD and explicit bare, live linkage, frame semantics,
exact manifest receipts, injective filenames and independent program-body admission.
Q4 is complete under [its contract](q4-acceptance.md).

D2 is the sole active milestone: consumer-shakeable libraries, interface timing,
placement, lazy effects/cycles, typed host-module lowering, source maps and
Q4's graph-preserving public const product publication (currently diagnosed).
Remaining order: **D2 → D3 → N1 → N2 → V1 → V2**. N2 owns static native const
storage and native public const ABI.

Per-module elaboration is required in D3 with the shared graph/session owner.
Q2 arbitrary target-fragment relocation and S4 unrestricted public erased/
identity/mutable ABIs are explicitly scoped future proposals, not claimed work.
Keep independent objectives, size-focused effort13, expensive marginal tactics
at14/15 with explicit overrides, and hard TOML vetoes. Each new control documents
situations, gates and compile/size/runtime tradeoffs.

Workspace `/home/azureuser/lilscript`, branch `finer/059-idiom-directed-naming`.
Native Clang: `/home/azureuser/lilscript-work/toolchains/clang-18/root/usr/bin/clang-18`.
Do not touch the stopped remarklil rebuild owned by the separate task.
