# Q2 bounded content-addressed build reuse

Implement an optional whole-build artifact cache behind strict TOML and existing
`--cache DIR|off` controls. An unchanged complete request may reuse its checked,
optimized and admitted outputs plus deterministic logical receipt. Fresh module
resolution and complete source/embedded-host identities precede path hits; the
compiler/encoder identity, full configuration and service ceilings qualify each
record. Any mismatch, corrupt/oversized record, IO failure or explicit audit
setting falls back to a normal build. Deadline-limited builds remain uncached.

Use fixed disk capacity with checksummed records, no unbounded key directory or
retained IR graph. Report current physical cache work separately from the reused
cold logical/search receipt and the previous run's timing. Keep checked-session
callbacks cold: a persisted artifact is not a live source proof.

After the coherent batch, check changed transitive files, newly resolved files,
embedded host changes, policies, objectives, resource limits, native handoff,
corruption/collisions/concurrent writes and all-objective cold/warm/disabled
behavior. Measure affected repeated builds; no full library/fleet rerun.

Prediction: warm complete requests avoid checking, normalization, formation,
search and encoding. Fresh resolution and cache IO may cost more for tiny or
changed builds. This does not claim per-module elaboration caching, incremental
formation, decision-lock replay or complete remaining resource admission; those
remain Q2 obligations.

## Result

Whole-build reuse is implemented for the public compile-source/path/entries APIs
and CLI, including all-objective, multi-file and native C/header handoffs. Strict
`cache.build_reuse` defaults true but needs an explicit cache directory; audit
flags and deadline-limited policies bypass it. Fresh discovery includes current
symlink/package resolution and embedded-host body/linkage identities. Cold
compilation must report the same actual inputs before its result is stored.
The shared compiler/encoder fingerprint owner also serves codec receipts.

One checksummed 64-slot file has a maximum extent of 268,440,576 bytes. Each
record is bounded by 4 MiB and one eighth of the effective retained-byte ceiling;
oversized, corrupt, colliding, concurrently inconsistent or inaccessible records
miss safely. Artifact digests, winner indices and delivery references validate
before publication. Unmeasured low-effort codec scores remain unmeasured.
Checked-session callbacks always run the frontend. No borrowed proof crosses
processes; only complete detached handoffs and their logical receipts do.

[12 distinct focused tests](focused-tests.json) pass, plus schema agreement and
[release CLI controls](cli-controls.json). Coverage includes transitive/host/linkage
changes, retargeted symlinks, configuration and service ceilings, hard vetoes,
all objectives, multi-entry delivery, native C handoff, IO/refusal/corruption and
concurrent records. Initial new fixtures used unsupported single-quoted source
strings and a not-yet-supported native public export; those were corrected, and
native public-export refusal remains asserted. An overstrict reader initially
rejected valid unmeasured level-0 scores; the corrected handoff preserves None.
The first parity assertion also incorrectly equated physical codec scratch peaks;
logical bills and cache-hit copies of the original resource receipt are checked
separately. Only failed/affected cases were rerun. No full suite was run.

Pin `/home/azureuser/lilscript-work/bin/q2-build-cache-1/lilscript`, SHA-256
`8776c5e0224e0a6b1ef2e5d94cb83bc18bf7bac26906eae7f5ce3d7da6268a93`,
algorithm 49 / search 32 / walk 10 / local facts 15. [Source identity](identity.json),
[comparison](comparison.json), and [runner](measure.mjs) retain the result.
All **72** release artifact/runtime checks pass: three generic controls × three
objectives × previous/disabled plus three cold/warm pairs. Current modes retain
identical files, logical bills, search counts and semantic policy fingerprints.
All prior outputs match: **6,615 raw / 1,715 gzip / 1,001 Brotli**, zero deltas.

| Control | Objective | Cold CPU seconds (three samples) | Warm CPU seconds (three samples) |
|---|---|---|---|
| 128 independent bodies | raw | .13 / .13 / .13 | .01 / .01 / .01 |
| 128 independent bodies | gzip | .12 / .12 / .12 | .01 / .01 / .01 |
| 128 independent bodies | Brotli | .15 / .14 / .14 | .01 / .01 / .00 |
| Spelling choices | raw | .08 / .07 / .07 | .01 / .01 / .01 |
| Spelling choices | gzip | .08 / .08 / .09 | .01 / .01 / .01 |
| Spelling choices | Brotli | .14 / .14 / .14 | .01 / .01 / .01 |
| Two-file setter | raw | .02 / .02 / .02 | .01 / .01 / .01 |
| Two-file setter | gzip | .01 / .01 / .01 | .00 / .01 / .01 |
| Two-file setter | Brotli | .05 / .05 / .05 | .01 / .01 / .01 |

These are coarse `/usr/bin/time` samples; .00 is below its displayed resolution,
not free work. Warm hits execute zero source effect summaries, target passes or
encodes. Fresh discovery/parsing, fingerprinting, IO and decoding remain. Cold
cache builds cost about .01–.02 seconds more than disabled builds on these
controls, and the tiny gzip case has no clear benefit. Record payloads span
43,798–73,356 bytes here. No fleet/general speed or memory-peak claim follows;
cache IO/returned buffers remain separate from the reused cold compiler ledger.
The report separates current elapsed/cache work from the old phase/codec times.

No ratchet baseline or port artifact changed. Q2 remains active for per-module
elaboration reuse, incremental formation, decision locks, remaining global rule
dependencies and resource admission.
