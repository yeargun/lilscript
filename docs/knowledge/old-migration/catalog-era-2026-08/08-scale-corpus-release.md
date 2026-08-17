# Phase 08 — scale, applications, and release gates

Parent: [migration](README.md). Promotion rules:
[release gates](../verification/release-gates.md). Failure policy:
[triage](../verification/failure-triage.md).

## Objective

Scale beyond 500 maintained paired cases without turning case count into the claim.
Promote a layered evidence system that catches tiny compiler drift and real delivery
failures.

## Current checkpoint

The catalog has crossed the numerical waypoint at 525 unique cases, including 125
new `edge-*` cases across thirteen semantic families. The authoritative full report
has `selectedBy = "all"`, `catalogCases = 525`, `cases = 525`, `passedCases = 525`,
`failedCases = 0`, and `failureEvents = 0`. Its matching-lane strict/tie/loss counts
are raw `525/0/0`, gzip-9 `522/3/0`, and Brotli-11 `523/2/0`; all five ties remain
passing non-losses under their `le` contracts. The frozen compiler SHA-256 is
`6962cea23cf08148e2d2fc76b9f1c2a7ca6876c5e488d56da1f1bcb5341de897`, and the
frozen scorer SHA-256 is
`975bf256f7c8dfdeb5864a9e96d1a26e43a8a90726dc2227daf1026c2f1d20e7`.

Crossing 500 is **not** phase completion. Focused reports overwrite the ignored
summary; quote a count only with `catalogCases`, `selectedBy`, passed/failed cases,
failure events, and matching binary provenance.

## Scale plan

1. Pass 500 reviewed cases with coverage-balanced families; generated value variants
   are counted separately from unique semantic templates.
2. Grow combinatorial and metamorphic variants from deterministic seeds.
3. Establish the separate
   [algorithm challenge lane](../verification/algorithm-challenges.md): independently
   idiomatic pairs, fixed runtime vectors, opportunity tags, and hard per-case
   three objective-specific matching-metric gates across small, medium, and large
   structural tiers.
4. Add medium/large modules and apps that combine features and cross meaningful
   codec windows without inert padding.
5. Maintain Closure ADVANCED pairs for closed-world programs.
6. Maintain Terser/Oxc/esbuild/Vite lanes for ordinary JS and mixed apps.
7. Promote browser-host cases and runtime non-inferiority where relevant.
8. Use jQuery and other popular libraries as boundary-specific pressure tests; they do
   not become wins until exact API, behavior, size, and required performance gates
   pass. jQuery has its own [phase 09 convergence plan](09-jquery-library-convergence.md).

## Release layers

- per-change: compiler unit/conformance + changed micro families;
- merge: full paired micro suite, differential oracle, config contracts;
- release: all of the above plus the structural algorithm lane, Closure apps,
  bundle/Lilpack, browser, scenarios, popular libraries, and reproducible artifacts;
- scheduled deep search: widest beams, multiple codecs/configs/engines, corpus
  minimization, and trend reports.

## Exit criteria

- The release script invokes every lane described as release-blocking.
- Every required optimization opportunity has structural cases, and each structural
  tier reports real function/module/boundary/vector counts rather than generated
  parameter totals.
- No green aggregate hides a losing required case; totals are descriptive only.
- Tool/config/source changes require an explicit evidence refresh reviewed like code.
- Quarantines expire and block release at their deadline.
- Public claims cite generated reports and state scope. “Never larger” is used only for
  the exact eligible corpus and metrics that enforce it.

After this phase, continue adding cases when a bug, language feature, bundling mode,
or competing tool exposes an unowned coverage cell. The suite is a living compiler
specification.
