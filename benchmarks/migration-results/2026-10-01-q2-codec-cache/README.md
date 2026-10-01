# Q2 transparent persistent codec measurements

1. Add strict TOML controls for codec reuse and an optional cache directory,
   with `--cache DIR|off` overriding TOML. Default: existing within-build reuse,
   no disk writes. Paths resolve from the configuration file (CLI from cwd).
   Describe these physical controls outside semantic policy fingerprints.
2. Persist only complete codec measurement receipts in a fixed-capacity file.
   Keys bind final bytes, settings/role, compiler executable, platform, backend
   versions and accounting version. Checksums reject partial/corrupt writes;
   full key comparison rejects collisions. IO errors are misses, never build
   failures. Revalidate the canonical backend before accepting persistent hits.
3. Keep cold logical work and scratch admission on every reused measurement.
   Disabled/warm/cold storage must not grant more search. Report memory hits,
   disk hits and physical encodes separately from logical codec judgments.
4. Batch configuration, malformed/colliding/concurrent cache, bounded storage,
   resource-refusal and independent-objective behavior checks, then one matched
   cold/warm/disabled release comparison. No library/fleet rerun.

Prediction: identical winners and logical search, less physical encoding on
repeated builds/unchanged files. Disk IO and executable fingerprinting have a
cost; the directory is opt-in. Build/elaboration caches, decision locks and
incremental formation remain separate unfinished Q2 work.

## Result

Implemented and qualified as a Q2 batch. The release pin is
`/home/azureuser/lilscript-work/bin/q2-codec-cache-1/lilscript`, SHA-256
`549214afa580fd5c348e0680135b9faf61eecf59224306d9292da2c141bb9bb1`;
algorithm 45 / search 32 / walk 10 / local facts 15.
[Source identities](identity.json), [comparison](comparison.json), and
[reproduction script](measure.mjs) retain the exact inputs and artifacts.

Eight focused tests passed: strict configuration and fingerprint separation;
full key identities; corruption, truncation, collisions and concurrent writers;
unavailable storage; cold/warm/disabled logical accounting and memory/work
refusals; and search parity across all objectives. The initial build hit the
existing JSON macro recursion limit after adding telemetry; inserting that
field separately fixed it without increasing the limit. After those tests,
disk initialization was made lazy and per-output configuration was wired; the
final release compiled in 59.71 seconds and the release comparison below
exercises those changes. No full suite or fleet run.

All 18 release artifact/runtime oracles pass. Each case/objective's cold, warm
and disabled modes have identical file hashes, semantic policy fingerprint,
logical work, candidate counts and exact selected size. CLI overrides are
checked against conflicting TOML. Selected totals are 559 raw / 412 gzip /
348 Brotli bytes, separately optimized per objective.

| Case/objective | Physical encodes: cold / warm / disabled | CPU seconds: cold / warm / disabled |
|---|---:|---:|
| Spellings/raw | 0 / 0 / 0 | .06 / .06 / .06 |
| Spellings/gzip | 65 / 2 / 77 | .08 / .07 / .06 |
| Spellings/Brotli | 115 / 2 / 137 | .14 / .07 / .14 |
| Two files/raw | 0 / 0 / 0 | .00 / .00 / .00 |
| Two files/gzip | 6 / 0 / 14 | .01 / .01 / .00 |
| Two files/Brotli | 6 / 0 / 16 | .01 / .01 / .01 |

The first gzip build can reread entries evicted from its memory table; hence
its two disk hits are valid despite starting with an empty disk cache. Fixed
slot collisions leave two encodes in the warm spelling builds. Raw builds
create no cache file. Each compressed cache stays within 393,216 bytes.
Timing is one coarse process triplet, not a general speed claim: executable
fingerprinting and IO can cost more than encoding tiny gzip inputs. Keeping
the directory opt-in reflects that negative evidence.

This adds a physical backing to the existing measurement owner, with no
alternate score or artifact-qualification path. Build/elaboration reuse,
decision replay, dirty scheduling, shared formation and remaining admission
work keep Q2 active.
