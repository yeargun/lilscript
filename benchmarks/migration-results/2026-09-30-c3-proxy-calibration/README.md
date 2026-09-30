# C3: matched effort-13 proxy calibration

Compiler source `632614da`, SHA-256 `475f66d876a10a8335c3c0ea1a528d0aeccc38e0b68ec1584943c6e735ded254`; canonical codec SHA-256 `1605ba6a5d786bea2e5b99f416cb914c8577b6d239fe227d07354e002ecea9c6`. Runner/configuration source `682a1d70`.

All 642 existing development programs were compiled independently for raw, gzip and Brotli at effort **13**, using the explicit bare-script boundary. The [frozen TOML files](configs/on/raw.toml) retain the existing generic settings and differ only by objective and proxy mode. Normal pruning, audit and no pruning all pass every execution oracle. Normal mode initializes a separate experiment baseline; audit and no pruning pass against it. The ordinary accepted baseline is unchanged.

| Objective | Normal bytes | Audit bytes | No-prune bytes | No-prune delta | Normal exact codec probes | No-prune exact codec probes | No-prune optional-work ratio |
|---|---:|---:|---:|---:|---:|---:|---:|
| Raw | 59,019 | 59,019 | 59,019 | 0 | 0 | 0 | 1.242× |
| Gzip | 54,058 | 54,058 | 54,048 | −10 | 7,442 | 9,415 | 1.230× |
| Brotli | 43,753 | 43,753 | 43,753 | 0 | 8,719 | 8,719 | 1.000× |

Counts are summed across the 642 programs for each independently targeted objective. Raw scoring still consumes logical work even though it runs no encoder. Gzip's proxy is exact, so its probe count includes proxy-rejected moves. These are deterministic work counts, not paired process-CPU measurements. Maximum accounted retained memory is unchanged within each objective; neither structural nor terminal search reports a stop.

Audit mode preserves every output hash. It records 3,096 raw and 4,947 gzip rejected scores, with no negative delta. **Brotli rejects no move in this corpus** at its existing margin, so this run does not calibrate Brotli false-negative behavior. Larger generic programs and an independent protected held-out set remain necessary.

All six gzip improvements come from previously pruned naming restarts whose initial artifacts were larger but whose refined results won. For example, one enum program's Global start is 126 bytes against a 125-byte initial artifact, so normal pruning skips it; refinement reaches 107 bytes, beating the completed normal result of 109. Both Global and Source starts yield wins in the [retained restart evidence](restart-evidence.json). Disabling all proxy rejection is broader than this observed opportunity and adds work even in raw, which gains nothing here.

The next bounded hypothesis is to revisit rejected naming starts after the existing search completes, retaining its winner and preserving normal move pruning. The new work must remain bounded, configurable and measured independently per objective. No heuristic default changes are part of this calibration.

[Normal report](on.md), [audit report](audit.md), [no-prune report](off.md), [verified summary](summary.json). Full JSON reports retain resolved policies, configuration identities and hashes of each compiler explanation. Explanation and artifact files remain under `~/lilscript-work/measurements/2026-09-30-c3-proxy-calibration/{on,audit,off}`.

Reproduce the summary with:

```sh
python3 benchmarks/migration-results/2026-09-30-c3-proxy-calibration/summarize.py \
  ~/lilscript-work/measurements/2026-09-30-c3-proxy-calibration
```

The summarizer verifies compiler/codec/bar identities, configuration hashes, all explanation and delivered-artifact hashes, 1,794 distinct source files, matching input graphs, effective resources and policies differing only by proxy mode. Only the generated corpus work-directory prefix is normalized; original receipts retain full paths. This is inspected development evidence, not held-out qualification, runtime parity, an encoder-noise estimate or a global-optimality claim.
