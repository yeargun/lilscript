# S2 — aggregate facts, scalar storage and copy elision

S2 implementation is complete under its [finite contract](../../../docs/migration/s2-acceptance.md).
This directory began on September 30; its final checkpoint is October 1, 2026.
It retains focused development evidence. It does not replace the accepted
ratchet baseline or claim fleet, runtime-speed or Closure ADVANCED qualification.

## Implementation

Shared allocation facts track bounded alias origins, fields, escape and identity
observations. Their consumers flatten private lexical namespaces, forward safe
field values, collect fresh construction stores and eliminate overwritten or
unread storage. Scalar banks preserve alias mutation, closure activations,
initialization order and declared field types. Record aliases feed the existing
record family, preserving absent-key normalization and special property keys.

Complete-use physical proofs let JavaScript update uniquely owned nested value
products without rebuilding every ancestor. Native formation transfers consumed
managed SSA owners without a retain/release copy, then clears the temporary.
Snapshots, dynamic/host observations, nullable element normalization and
unproved activation timing keep their existing representation. Captured native
banks remain packed when scalarizing would add allocated boxes.

`constant-folding`, `dead-code-elimination` and `scalar-replacement` independently
control their work. Scalar replacement now registers its native producer;
receipts include shared edits, bounded-analysis work and actual native ownership
transfers. The [configuration reference](../../../docs/configuration.md)
describes purpose, gates and size/compile/runtime tradeoffs. There is no new
effort policy: the comparison uses default effort 13.

## Retained identities

The comparison uses immutable local pins:

| Artifact | SHA-256 |
|---|---|
| S1 `s1-complete-4/lilscript` | `628028fe56197da6daa2cd09088fc21b34db11e7ff848020ff822b958ef87268` |
| S2 `s2-complete-2/lilscript` | `a1c7c7bc8571e1da983b1178dfb2b1924f73a61e9df0ae64aa6b88dad57bdece` |
| Measurement codec | `f0fcfce6e4a99e704b535fe7e6f97cda6953460e05c77c28c6cda45d2bc4d58c` |
| S2 source digest | `96824f2595631cd25339d393b44e2324208638b620102ce6d5d30739dada5d6c` |

[identity.json](identity.json) records all 368 Rust/Cargo source hashes, the
pre-commit HEAD `343a3af0`, and compiler/codec paths and hashes. The complete
source list and both binaries were checked against those identities after the
final release build. Subsequent closeout edits affect documentation only.

## Focused correctness checks

- [focused.log](focused.log): 22 final S2 tests pass. They cover aliases,
  loop/closure activation, reentry, initializer effects and throws, TDZ, field
  mutation, namespace calls, open script roots, identity/dynamic/host refusals,
  record absence and undefined payloads, nullable declared storage, tactic
  vetoes, product snapshots and native ownership.
- [compatibility.log](compatibility.log): 75 adjacent checks pass across the
  implementation batches: 52 shared-rule, eight product, six record, four
  tactic-control, four scalar-initialization and one native-availability check.
  They were not all rerun after the final normalization correction; the final
  22-test batch covers that correction.
- The native tests execute three C variants under GCC and Clang at O0/O2 and
  with UBSan, plus Clang ASan with leak detection: 21 execution profiles.
  These establish behavior and ownership safety for the exercised paths, not
  a runtime-speed claim.
- The final release compiler and codec build succeeds. Generated configuration
  reference and whitespace checks pass. No full library/CLI, port, full ratchet
  or paired CPU suite was rerun; S1's deferred checks remain deferred.

The focused Rust run used `RUST_MIN_STACK=16777216` and the existing Clang 18
toolchain selected by `LILSCRIPT_NATIVE_CLANG`. Test helpers execute emitted
JavaScript/C against explicit expected observations; the size runner executes
independent JavaScript oracles against both unmodified compiler outputs.

## Independent objective comparison

[measure.mjs](measure.mjs) compares 12 existing generic cases and three S2
development programs at effort 13 with IIFE delivery. [configs](configs/)
retains each objective's configuration; [artifacts](artifacts/)
retains inputs, independent oracle programs and compiler-written output.
[comparison.json](comparison.json) records hashes, expected observations,
per-objective exact sizes and rule receipts.

| Selected objective | S1 bytes | S2 bytes | Change |
|---|---:|---:|---:|
| Raw | 1,734 | 1,675 | −59 |
| Gzip 9 | 1,606 | 1,559 | −47 |
| Brotli 11 | 1,306 | 1,273 | −33 |

All 90 compiled behavior observations pass, with no individual size regression
under its selected objective. The existing 12 cases retain the same sizes.
The array-alias case saves 19/13/14 bytes and the namespace case saves 40/34/19
bytes in raw/gzip/Brotli respectively. The owned-product case has a smaller
direct recipe in its focused test, but search already finds an equally small
winner, so its end-to-end comparison is unchanged.

This small sample establishes concrete opportunities and preserves artifacts;
it is not a calibration set or evidence of broad performance parity. No
held-out programs were inspected and no accepted size baseline was changed.

## Remaining owners

S3 owns call-shape expansion and call-specific transport, including the two
historical inline-parameter/waiting-operand costs. Q1/S4 own replacement of
cleanup for representation-created subjects; Q2 owns frontend-wide accounting
and dirty scheduling; N2 owns unsupported native Record behavior. These
dependencies are recorded in the contract and [coverage map](../../../docs/migration/coverage.md).
