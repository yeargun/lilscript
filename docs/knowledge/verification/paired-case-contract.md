# Paired-case contract

Parent: [verification](README.md). Layout: [case layout](case-layout.md).
Measurement: [codecs](codec-measurement.md).

## Unit of evidence

A case is an independently reviewable LilScript program and JavaScript program that
implement the same declared observable contract. They need not have matching syntax
or object layout unless layout is part of that contract. The pair should be idiomatic
for each language and should expose the optimization idea named by the case.

## Required equivalence

Choose the strongest applicable oracle:

- exact stdout/stderr and exit status;
- return-value serialization with explicit handling for `NaN`, `-0`, `undefined`,
  bigint, cycles, and identity;
- ordered effect trace;
- public API descriptor/arity/constructibility/throw behavior;
- DOM/event/network snapshot in a real browser;
- complete artifact/module/chunk behavior.

Generate expectations from a declared semantic specification or independently
trusted source, not whichever compiler happened to run first. Today’s micro runner
executes the reference JS and verifies a checked-in digest covering that source,
stdout, case name, and strictness. This makes changes explicit and reproducible, but
it does not make the JS oracle independent; phase 01 must add reviewed specifications
or expectations where a shared source mistake could bless both baseline and oracle.

## Fairness rules

- Same target environment and ECMAScript floor.
- Same public boundary: closed app vs closed app, reusable module vs reusable module,
  script-tag facade vs script-tag facade.
- Same observable numeric and error semantics. Ordinary LilScript `int`
  multiplication is JavaScript binary64 `*` followed by signed-i32 normalization,
  while the explicit `Math.imul` intrinsic uses `Math.imul`; the reference must not
  substitute one for the other. JavaScript may need `|0` or explicit checks to match
  the declared `int` operation, and those are contract costs, not sabotage.
- No unused padding, artificial long names, repeated dead work, or avoidable dynamic
  bags added merely to make JavaScript lose.
- Both sources may use their language’s natural representation. LilScript is allowed
  to win because a fixed shape is a `struct`; JS is allowed to use the clearest
  ordinary fixed-shape representation. A `.lil` that is a `JsValue` transliteration
  of the JS side is not that pair — it is glue-TS and is classified before size
  ([compressor surface](../language/compressor-surface.md)).
- Baseline options are fixed by lane, never tuned per losing case unless the tuning is
  available symmetrically to all cases and recorded.

## Size gate

For metric `m`, define:

`baseline_min[m] = min(size[m] for every semantically eligible baseline artifact)`

Let `lilscript_objective[m]` be the independently compiled LilScript artifact whose
configured cost model is `m`. An ordinary case passes when
`size[m](lilscript_objective[m]) <= baseline_min[m]`; a strict case passes with `<`.
Do not require one LilScript artifact to dominate every codec. Its non-selected
metrics are diagnostic and may regress. Select the baseline minimum independently
too: Terser may win raw while Oxc wins gzip and Closure wins Brotli.

Aggregate totals and family packs are secondary evidence. They may show codec context
effects but cannot hide a required per-case loss.

## Boundaries and variants

Each result names one boundary and one config. Public and closed-app outputs are
separate rows. Fast and release search are separate rows. A variant may share source
files but must not overwrite another variant’s emitted artifacts or report.

## Ineligible baselines

A baseline can be excluded only for a recorded reason: parse/transform failure at the
declared target, semantic mismatch, inability to preserve the public boundary, or
tool crash. Being larger is never an ineligibility reason. Keep the log and version.
