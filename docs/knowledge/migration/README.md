# Active compression-verification migration

Parent: [knowledge tree](../README.md). Verification:
[verification](../verification/README.md). Live state:
[board](board/README.md). Architecture plan:
[07 — global compressor](07-global-compressor.md).

This file is the **order** of the work and its **current status**. The
[board](board/README.md) is **where each task is**. Resume cold with
`node scripts/board.mjs status`.

The **main goal** is independently authored LilScript/JavaScript pairs that prove
compressability, plus a compiler that chooses representations by typed proof and
complete-artifact score — not by another peephole or port TOML. Each gated metric
compares the compiler's own JS to the smallest valid Terser / Oxc / esbuild /
Closure ADVANCED artifact of the JavaScript source. LilScript must be **no
larger**. A strict-win case must be smaller.

## Where we are (2026-08-28)

Two tracks. They are not sequential leftovers from an older catalog-era plan.
The catalog-era 00–09 files are gone.

| Track | What it is | State |
|---|---|---|
| **00–06 — evidence loop** | Paired cases, catalog, algorithms | **Standing.** 47 hand-authored `canonical/` folders. Runner, catalog, and `comparison/algorithms/` are wired into `scripts/release-check.sh`. Last recorded full `comparison/cases` pass in the journal: 617/617 (2026-08-25). Keep adding folders. Never stop running `--canonical-only` when a fold lands. |
| **07 — decision system** | Proof → legal representations → scored family, so **size-first library** compiles get smaller | **Current architecture work.** Contract and refuse-list: [07](07-global-compressor.md#size-first-library-contract). 07.1 remainder is [ident-02](board/notes/ident-02.md). Do not add size tactics while identity folds still rematerialize rebound receivers. |

Pressure ports (jQuery, markdown stack, marked, PostHog) are **not phases**.
Classify a loss before any compiler change
([objectives](../compilation/objectives.md),
[compressor surface](../language/compressor-surface.md)):

1. compiler bug (identity/search ranked invalid JS, unsound coalescing);
2. missing language proof (07.7 RFC);
3. port still written as JavaScript (`JsValue` bags, vendored host);
4. legitimate dynamic hatch (clsx).

None of those is a library matcher in `js_peephole`.

## Invariant

For every eligible case and metric `m` in `{raw, gzip-9, Brotli-11}`:

```text
size[m](LilScript compiled with cost_model = m)
  <= min(size[m] of eligible JS minifiers, including Closure ADVANCED where that lane exists)
```

A semantic mismatch is red before size. One LilScript artifact does not have to
win every codec; each `cost_model = m` compile is gated on metric `m`. A size
loss is a compiler bug, glue-TS `.lil`, or a missing language proof. Parameter
copies of one fold do not count as coverage.

## Phases

| Phase | Status | Purpose | Exit |
|---|---|---|---|
| [00](#phase-00) | standing | Folder-per-case runner | Runner is the daily loop; keep using it |
| [01](#phase-01) | standing | Integers, numbers, bools, strings | Scalar families stay `le`; no new gated loss |
| [02](#phase-02) | standing | Branches, loops, closures, DCE | Control/function families stay `le`; named `lt` stays `lt` |
| [03](#phase-03) | standing + hole | struct/class/enum vs JS objects | Typed layout stays a Brotli win; exported constructor identity waits on 07.4 / 07.7 |
| [04](#phase-04) | standing | Arrays, records, maps, throw, generators, tasks | Edge families stay green; host stays `extern` |
| [05](#phase-05) | standing + live bug | Modules, lazy, codec search, bugs the suite finds | Every red row has a board note; ident-02 is the live identity hole |
| [06](#phase-06) | standing | Canonical + catalog + algorithms block release | Those lanes stay green; [gate-01](board/notes/gate-01.md) is a separate codec-contract red |
| [07](07-global-compressor.md) | **current** | Size-first libraries via proof + search, not glue | [Library contract](07-global-compressor.md#size-first-library-contract): typed ports stay wins; class/plain-data holes close; >16 KiB search finishes; zero library matchers |

## Working rules

1. Add several related folders, run `--canonical-only` (or `--only family/`), then broaden.
2. Compare **compressed minified JS** vs **LilScript compiler output** under `lilscript-codec`. Never post-minify LilScript for the gate. Terser-on-our-artifact is diagnostic, not a tactic.
3. If LilScript is larger, classify before coding: compiler bug, missing proof, glue-TS port, or legitimate hatch.
4. `lt` is a named typed advantage. Ordinary portable code is `le`.
5. The generated catalog remains a regression net. Canonical folders are the reviewed *why*.
6. A fold that only pays on one port is glue. A syntax that lets every port state a fact Terser guesses is 07.7.

## Ownership

- This file owns order and phase status.
- [`board/`](board/README.md) owns live task status and notes.
- [07](07-global-compressor.md) owns the architecture sequence (07.1–07.7).
- [Verification](../verification/README.md) owns measurement meaning.
- `comparison/cases/canonical/` owns the hand-authored corpus.
- `comparison/cases/catalog.mjs` owns generated variants.
- `comparison/algorithms/` owns multi-function challenges.

## Phase 00

Layout: [case layout](../verification/case-layout.md). Runner:
[`comparison/cases/README.md`](../../../comparison/cases/README.md).

**Exists.** A case is a directory a human can open:

```text
comparison/cases/canonical/<family>/<id>/
  case.toml      # expect = "le" | "lt"
  main.lil
  main.js
  README.md      # required for lt and for any non-obvious contract
```

Families on disk: `scalars/`, `strings/`, `control/`, `functions/`,
`aggregates/`, `wins/`, `collections/`, `effects/`, `host/`.

```sh
nvm use
node comparison/cases/run.mjs --canonical-only
node comparison/cases/run.mjs --only aggregates/
```

The runner compiles `main.lil` with the three gold configs, minifies `main.js`
with Terser, Oxc, and esbuild, executes every artifact, and gates each LilScript
objective against the metric-specific JS minimum. `--canonical-only` discovers
every `case.toml` under `canonical/`. Artifact paths replace `/` so build
outputs stay flat.

Still add: any minimized identity or search hole (phase 05 / 07.1). Do not
restart the runner.

## Phase 01

Coverage: [coverage matrix](../verification/coverage-matrix.md).

**Exists:** `canonical/scalars/`, `canonical/strings/`. JS must use `|0` after
ordinary LilScript `int *`. Do not pair `Math.imul` unless the LilScript source
calls `Math.imul`. Keep every scalar case `le` in raw, gzip-9, and Brotli-11.

## Phase 02

Language: [control](../language/control-flow-errors.md),
[functions](../language/functions-closures-generics.md).

**Exists:** `canonical/control/`, `canonical/functions/`. `lt` is for proven DCE
and identical-function folding. A loop both sides must run is `le`.

Do **not** try to close the jQuery `if(` / `?:` gap by adding more statement-`if`
cases or a peephole that invents ternaries. Source expression-if and scalar
literal match now lower directly to conditional phis; `?` remains nullable.
Statement versus expression spelling is an IR family, which jquery-01 already
measured: post-hoc contraction lost.

## Phase 03

Language: [aggregates](../language/aggregates.md),
[compressor surface](../language/compressor-surface.md). Compilation:
[aggregate lowering](../compilation/aggregate-lowering.md),
[class identity](../compilation/class-identity.md).

**Exists:** `canonical/aggregates/` (`struct-point`, `class-scale`,
`class-counter`, `enum-dispatch`, `nested-struct`), `canonical/wins/`. Identity-free
`class-scale` / `class-counter` must stay dissolved (`lt`).

**Hole:** there is no `aggregates/exported-class-identity` parity case yet.
`export class` is type-only by language contract. Add that folder when 07.4 can
emit named `class` for an identity-observed constructor **value**, without
flipping the type-only default.

If a case loses, classify before blaming layout search.

## Phase 04

Language: [collections](../language/collections-intrinsics.md),
[async](../language/async-generators-regex.md).

**Exists:** `canonical/collections/`, `canonical/effects/`, `canonical/host/`.
Parity first. Strict wins only where a typed intrinsic removes a JS shape the
minifier must keep. Host touches stay `extern` / `JS.*`. Ordinary-`{}` vs
null-proto `Record<T>` is 07.7, not a collections fold.

## Phase 05

Compilation: [objectives](../compilation/objectives.md),
[candidate search](../compilation/candidate-search.md).

When a canonical case fails, keep the folder and add the smallest extra folder
that isolates the bug. Module/lazy cases join once a fair JS bundler baseline
exists for that boundary. Until then, do not compare a closed script to an ESM
library.

**Live:** [ident-02](board/notes/ident-02.md) — rematerialization folds must share
one receiver-liveness check. ident-05 (search ranking invalid JS) has landed.

## Phase 06

Gates: [release gates](../verification/release-gates.md). Board:
[gate-01](board/notes/gate-01.md).

Canonical folders, the generated catalog, and `comparison/algorithms/` stay
green and already run from `comparison/run-all.sh`. 500 catalog entries is not
completion. Completion is: no unowned coverage-matrix cell, no gated loss, and
release-check staying green.

`gate-01` is independent: five runners import Node compressors, so
`benchmarks/codec-contract.test.mjs` is red at HEAD. Do not weaken that pattern
list to make identity work look green.

## Phase 07

Full sequence and **size-first library contract**:
[07 — global compressor](07-global-compressor.md).
Objectives: [objectives](../compilation/objectives.md).
Registry: [decision registry](../compilation/decision-registry.md).
Surface: [compressor surface](../language/compressor-surface.md).

A library compiled `priority = size-first` must match or beat pinned Terser /
Oxc / Closure ADVANCED configurations on the declared equivalent supported
release corpus, with designated typed wins remaining strict, and must not stay
stuck behind starved search, type-only class, or `assume_*`. Root
`lilscript.toml` is a
language-test compression **subset** — it is not the size-first library
profile. Glue (library matchers, post-minify, one-way Brotli doors) is how
we fail that even if raw bytes move.

| Step | Board | Status |
|---|---|---|
| 07.1 Identity before search | `ident-05` landed; `ident-02` active; `ident-03`–`ident-04` todo | ident-05 **landed**; 02 active |
| 07.2 Contract, provenance, and one registry | `arch-02` | blocked on ident-02 |
| 07.3 Reversible priors | `arch-03` | blocked on ident-02 |
| 07.4 IR emits legal aggregate/closure/property shapes | `arch-04` | blocked on ident-02 |
| 07.5 Hygienic target AST; peephole is contraction | `arch-05` | blocked on arch-04 |
| 07.6 Search that can finish | `arch-06` | blocked on arch-02 |
| 07.7 Language proofs + explicit lowering contracts | `arch-07` | todo — RFCs/cases, not flags; source `\|0` and library ABI are contract tests |

07.7 may be drafted in parallel with 07.1. It must not land as optimizer flags
first. The 07.4 named-class representation enables constructor-value syntax for
published classes; plain-data unblocks deleting
`assume_pure_property_reads` from port TOMLs; expression-if
unblocks treating `local_phi_expression_regions` as recovery of source. The
same contract lane distinguishes source-authored `|0` from generated integer
normalization and freezes reusable-library ABI independently of objective.
