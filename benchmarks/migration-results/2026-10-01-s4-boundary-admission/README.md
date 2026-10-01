# S4 public boundary admission

1. Run the existing public product/callable ABI proof in the shared frontend,
   before optional transformations and search. The build, check API and module
   graph clients must report the same declaration/module span. Target formation
   continues to validate its immutable input through the same proof owner.
2. Correct snapshot-array qualification: a locally read-only array is not
   isolated when callbacks, host accesses, suspension or writes through another
   alias can observe or change the original array during the call. Reuse the
   shared operation-effect owner for that condition; preserve supported closed
   read-only loops. Admit and release proof scratch on every exit.
3. Record the concrete public ABI and its limits. Erased type arguments cannot
   supply a missing public schema/discriminant; do not infer one from getters,
   field names or current call sites. Mutable aliased collections remain an
   implementation obligation and must never be accepted through copied arrays.

Prediction: supported closed controls stay byte-identical. Previously accepted
reentrant snapshot-array programs now fail with a source diagnostic instead of
emitting stale reads. Checking does bounded interface work but no target
formation, rendering or search. Verify focused API/module/callback/boundary
oracles, snapshot refusal and budget cleanup; no full-library/fleet run.

## Result

Public interface checks now run before source rules/search, using the same
adapter proof as target formation. Module re-exports retain the declaration's
module/span. Snapshot-array qualification uses shared effects to reject reentry,
suspension, host obligations and observable alias writes. Its scratch storage
is admitted. The common effect owner now recognizes an identical checked
`JS.assume` view while retaining real public-decoder effects.

49 distinct affected checks pass across two focused runs. The first batch
identified an overly conservative identity-view effect and a test using
unsupported direct re-export syntax; both were corrected. Only affected checks
and the shared effect suite were rerun. The final release compiler passes 18
independent artifact oracles with exact score/delivery comparisons. No full
library, fleet or timing run.

Pin `/home/azureuser/lilscript-work/bin/s4-boundary-admission-1/lilscript`, SHA-256
`214159f151ebcc3cfee68beb8c77c714178aafd63635af710bde5e7ddcd20131`;
algorithm 42 / search 32 / walk 10 / local facts 15.
[identity.json](identity.json) records exact source hashes;
[comparison.json](comparison.json) records artifacts and work counts.

| Matched program | Raw before = after | Gzip before = after | Brotli before = after |
|---|---:|---:|---:|
| Concrete product | 175 | 134 | 112 |
| Closed struct-array snapshot | 192 | 144 | 116 |
| Captured callback | 186 | 131 | 107 |
| **Total** | **553** | **409** | **335** |

Every control is byte-identical. The separate [counterexample](reentrant-array.json)
retains the previous compiler's wrong result **11**, the independent expected
result **12**, and the new source refusal at the exported declaration. Rejection
is not counted as a compression win or as implementation of mutable host-array
transport.

The [S4 public ABI decision](../../../docs/migration/s4-public-abi.md) replaces
an unspecified universal-crossing requirement with the existing concrete public
contract and explicit diagnostics. Broader witness/identity/collection ABIs are
unimplemented future proposals; this migration does not invent their semantics.
S4's defined consumers, controls and removals are complete under that scope.
Native completion remains N1/N2 and broad qualification remains V1. No baseline
or port artifact changed.
