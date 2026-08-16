# Negative results and non-wins

Parent: [evidence](README.md). Triage:
[failure triage](../verification/failure-triage.md).

Negative evidence is part of the compiler specification. Keep it when it identifies a
non-monotonic transform, missing proof, invalid boundary, or search-budget failure.

Current examples:

- jQuery's latest checked-in pre-canonical public row is ineligible and larger than
  npm in all recorded size columns; historical full-mangle/post-minified checkpoints
  do not reverse that result. Its exact current canonical bytes remain pending.
- More jQuery inlining and a broad conversion from bound arrows to `this` methods
  regressed the measured artifact; higher inline limits are not assumed smaller.
- Region outlining often wins raw and loses gzip/Brotli, so the canonical pass is off
  and only an allowed codec-scored search may probe it.
- Applying Oxc to already selected LilScript app output shortened raw in a historical
  audit while worsening Brotli on four of five rows. Post-minification remains a
  proposal, not a free final stage; rerun before quoting that count as current.
- A legal comma/conditional rewrite on a broad surface displaced a stronger bounded
  beam candidate, motivating size caps and better frontier tests.
- Pre-canonical focused edge summaries for generators, closures, arrays, exceptions,
  and records are retired: system/Node codec measurements and the former Brotli
  scorer are not interchangeable with the shared canonical scorer. A focused
  `--only` report may still be useful for triage, but it cannot establish a family or
  525-case result. Capture final canonical family runs and the full catalog before
  quoting current pass counts or byte deltas; no win offsets a remaining per-case
  failure.
- The record/JSON investigation exposed a block-local proof limit and led to the
  closed-record observation-projection candidate. The optimizer keeps the
  unprojected artifact, projects only proven immutable observations, and retains
  mutable/unknown/phi/host bailouts. Ordinary `{}` backing for a surviving
  `Record<T>` remains ineligible because it would violate the observable
  null-prototype contract. Its old focused byte ranges are withdrawn pending a
  canonical rerun.
- Results from the initial six structural challenges are superseded by the audited
  11-pair/42-vector corpus. The JavaScript frontier and behavior/fairness audit are
  complete. The first full canonical run's 3/11 result remains useful migration
  evidence, but it is superseded as current status by the post-fix 11/11 full report:
  zero failure events and observed strict wins in all 11 raw, 11 gzip, and 11 Brotli
  lanes. Its large-event row is `803/475/424` against Closure
  `853/489/425` in the matching objectives.

For every negative result, preserve exact artifacts/config/tool versions when
practical, minimize the reproducer without erasing codec context, classify semantic vs
size vs eligibility, and add a regression once fixed. Archive stale numeric logs, but
keep the lesson linked from the active plan.
