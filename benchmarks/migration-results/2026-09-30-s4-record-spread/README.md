# S4: typed record spread

Source `010e7e6b`. Compiler `~/lilscript-work/bin/s4-record-spread-1/lilscript` SHA-256 `0fd5430d4bc578ae54fc668ffa9fbdddde2f69cb3bc131b8e832d6805a95ad98`; codec SHA-256 `6ce1e5e56bac7bfa8bce1037a530f9f9661e6182a98733d30fd76419a0fd8a63`.

Typed `record` spreads now evaluate each source once and finish copying its own keys and values before the following entry. Null-prototype storage, key order, overwrites and value-struct copy semantics remain explicit. Counted copies inside call arguments, lazy branches and loop conditions stay at their original evaluation occurrence. Dynamic object spread and native record support remain separate work.

The [generic gate](ratchet.md), with [complete receipt](ratchet.json), passes **all 642 programs under all three objectives**, with zero compile, crash or behavior failures. The prior 33 refused lanes are now supported. All 1,893 previously passing cells retain both their size and emitted-byte hash. Totals now include 11 additional programs, so the added bytes are not a size regression or a compression improvement over a prior artifact. The supported baseline update accepts this coverage change.

The obsolete record-spread refusal entry is removed. The 30 catalog cells that are larger than the competitor have an explicit S2 owner for propagation, flattening and copy removal; the existing canonical/old-compiler loss owners remain. Correct support does not close those compression gaps.

[Port qualification](ports.json) passes 29/29, 1,353/1,353 and 21/21 tests. All 20 reported files are byte-identical to the previous qualified compiler.

Verification: 1,706 library tests passed, four existing ignores and the already-qualified expensive data-table case excluded. Five focused tests pass again after the final placement-depth adjustment. They cover no-source-binding temporaries, own/numeric/special keys, value copies, mutation between spreads, early throws, lazy branches, callee/argument order and loop conditions. Public builds cover raw/gzip/Brotli, ES2015/ES2022 and target compaction both on and off. No runtime speed claim is made.
