# Library comparisons and source-build records

The deliverable is [report-library-drift.html](../../report-library-drift.html), a
single offline HTML file containing all 27 current library comparisons, source
build measurements, limitations and verified GitHub Pages deployment records.
Public pages retain their existing designs and show current ESM results and
machine/build facts. They do not contain compiler-history or before/after panels.

All 27 original Git repositories were built with their native commands. Twenty-
five LilScript package builds complete and 23 recorded repository commands pass.
Vue/Monaco compilation and React Markdown/Remark checks remain incomplete. The
per-library records identify the exact source and compiler used; partial API
coverage remains explicit.

## Recorded workspaces

- `/tmp/lilscript-page-refresh-20260910/publications.json` maps every library to
  its publication checkout, commit, workflow and verified live page.
- `/tmp/lilscript-source-performance-20260910` contains the 27 paired source-build
  jobs, original checkouts, raw results and artifact measurements. Its compiler
  is `4dc4e3337d9ffd758d65e3852682d6e13d9331c1`, publicly available on
  `yeargun/lilscript` branch `comparison-source-build-2026-09-10`.
- `/tmp/motionlil-cost-audit-20260911/current` contains the Motion runtime fixes,
  narrow-import correction and current publication.
  `/tmp/motionlil-cost-audit-20260911/build` contains
  the paired source builds and 30-pair browser measurements. The compiler is
  `e5f7f254470ae4af178f3625b8b3dfd8fe501ad7`, publicly available on
  `fix/inherited-field-analysis`, with draft PR 2 targeting the public-class
  defaults fix. Both compiler fixes are also applied to the user's compiler
  workspace, preserving existing edits. Earlier measurements remain in
  `/tmp/motionlil-runtime-20260910/build` and
  `/tmp/motionlil-behavior-20260911/build` and
  `/tmp/motionlil-behavior-final-20260911/build`; do not publish their performance
  figures with the current distribution.

Both build lanes run three times on the same Azure Standard_D16als_v7 worker,
with alternating order, cleared outputs and Node 24.11.1. Installation is
recorded separately and excluded. Original production ESM assembly is separately
measured after the native build. Package output formats and checks differ, so
the report presents contextual build times rather than speedup ratios.

`source-build-worker.py`, `compiler-wrapper.py` and `source-esm.mjs` capture the
builds. `jobs.json` pins native commands, sources and release-specific dependency
adjustments. Raw logs and lockfiles are published under each repository's
`comparison/source-build/`; `site/source-build.json` consolidates the evidence.
Do not regenerate jobs blindly or replace the recorded dependency pins.

`publish-source-builds.py RUN --ports motionlil` refreshes measured data; set
`PUBLICATIONS_FILE` to select an isolated publication map. The source/artifact
guard in `scripts/build-comparison.mjs` rejects stale measurements. Special size
row handling for cn, KaTeX, PostHog and PlayCanvas must be preserved.

## Motion runtime and performance

Motion's final comparison uses its original source-built ESM and the exact
`motionlil/full` ESM used for the size comparison. It exposes all 312 original
export names, but extended constructor/layout adapters remain incomplete. The
52-export default entry is reported separately and cannot support a whole-library
compression claim. Thirty paired browser trials follow
two warmups per lane, alternate order and use a fresh page for each trial.
Identical DOM, CSS, keyframes, easing and duration are required. Seven workloads
must match native backends/options, sampled values and final values before
publication, including full `animate()` string transforms. Consult the current
recorded intervals for performance conclusions.

Motion's `scripts/measure-performance.mjs` captures CPU, script, style/layout,
setup and observer frame-cadence measurements. Library RAF requests, executions
and queued callbacks are recorded separately in untimed validation. The
`scripts/check-natural-performance.mjs`
inspects every animated property of every fixture element over three natural
playbacks per lane. Intrusive style reads stay outside the CPU measurement.
Exact inputs, hashes, samples and protocols are published in `site/performance/`.

`refresh-motion-performance.py RUN REPO` updates the existing performance table
from the final results and verifies that size/performance inputs match. The old
`motion-performance-page.py` is the initial one-time migration and must not be
rerun over the current checker or methodology.

The runtime uses typed timing options, an enum, typed callback references,
checked pure helpers, shared public MotionValue methods and reused generator
state. Its frame loop follows upstream's reusable Sets and weak keep-alive
tracking. Native eligibility, generic keyframes, native spring easing and control
lifecycle follow the original. Native control accessors query only the browser
properties needed by the requested getter. Motion's
`comparison/runtime-investigation/README.md` documents the implementation and
37 browser checks. Maximum IR optimization and JavaScript optimization level 0
remain the recorded production configuration. All 1,626 compiler library tests
pass; broader compiler CI still reports its existing formatting and differential
generator failures.

The cost audit records controlled compiler builds, the broad cast import that
retained unrelated initialization, corrected entry scope and unchanged-reference
session effects. See `comparison/runtime-investigation/cost-audit.json` in the
Motion publication. The mini consumer size gate is restored to 20,000 raw bytes.

The subsequent structural size diagnosis is recorded in
`motion-size-rootcause.json`, with scripts and logs under `motion-size-rootcause/`.
Its 50-export experiments, emitted type-check counts and compiler-level variants
are unshipped diagnostics. Level 9 passes the existing 37 browser checks with the
full entry replaced, but has no new paired performance record. Level 13 fails on
an unresolved generated export for `press`. The report distinguishes this evidence
from the live, verified publication.

## Report and publication checks

```sh
python3 comparison/page-refresh/source-build-report.py
node comparison/page-refresh/check-report.mjs /home/azureuser/lilscript/report-library-drift.html /tmp/motionlil-cost-audit-20260911
python3 comparison/page-refresh/deployments.py /tmp/lilscript-page-refresh-20260910 --pending
```

The report check covers offline rendering, filters, evidence download and mobile
layout. Deployment checks verify the exact published commit, live comparison
JSON and machine/build facts. Stylesheet hashes verify the preserved designs.
The earlier `publish.py`, `report.py` and audit-panel scripts document the initial
investigation; use the current source-build publisher/report for these pages.
