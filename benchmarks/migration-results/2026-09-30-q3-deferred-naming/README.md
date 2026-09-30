# Q3: protected refinement of rejected naming starts

Compiler source `f484fadc25a26871200437f7d9f831f9fea6ac32`, SHA-256 `34a7ceb333f3a652e3f16394c34482e39ea139af7fd6273d92310afabbe3a262`; canonical codec SHA-256 `4a74cae23627dc3e2116aeed9fa8832d23f8bef1b46a33710a71c2442624301d`. The previous compiler is C1 source `632614da`.

At effort 13 and above, the terminal walk first completes its existing search and local naming/private-property refinements. It then revisits only naming starts actually rejected by the initial proxy, measures those starts exactly and refines them with ordinary move pruning. Only an admitted exact improvement replaces the completed winner. Fixed storage holds pending seed identities, and the deferred group shares one formation context. `[policy.search] deferred_naming_starts = false` vetoes the tail. The default remains effort 13; fast tiers and naming vetoes retain their earlier behavior. Search version 28 and walk version 7 fingerprint the change.

All 1,738 active library tests pass; four existing ignores and the previously qualified expensive table exclusion remain. Focused public-API tests execute each objective, prove the old search records remain an exact prefix, cover configuration/fast-tier/tactic vetoes and force resource exhaustion without losing the completed winner or retaining storage after handoff. An obsolete test ordering assertion was updated because local polish now precedes the additional tail; its size and replay assertions remain. Timing head bounds include the deferred group and the four existing head combinations.

The [effort-13 experiment](effort13.md) uses the same three frozen TOML files, source programs, bare-script boundary and canonical encoder settings as the previous C3 normal-pruning run. All 642 programs pass each objective. The [receipt verifier](summarize.py) checks both compiler pins, source/configuration/explanation/artifact hashes, matching task policies apart from the declared schedule change, effective resources, every old start/move record as an exact prefix, and no individual output growth. It verifies 1,224 distinct input paths.

| Objective | Previous bytes | New bytes | Delta | Previous probes | New probes | Optional-work ratio | Deferred starts / wins |
|---|---:|---:|---:|---:|---:|---:|---:|
| Raw | 59,019 | 59,019 | 0 | 0 | 0 | 1.497× | 338 / 0 |
| Gzip | 54,058 | 54,048 | −10 | 7,442 | 11,599 | 1.476× | 430 / 6 |
| Brotli | 43,753 | 43,753 | 0 | 8,719 | 8,719 | 1.000× | 0 / 0 |

Probe counts include exact gzip proxies. Raw work includes scoring and artifact handling without an encoder. All six prior C3 missed gzip opportunities are recovered: the two enum-dispatch cases save two bytes each, the array pipeline three, the min/max scan one and the two range-generator cases one each. No program grows, no search reports resource exhaustion, and maximum accounted retained memory is unchanged. Raw gains nothing in this development slice; Brotli rejects no starts here. Preserving the old prefix costs more counted work than the earlier no-prune experiment, which reached the same gzip gains with a different trajectory. These counters are not CPU measurements or held-out qualification.

The [ordinary generic gate](ratchet.md) passes all 642 programs under each objective and recovers exactly the same ten gzip bytes, with no other changed cell. Its supported baseline update accepts those six improvements. The [port run](ports.json) passes 29/29 Marked, 1,353/1,353 Zod and 21/21 PostHog tests. Marked and Zod artifacts are identical. PostHog's Brotli-selected core (`posthog.raw.js`, a historical filename) changes from 4,985 to 4,940 Brotli bytes; its raw/gzip coordinates also shrink, but they are secondary measurements of that same selected file. Independently raw- and gzip-selected PostHog outputs are unchanged. The ESM/CJS/UMD facades incorporating that core also shrink; those are not additive independent wins.

[Three alternating CPU pairs per workload](compile-cost.json) use the [nine frozen port/objective configurations](compile-workloads.json), all at effort 13 with equal effective resource ceilings. The explicit policy-change reason identifies only the new tail and schedule versions. Each configuration copies the earlier frozen port settings, changing only its selected objective.

| Workload | Previous CPU median | New CPU median | Ratio | Selected bytes | Exact codec probes |
|---|---:|---:|---:|---:|---:|
| markedlil-raw | 2.304 s | 4.812 s | 2.089× | 32,700 → 32,700 | 0 → 0 |
| markedlil-gzip | 2.036 s | 4.563 s | 2.241× | 9,859 → 9,859 | 89 → 186 |
| markedlil-brotli | 5.383 s | 11.347 s | 2.108× | 8,851 → 8,851 | 68 → 152 |
| zodlil-raw | 7.749 s | 16.769 s | 2.164× | 81,708 → 81,708 | 0 → 0 |
| zodlil-gzip | 9.652 s | 16.448 s | 1.704× | 29,648 → 29,648 | 106 → 173 |
| zodlil-brotli | 18.382 s | 40.404 s | 2.198× | 26,573 → 26,573 | 67 → 144 |
| posthoglil-raw | 0.380 s | 1.024 s | 2.695× | 16,360 → 16,360 | 0 → 0 |
| posthoglil-gzip | 0.544 s | 1.420 s | 2.610× | 5,809 → 5,809 | 47 → 135 |
| posthoglil-brotli | 0.941 s | 3.274 s | 3.479× | 5,224 → 5,200 | 25 → 97 |

Every previous/new CPU range separates in these three-pair samples. The report retains ranges, samples and host load; this establishes the observed extra work, not a universal CPU ratio. Eight of the nine matched output hashes are unchanged. The frozen effort-13 PostHog Brotli winner saves 24 selected bytes; its secondary raw coordinate grows by three bytes and gzip shrinks by twenty. This is distinct from the shipped-port boundary above. Raw and gzip port winners do not improve here. Peak accounted retained bytes grow in four cells (largest increase 519,385 bytes) and are unchanged in five; the report retains every work and memory count.

The additional exploration buys the measured generic gzip and PostHog Brotli wins at a substantial compilation cost. Raw has no measured win in this slice. The size-focused default permits the bounded tail and the explicit false setting removes it; this does not establish optimal objective defaults. Repeated formation/scoring and coupled naming/private-field refinement remain follow-up work.

Reproduce the default-effort summary with:

```sh
python3 benchmarks/migration-results/2026-09-30-q3-deferred-naming/summarize.py \
  ~/lilscript-work/measurements/2026-09-30-c3-proxy-calibration/on \
  ~/lilscript-work/measurements/2026-09-30-q3-deferred-naming/effort13
```

The [full verified summary](effort13-summary.json) retains per-case hashes, old/new starts and work totals. All generated explanations and artifacts remain under `~/lilscript-work/measurements/2026-09-30-q3-deferred-naming`. Broader objective calibration and reuse remain open; the bounded search does not establish a global minimum.
