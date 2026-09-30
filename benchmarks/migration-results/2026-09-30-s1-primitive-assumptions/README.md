# S1: primitive conversion without host assumptions

Source `60b29e13`. Compiler `~/lilscript-work/bin/s1-primitive-assumptions-1/lilscript` SHA-256 `ac7471fd0ecf51eef44dfa3a808c2d500746785dfd1ced5e2f4689e8680880da`; codec SHA-256 `e6fea5e741c2ddc295b6b8f9fb3e8fc28cd3e0d5370c37a7d6b4758c9e82f273`.

The bounded evaluator can fold checked constant numeric `toInt()` without `assume_pristine_builtins`: the emitted primitive implements numeric ToInt32 directly, without a mutable host lookup. Known primitive inputs, arity, evaluation bounds and the constant-folding veto still apply. Host-backed methods retain their assumption requirement.

Seven numeric and six scalar-focused tests pass, including nested calls, modified `Math.floor`, argument effects, modified string iteration and JavaScript/native execution under each objective with both folding permissions and both assumption values. Existing independent binary64 and UTF-16 edge oracles remain active. All 1,740 active library tests pass, with four existing ignores and the previously qualified expensive table test excluded.

The [generic gate](ratchet.md), with [full receipt](ratchet.json), passes all 642 programs under each objective. A [full artifact comparison](comparison.json) verifies all 1,926 outputs against the preceding Q3 pin. Only `cases/catalog/number/semantic-number-to-int-boundaries` changes:

| Selected objective | Previous | New | Delta |
|---|---:|---:|---:|
| Raw | 91 | 71 | −20 |
| Gzip | 69 | 55 | −14 |
| Brotli | 61 | 45 | −16 |

Every other artifact is identical. The supported `--update-baseline` run accepts these three improved cells; no growth is accepted. All 20 [port artifacts](ports.json) are unchanged, with reference suites passing 29/29, 1,353/1,353 and 21/21. These are development/reference results, not held-out qualification. No new compilation-speed or runtime-performance claim is made.

Logs: `/tmp/lilscript-s1-primitive-{numeric,scalars,full,release,ratchet,ports,baseline}.log`. Measurements use the immutable release pin while the next search-policy batch is developed separately.
