# C1: target tactic permissions

Compiler source: `8bb9dc3c` (isolated checkout). Release binary:
`~/lilscript-work/bin/c1-1/lilscript`, SHA-256
`aab3f7213aed3fce33f591cea074b86d3a9709d029aaf0d5aa2f11f450124f2c`.
Codec SHA-256: `9aeac973f74390fc3d8a7411cef2f89bc9defa3f8e967e8a0e6513efbae16ceb`.

The [ratchet report](ratchet.md) and [JSON receipt](ratchet.json) cover all 642
items and 1,926 objective lanes. All corpus totals and loss counts match the
accepted Y5 baseline, separately for raw, gzip and Brotli. Verdict: pass, no
new problems or improvements. The 36 existing ledgered failing lanes remain;
they are not counted as successful outputs. The accepted baseline was not
changed. The run took 85 seconds, which is a run duration, not a paired CPU
performance claim.

Library verification: 1,644 passed, nine existing ignores. The first complete
run passed 1,606 and lacked Clang for 38 native tests; all 38 passed when rerun
with the local Ubuntu Clang 18.1.3 distribution. A 16 MiB test-thread stack
was needed for the existing deep checker fixture. Independent Node checks
cover pooling, packing, fresh returned arrays and the fixed rule switches;
artifact tests cover vetoes before formation, during search under all three
objectives and after retaining permission evidence.

C1 remains partial: this batch covers these target rule/family permissions.
Startup/recurring reconstruction admission, the remaining family registry,
compatibility migration and complete public controls remain in the active plan.
