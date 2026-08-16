# Release gates

Parent: [verification](README.md). Active rollout:
[migration phase 08](../migration/08-scale-corpus-release.md).

## Gate layers

| Layer | Required evidence |
|---|---|
| Per-change | Rust unit/conformance, formatter/lint as relevant, changed micro families |
| Merge | Full paired micro/config suite, differential oracle, bundle fixtures |
| Release | Merge gates + 11-case structural algorithm lane + Closure apps + browser + Lilpack/apps/libraries/scenarios/popular lanes |
| Scheduled deep | maximum candidate budgets, multiple codecs/engines, trend/flake analysis |

The existing `scripts/release-check.sh` runs compiler tests, differential
verification, focused benchmark families, Closure app comparisons, app/library/
popular/scenario/browser lanes, and web builds. It now reaches
`comparison/cases/run.mjs` and `comparison/algorithms/run.mjs` through
`comparison/run-all.sh`, so both the micro contract and the Closure-inclusive
11-pair/42-vector structural contract are release-blocking. That wiring does not
upgrade either lane into browser or public-API evidence; those require their own
gates below. The current canonical structural report selects all 11 cases and passes
11/11 with zero failure events; every raw, gzip, and Brotli lane is an observed strict
win. That makes the structural component green, but does not by itself establish the
other release layers in this table.

## Size rules

- Required case rows gate individually; aggregate totals cannot offset a loss.
- Raw/gzip/Brotli comparisons use separately compiled objective artifacts and
  metric-specific eligible minima; cross-metrics are diagnostic.
- Strict-win tags remain strict.
- Public and closed-app boundaries gate separately.
- Candidate search must retain its configured baseline; a higher search budget may
  not worsen `size-first` transfer selection when that baseline stays legal.
- Split/lazy gates cover the complete declared artifact set and initial-load view.

## Correctness and runtime

No size row is eligible before its behavior oracle passes. Portable cases also require
the configured C/native matrix. Runtime gates apply only where a representative,
statistically defined workload exists; absence of runtime evidence must be stated,
not treated as a pass.

## Evidence refresh

Changing compiler code, LilScript/JS sources, configs, baseline tools, codec settings,
or harness logic invalidates affected reports. Regeneration is an explicit command and
reviewed diff. A prose table may summarize current JSON but must not be hand-maintained
as a competing source of truth.

## Claim levels

| Claim | Minimum support |
|---|---|
| Transform works | semantic regression + ablation |
| Smaller on case/family | current raw/gzip/Brotli report, named config/toolchain |
| Baseline parity on maintained corpus | every eligible required row green |
| Library win | exact public surface, behavior, size, and named performance gates |
| “Never larger” | only the exact corpus/boundaries/metrics with an enforced gate |

The mission is directional until the last claim’s scope is explicit. The suite must
make overclaiming harder than reporting the truth.
