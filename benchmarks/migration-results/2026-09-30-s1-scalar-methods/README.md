# S1: bounded constant scalar methods

Source `0b083b2d`. Compiler `~/lilscript-work/bin/s1-scalar-methods-1/lilscript` SHA-256 `decdb280794972678298f1c1ccf2e8cc156273a3b5d57decbaa338a434e5f853`; codec SHA-256 `66f3daab7c04bb535eeb8df6fa2708c789d13ac934b7fa3e33f81f1a938c834e`.

The constant-call evaluator now handles `float.toInt()` and `string.codePointLength()`, including calls nested in small functions. Conversion truncates and wraps into signed 32 bits, mapping non-finite values and either zero sign to zero. Counting follows UTF-16 iteration: a valid surrogate pair counts once, while each lone surrogate counts once. The existing builtin contract, folding veto, argument/effect checks and work/storage bounds still apply.

Six focused tests pass, including independent Node oracles for 533 binary64 bit patterns and 108 UTF-16 sequences; nested function removal; arity and resource refusal; and a patched host iterator without the pristine-builtin assumption. Public effort-13 builds execute identically in JavaScript and native C under raw/gzip/Brotli, with TOML folding both enabled and disabled. The full library suite passes 1,726 active tests, with four existing ignores and the previously qualified expensive table case excluded.

The [generic gate](ratchet.md), with [full receipt](ratchet.json), passes all 642 programs under each objective. Every size/hash is unchanged from the [indexed-call compiler](../2026-09-30-q2-call-index/README.md). All 20 [port artifact records](ports.json) are also identical, and reference suites pass 29/29, 1,353/1,353 and 21/21. This batch has no measured generic or port size win and needs no baseline update.

The focused [method program](methods.lil) exercises the added capability under explicit pristine builtins at effort 13. [Pinned measurements](method-results.json) compare each independently configured objective with the previous compiler:

| Selected objective | Previous | New | Delta |
|---|---:|---:|---:|
| Raw | 169 | 60 | −109 |
| Gzip | 131 | 46 | −85 |
| Brotli | 121 | 37 | −84 |

The [manifest](method-workloads.json) and three TOML files reproduce these builds. All six retained [artifacts](artifacts) pass the fixed [execution oracle](methods.out), with [engine/artifact identities and observations](oracles.json). This is one development example, not held-out calibration. The standard compiler runner retains one CPU pair per configuration for completeness; these subsecond samples do not establish a performance improvement or runtime parity claim.
