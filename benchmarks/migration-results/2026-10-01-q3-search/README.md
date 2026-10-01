# Q3: objective policy, protected effort and bounded scoring

Algorithm 64 / search schedule 33. Q3 is complete under its
[acceptance contract](../../../docs/migration/q3-acceptance.md).

- Complete assignment keys share negative evidence across compatible starts,
  bounded by two candidate/objective namespaces, 512 assignments and 256 KiB
  per namespace. Only a score that cannot improve `(objective, raw)` is skipped;
  another candidate or a potentially improving score requires fresh formation.
- Efforts 13–16 retain the completed preceding policy's qualified winners before
  new tactics/frontiers spend optional work. Every objective and tier admits its
  mandatory baseline before the seal. Resource failures before that seal fail
  the request; later optional failures preserve admitted incumbents. Explicit
  decision locks remain specific to their effort. Fast-tier replay comes from
  the actual level-12 policy, not a higher policy with different tactic gates.
- Separate raw/gzip/Brotli rows and additional objective orders come from frozen
  C3 training. `calibrate-prior.py` verifies its source receipts. The ordinary
  incumbent remains protected; automatic extra ordering begins at 14, explicit
  on at 13. Existing proxy margins and seed values are deliberately unchanged.
- Data estimates measure isolated complete decoder/literal fragments with the
  requested codec settings (Brotli's proxy quality is capped at 5). Final names,
  surrounding compression context and helper sharing remain whole-artifact
  concerns. Negative estimates never delete legal alternatives.
- TOML `execution.jobs` and `-j` select 1–4 workers for independent delivered
  files. Fixed batches, pre-admitted bounded scratch/work, private worker
  ledgers and ordered cold-receipt consumption keep bytes/logical admission
  independent of concurrency. Single-file search and deadline requests remain
  serial. A private-cap failure falls back to the ordinary scorer.

The [configuration reference](../../../docs/configuration.md#objective-search-and-scoring-workers)
explains situations, gates, exact controls, memory/work costs and fallback
behavior. These controls neither grant runtime permission nor claim exhaustive
optimization.

## Focused verification

[25 distinct focused checks pass](qualification.json), covering all three
objectives, completed efforts 12–15, default/explicit policy resolution, decoder
exactness under canonical/custom codec settings, negative alternatives, worker
cold/reuse/refusal parity, multi-entry Node observations, mandatory baseline
failure, objective-share unwind, vetoes and zero codec/checkpoint work at level 0.

Initial/follow-up logs retain failures and fixes. One real error caught a
mandatory checkpoint-baseline resource failure as optional; the catch now needs
a completed seal. Other corrections fix a source fixture's quotes, exercise
workers at measured effort 1, classify actual naming restarts correctly and
verify existing raw tie breaks with explicit `raw_delta` receipts. Only failed
or newly added checks were rerun. The subsequent fast-stop receipt correction
is covered by standalone level-12/13/14 release comparisons. No full library or
fleet run.

The Q2 report-label additions had been reviewed after its last test build.
This build found their expanded JSON macro exceeded the recursion limit; the
same fields are now assigned after constructing the report.

## Cost evidence and the markedlil diagnosis

The [historical trace](../../../docs/old-history/migration/history.md) already
explains the original 14 → 46 judgments: structural discovery stayed at five
proposals, two structures, six renders and five probes. V2 created a better
structural incumbent, causing a second terminal start with 31 judgments.
That accounts for 31 of the 32 added judgments; three additional rejected
integer-hint trials were a small part. The extra paths bought 13 Brotli bytes.
No library-specific threshold is introduced here.

The current Q2 raw/13 trace has 354 judgments and 1,254 full formations, with
175 incremental print variants and 22 whole-tail hits. Its one enabled/disabled
pair (48.48/54.59 CPU seconds) is prior cost evidence, not a Q3 speed measurement.
Distinct source candidates cannot share an assignment verdict just because
compressed sizes match. Q3 instead reuses complete compatible assignments and
reports checkpoint work separately. No markedlil rebuild or fleet comparison
is repeated for this policy batch.

## Release qualification

The pinned [compiler/source identity](identity.json), [CLI controls](cli.json),
[48 generic builds](artifacts.json) and [six affected data builds](data.json)
are retained. Every output passes an independent Node oracle and canonical
codec remeasurement. Protected checkpoints, including level-12 fast-stop hashes,
match separately built policies exactly. No qualification input fitted the
objective prior; evaluation families are explicitly already exposed.

| Objective | Four programs, effort 12 | Ordinary 13 | Protected 13 | Protected 14 |
|---|---:|---:|---:|---:|
| raw | 18,083 | 18,031 | 18,031 | 18,031 |
| gzip | 8,741 | 8,739 | 8,739 | 8,739 |
| Brotli | 6,357 | 6,248 | 6,248 | 6,248 |

Summed single CPU observations across the twelve program/objective cells are
0.71 / 1.91 / 2.28 / 4.43 seconds respectively. These are cost observations,
not stable timing estimates. Protected 13 adds checkpoint work; 14 explores
more without a further size win here. Ordinary/protected 13 recall 528 verdicts
in their final-tier traces; 14 recalls 1,833. These counts are not a speedup
claim or counts of all lower-tier work.

The data fixture stays at 7,883 raw and 2,673 gzip, but Brotli rises 2,085 →
2,091 against algorithm 62. The new decoder-inclusive estimates order different
site moves: the flat string table finishes as a dictionary rather than front
coding, and the former global-naming continuation no longer wins. All legal
forms remain available and the selected moves improve their actual incumbent.
This is a bounded-search path regression, not decoder inexactness. No special
case or calibration on this fixture was added. The comparison includes
algorithm 63's accounting changes, and V1 owns broad totals/individual cells.
Single CPU observations for this fixture are 0.69 → 0.89 raw, 0.47 → 2.29 gzip,
and 2.62 → 3.03 Brotli; estimating complete compressed fragments and retaining
checkpoints costs work. Runtime observations remain exact; these measurements
do not establish hot-path or startup parity on external libraries.

TOML jobs, CLI override and both out-of-range rejection paths pass on the pinned
release. Focused checks establish deterministic worker bytes/admission, including
cold/reused receipts and resource refusal. There was no full library rerun.
