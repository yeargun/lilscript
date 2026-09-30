# Q2: index single-call statement sites

Source `c8c4dc58`. Compiler `~/lilscript-work/bin/q2-call-index-1/lilscript` SHA-256 `9a0e2bc6874db488e19237de3fc2ec76567b3aafe0f68357418b3d8b93106d4e`; codec SHA-256 `e2a3d8edff90be21b5e0fbc87eb5ee62b66d1e1d857085bc83eecd677ce02b52`.

The block inliner builds one binding-indexed call-site table for each unchanged target tree, replacing a whole-program statement search for each eligible function. Declaration order and the first supported site are preserved. Arguments are cloned only for the selected edit; body and parent checks are delayed until there is a supported site. Temporary index storage is admitted and released before editing. Each edit rebuilds the index, retaining the same opportunities, depth checks and round bound.

All 1,720 active library tests pass, with four existing ignores and the previously qualified expensive table test excluded. Test builds compare each eligible lookup against the independent old scan. Added execution/resource oracles cover all supported statement forms, nested returns, temporary-storage release and refusal without mutation.

The [generic gate](ratchet.md), with [full receipt](ratchet.json), passes all 642 programs under each objective. Every size and emitted-byte hash matches the [preceding qualification](../2026-09-30-c1-resources-q2-profile/README.md). All 20 [port artifact records](ports.json) are also unchanged; reference suites pass 29/29, 1,353/1,353 and 21/21. No baseline update is needed.

[Three alternating CPU pairs](compile-cost.json) retain complete samples, workload/compiler identities and matched effective resources at frozen effort 13:

| Workload | Previous median CPU | New median CPU | Ratio | Exact judgments | Brotli bytes |
|---|---:|---:|---:|---:|---:|
| markedlil | 7.211 s | 6.317 s | 0.876× | 68 → 68 | 8,851 → 8,851 |
| zodlil | 19.375 s | 18.012 s | 0.930× | 67 → 67 | 26,573 → 26,573 |
| posthoglil | 0.977 s | 1.063 s | 1.088× | 25 → 25 | 5,224 → 5,224 |

Zod's ranges are separated: 19.286–19.398 versus 17.645–18.478 seconds. Marked ranges overlap (6.054–7.691 versus 6.054–7.016), as do PostHog's (0.965–1.218 versus 1.018–1.095). Background host activity was present; all samples and load readings are retained. This supports a measured Zod CPU improvement, without a general speed claim or hiding the higher PostHog median.

Diagnostic warmups place Zod's single-call rule at 3.783 → 2.338 seconds, with 156 rule invocations on both sides. Warmup elapsed counters are attribution evidence, separate from paired process CPU. Artifact hashes and terminal judgments are identical on all three workloads. This removes repeated lookup work without reducing default-level size quality. Persistent formation reuse, broader dirty scheduling and resource completion remain Q2 work.
