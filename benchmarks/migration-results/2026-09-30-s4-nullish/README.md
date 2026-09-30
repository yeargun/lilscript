# S4: checked nullish assignment

Source checkpoint `8ebb686d`, compiler `~/lilscript-work/bin/s4-nullish-1/lilscript` SHA-256 `599f70fec9044290254fd2f9b0e2673da1cfa31c6005ad083515bb2fa4b1e88b`; codec SHA-256 `a91cc3c0f84f69a34287f74e420142960e0eec9185c81683f47ea274acd49135`.

`??=` now uses the existing checked place, load, lazy nullish region and store operations. Receivers and keys are captured once; a present value does not evaluate the RHS or call a setter. Copying and callable-name inference keep the source assignment rules.

All four carried JavaScript cases are enabled. The 18 focused tests pass, covering getter/RHS/setter exceptions, absent and falsy values, temporal initialization, reference capture across RHS mutation, value-struct copies, callable names, all objectives and ES2015/ES2022 with target compaction both enabled and disabled. The native fixture passes GCC/Clang O0/O2 and sanitizers. The subsequent full scheduler run includes these tests.

The [generic ratchet](ratchet.md), with [complete receipt](ratchet.json), passes 642 inputs × three objectives. Only `cases/catalog/nullish/semantic-nullish-lazy-assignment` changes: it now passes all three lanes. Every previously passing lane retains its size and selected output hash. Ledgered failures fall from **36 to 33**. Added artifact bytes for a formerly missing result are coverage, not a matched size regression or improvement.

The stale failure entry is removed and the [supported baseline update](ratchet-baseline.json) passes. This qualifies the language gap; general defaults, generic methods and other remaining S4 work are separate.
