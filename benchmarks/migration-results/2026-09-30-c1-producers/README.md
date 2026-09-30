# C1: producer availability and configuration diagnostics

Source checkpoint `0609fe07`, pinned compiler `~/lilscript-work/bin/c1-producers-1/lilscript` SHA-256 `b588431387945a06492ab8c0340173906ee5e6eeee86f758b7b481724d038491`; codec SHA-256 `4ad51a603d1a5b80fe8cf3c22436070df1d4c49f26768bb1afd8882b488b36b9`.

Policy algorithm 3 records producer stages, target availability and prerequisites. Explicit settings without a producer receive diagnostics. Contradictory legacy aliases fail in either order. Naming prerequisite vetoes propagate independently of registry order. Service and CLI receipts expose diagnostics separately from semantic identity. The existing level-16 startup compatibility grant is diagnosed; its migration remains open.

Focused policy/service verification passes 53 tests; all six CLI tests pass. The full library run passes 1,675 tests, with nine existing ignores and the previously qualified expensive data-table test excluded. This includes S1 loops and S2 private record payloads, and excludes D1's subsequent private script wrapper.

The [generic ratchet](ratchet.md), with [complete receipt](ratchet.json), passes all 642 inputs × three objectives with the same 36 ledgered failures. Its complete per-lane state, selected-size and output-hash table is identical to S1's accepted baseline. The availability metadata and diagnostics introduce no artifact change on this corpus. This does not establish runtime parity or finish all family controls.
