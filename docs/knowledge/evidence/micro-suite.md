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

Hand-authored [`canonical/`](../../../comparison/cases/canonical/) folders are a
separate reviewed corpus. The latest `--canonical-only` run was 47 cases, all
strict wins in raw, gzip-9, and Brotli-11 against the metric-specific Terser/Oxc/
esbuild minimum. That does not replace the generated catalog; it is the
readable "why" layer.

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
- The reviewed generated-catalog oracle is 549 cases; see
[`oracle-manifest.json`](../../../comparison/cases/oracle-manifest.json).
Do not quote an older digest from this page. The generated `summary.json` is
authoritative only when `selectedBy` is `all` or `canonical` as appropriate.

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
