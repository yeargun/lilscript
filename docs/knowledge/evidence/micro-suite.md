# Paired web micro suite

Parent: [evidence](README.md). Executable contract:
[`comparison/cases/README.md`](../../../comparison/cases/README.md). Verification
policy: [paired cases](../verification/paired-case-contract.md).

`comparison/cases/` is the shortest feedback loop for the user's central invariant:
independently authored LilScript and JavaScript run the same stdout contract, then
each objective-specific LilScript artifact must not exceed the strongest valid
JavaScript artifact in its matching gated size metric.

## Current runner contract

- The durable `catalog.mjs` materializes ignored per-case folders; a checked-in oracle
  manifest hashes the full catalog's JS source, stdout, name, and strictness.
- At the shared ES2022 target, Terser uses three top-level compression passes, Oxc
  uses top-level mangling through pinned Rolldown, and esbuild emits both a script
  transform and a closed-IIFE/top-level-mangling candidate. Every candidate executes;
  invalid output fails the case and is excluded from minima.
- LilScript compiles independently under raw, gzip, and Brotli size-first gold
  configs with `candidate_search = "always"`; this makes the declared 1536 limit
  effective instead of silently using production's 384 cap. It is not post-minified.
- Each metric selects its own minimum valid JS artifact. Gzip is level 9 with
  deterministic time; Brotli is quality 11 and `lgwin=22`. There is no small-file
  exemption.
- `le` cases require each objective-specific build to be non-losing in its matching
  metric; `lt` cases require strict wins in those three independent lanes. A
  Brotli-selected artifact is not required to win raw or gzip, and vice versa.
- Unless both `LILSCRIPT` and `LILSCRIPT_CODEC` select an explicit pair, the runner
  builds fresh release compiler and scorer binaries together. Supplying only one
  override is rejected. The scorer must still pass schema, static-codec provenance,
  binary-digest, and exact-fixture checks; reports retain both executable hashes so a
  standalone diagnostic run cannot be mistaken for the repository's joint build.
  Schema 5 records exact options, canonical scorer and
  runtime/platform/Oxc-binding identity,
  source and emitted-artifact hashes, candidate durations, and
  compiler/tool/config/runner/corpus/oracle provenance. It is reached by release
  check through `comparison/run-all.sh`.

## Expansion status

The catalog now contains 525 unique generated cases after adding 125 `edge-*` cases
covering i32, numbers, short-circuit effects, loop control, closure mutation, arrays,
records/JSON, Map/Set, nullish/optional access, UTF-16 strings, exceptions/finally,
generators, and async tasks.

The pairing audit repaired reference evidence rather than changing compiler results:

- 195 cases contained 260 JavaScript expression/call sites where ordinary LilScript
  `int *` had been paired with `Math.imul`. They now use ordinary binary64 `*`,
  signed-i32 normalization, and the exact nested operation order. Old and new
  references happen to print the same values for the current inputs, but only the
  repaired form represents the language outside those samples. The current catalog
  has zero `Math.imul` calls; if a future LilScript case explicitly uses that
  intrinsic, `add()` requires the reference to have the same explicit-call count in
  [`catalog.mjs`](../../../comparison/cases/catalog.mjs).
- Five enum-dispatch pairs now give JavaScript the same numeric-discriminant model
  instead of charging it for a metadata object. Six host-has-own pairs use the
  target's direct `Object.hasOwn`, including its first-class-function form.
- The reviewed full-catalog oracle is 525 cases with SHA-256
  `0d1f156cc7863a88b464250f00c0a38831b0565208e81159bf154576aa0ee052`; see
  [`oracle-manifest.json`](../../../comparison/cases/oracle-manifest.json).

These repairs and the move to the shared canonical `lilscript-codec` scorer supersede
earlier focused totals. The authoritative full-catalog run has `selectedBy = "all"`,
`catalogCases = 525`, `cases = 525`, `passedCases = 525`, `failedCases = 0`, and
`failureEvents = 0`. Its matching-lane strict/tie/loss counts are raw `525/0/0`,
gzip-9 `522/3/0`, and Brotli-11 `523/2/0`. Ties are passing non-losses for their
`le` cases; they are not counted as strict wins. The frozen compiler SHA-256 is
`6962cea23cf08148e2d2fc76b9f1c2a7ca6876c5e488d56da1f1bcb5341de897`, and the
frozen scorer SHA-256 is
`975bf256f7c8dfdeb5864a9e96d1a26e43a8a90726dc2227daf1026c2f1d20e7`.

The current ignored summary may be overwritten by a narrow `--only` diagnostic run,
so do not cite it as a full-catalog result unless `selectedBy` is `all`, its schema is
current, and its corpus/oracle/scorer provenance matches. Every red size or semantic
row remains a per-case failure; aggregate wins cannot offset it.

`summary.json`/`summary.md` are generated and may be overwritten by a focused run.
Always check `selectedBy`, `catalogCases`, `cases`, `passedCases`, and `failedCases`
before quoting it as full-corpus evidence.

## Limits

Stdout derived from the reference JS is reproducible but not an independent semantic
specification. The lane also does not prove public API descriptors, browser host
behavior, module artifact sets, or arbitrary JS. Those contracts live in later
migration phases.
