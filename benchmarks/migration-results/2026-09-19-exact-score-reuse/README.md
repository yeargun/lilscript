# Exact-Score Reuse Evidence

Verification of migration task 006-P1, not a new plan or a completed milestone.
The sole task record remains in [the migration plan](../../../docs/migration/index.md#006-p1-exact-score-reuse).

## Result

- 92 focused tests passed, including six new regression tests; 2,606 unrelated tests were filtered out.
- Runtime checks used Node 24.11.1. The test build is debug, with assertions enabled.
- Equal Global/Scoped outputs retain separate naming trials but share their gzip/Brotli scores. A different Source output is measured independently: three naming trials, two distinct outputs, two optional codec probes.
- All 16 paired existing schedule cases retain identical winning JavaScript bytes.
- The twelve-state oracle still selects 205/159/124 bytes for raw/gzip/Brotli. Its 36 outputs are distinct, so all 70 optional probes remain. Full staged lookup adds 1,115 logical work units (9,541,882 to 9,542,997), with no new retained cache.
- The existing four-point Brotli interaction remains 143 bytes baseline, 144 literal-only, 144 naming-only, 142 combined. This is not a proof of traversal through a losing structural parent.

## Records

- [receipt.json](receipt.json): scope, identities, comparisons, independent review and limitations.
- [inputs-final.json](inputs-final.json): final compiler/build/fixture content inventory; receipts and documentation are excluded.
- [build.json](build.json), [build-artifacts.jsonl](build-artifacts.jsonl), [build.log](build.log): final incremental build command, environment, profile and diagnostics.
- [affected-tests.json](affected-tests.json), [affected-tests.log](affected-tests.log): final commands, executed case names, output observations and exact score checks.
- [baseline/receipt.json](baseline/receipt.json): source-qualified cached baseline, six replayed tests, original build evidence and 976-input pin.
- [source.patch](source.patch): this iteration's source diff, isolated from preexisting dirty-tree work.

The initial `new-tests` failure and intermediate build/test records are retained. The new integration fixture first assumed all naming styles emitted equal bytes; the corrected fixture explicitly distinguishes Source-style output. Production score reuse did not need a workaround. Final tests ran against the final content inventory, unchanged before/after testing.

## Scope

Reuse is limited to live retained records and exact requested codec coordinates. Primary/dependency bytes and file partition must match; provenance and eligibility are not shared. The existing preflight still stops new rendering after probe exhaustion, while already queued duplicates can reuse scores. Probe counts describe package-codec coordinates, not individual dependency encoder calls.

No full library fleet, release-speed experiment, production-service migration, global-optimum claim or competitive qualification was performed. Timings are single debug trials. Public/direct artifact admission and the true structural interaction fixture remain work in the canonical plan.
