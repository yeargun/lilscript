# Q2 dependency-driven effect components

1. Keep one previous effect analysis during source normalization. Reuse complete
   recursive-component results only for matching body/table revisions, resolved
   storage/callees, dependency summaries and initialization answers. Changed
   dependencies invalidate callers; unknown/truncated summaries stay conservative.
2. Share unchanged component results between structural and scheduled
   initialization passes. Within a recursive component, rerun only consumers
   whose callee summaries changed, retaining structural order and the convergence
   ceiling. No semantic change or extra search permission follows from reuse.
3. Extend the existing normalization audit control to this physical reuse;
   retain bounded phase-local ownership and report actual summary visits/reuse.
   Compare cold/dense and reused results, recursive/callback/capture and TDZ
   invalidation, all-objective artifacts and deterministic logical work after a
   coherent batch. No full library/fleet rerun.

Prediction: fewer effect-summary executions when other bodies change, without
changing bytes or semantic facts. Conservative table/storage invalidation and
extra comparison/storage costs may limit the benefit. This batch does not close
other Q2 resource owners or the build-cache/decision-lock obligations.

## Result

Whole recursive components now reuse effect summaries under exact body/table,
storage/callee and initialization dependencies. The structural and scheduled
passes share stable answers; dirty recursion retains the original order and
iteration ceiling. Constant-only edits no longer spuriously revise cell tables.
`cache.normalization_reuse=false` forces cold summary execution, and the phase
releases previous analyses before publication. Other global rule dependencies
and comprehensive source analysis/edit admission remain open.

The comparison found an existing compiler failure on the mutually recursive
even/odd control: gzip and Brotli fail in the previous pin. A conditional
spelling negated its condition before consulting the selected alternative;
declining the rewrite left that mutation behind and flipped it every round.
Eligibility now plans the negation without mutation and commits it only when
selected. An initial expression-inlining hypothesis was rejected and its
speculative cycle guard removed. This is a correctness/convergence repair, not
a compression win. The round ceiling remains unchanged; refusal now identifies
the last changing rule.

[41 distinct focused tests](focused-tests.json) pass. They cover cold/reused
facts, recursive convergence/truncation, callbacks, storage/callee/table/TDZ
invalidation, bounds traps, local scheduling, native captured activations,
logical-work parity, disabled/selected conditional spellings and independent
Node behavior. Two initial new fixtures were corrected to exercise actual
index operations and initialization boundaries, then rerun alone. The late
conditional fix ran six affected tests in 28.59 seconds. No full suite ran.

Pin `/home/azureuser/lilscript-work/bin/q2-effect-components-2/lilscript`, SHA-256
`4cd627217bd2d5a5208fe65aee4bca6d56f74a1e9fd3afdcdeee8afbd507eac8`;
algorithm 48 / search 32 / walk 10 / local facts 15. [Identity](identity.json),
[comparison](comparison.json) and [runner](measure.mjs) retain the evidence.
The preliminary pin is retained in [initial-pin-identity.json](initial-pin-identity.json);
its comparison stopped on the old compiler failure.

All 61 successful release artifact/runtime checks pass: seven controls under
three objectives and three modes, excluding the two previous-pin failures.
Current reused/disabled modes match every file, logical bill, search count and
semantic policy fingerprint. All matched prior outputs are byte-identical:
**2,486 raw / 1,136 gzip / 867 Brotli**; the gzip/Brotli totals exclude the
repaired recursive control (92 gzip / 71 Brotli bytes in the new pin).

Across the 21 current case/objective pairs, physical source effect summary
executions fall **1,278 → 390**. Independent bodies fall 132 → 65, recursive
components 20 → 7 and spelling controls 60 → 19 per objective. These counters
exclude graph/key construction and retained-analysis copying. Coarse CPU
samples show no clear overall gain; source analysis backing is still outside
complete ledger coverage, so unchanged reported peaks do not establish an
unchanged physical memory peak. No fleet, runtime-speed or compression gain is
claimed. No ratchet baseline or port artifact changed. Q2 remains active.
