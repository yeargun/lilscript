# Candidate search

Parent: [Compilation](README.md). Philosophy: [global optima](global-optima.md). Knobs: [cost model](../config/cost-model.md), [JS optimizations](../config/javascript-optimizations.md). Code: `optimize_and_select_javascript`, `select_javascript_candidate`, `finalize_javascript_candidates`, `extend_javascript_candidate_beam` in `src/compiler.rs`.

## Two levels

### Level 1 — IR optimizer variants

Always first: `config.js_optimizer_options()`. Then opportunistic clones (each must still compile):

| Variant | Gate |
|---|---|
| `inline_closure_factories = false` | `ir-closure-factory-variants` + compression decision |
| inlining fully off | `ir-inlining-variants` |
| no constant-parameter specialization | `ir-specialization-variants` |
| no call-site specialization | `call-site-specialization` |
| no capture-signature cloning | `capture-signature-cloning` |
| reusable helpers (inline off + specialization off) | helper when both families exist |
| function subsumption on and off | `js_function_subsumption_variants_enabled` |
| phase-order: no early CSE, aggressive inline, both | `ir-phase-ordering-variants`; **broad modules** get one combined probe |
| compress passes all-off; outlining contrast; fusion off; merging off | `ir-compress-pass-variants` |
| strongest bounded aggressive-inline phase probe + outlining | phase-order variants + outlining are both legal |

Each variant: clone IR → optimize → emit with **configured** `js_options()` → score
transfer → deduplicate equivalent probe pairs → keep top `candidate_beam_width`
finalists. The configured optimizer is fixed first. With at least two optimizer slots,
the outlining contrast is reserved second; with at least three, the strongest bounded
aggressive-inline + outlining interaction is reserved third. This lets aggressive
inlining expose a repeated composite without crossing outlining with every optimizer
tuple.

A closed script whose policy enables `pure-helper-inlining` may give an IR probe one
additional emission before structural pruning. An IR whose
`repeated-region-outlining` report changed is pre-scored with `AllEligible`; an IR
with ordinary inlining disabled is pre-scored with `SingleStaticUse`. The probe rank
uses the better configured/interaction emission, while raw-growth admission still
uses the configured optimizer emission as its baseline. Deduplication keys the pair,
not just configured code, so two equal configured emissions with different useful
interactions cannot erase each other. Emission failure simply omits this optional
probe.

If that interaction makes the IR a finalist, its already rendered code and exact
options seed the terminal selector and consume a slot in that finalist's ordinary
candidate budget. This matters when a small budget cannot reach the late helper
family independently. For outlined IR, provenance keeps a multi-use outlined helper
as the shared composite while `AllEligible` substitutes its eligible leaf helpers;
an outline reduced to one call may still be substituted normally.

Byte/count budget is split across finalists. At least the configured output of each retained IR variant is measured.

### Level 2 — emission beam per IR finalist

From each finalist’s `IrJsOptions`, extend by families (pooling, elision, SSA,
loops, layout, alphabets, …) using `extend_javascript_candidate_beam`.
Candidate-level alternatives include closed-record observation projection
([boundary](aggregate-lowering.md#closed-record-observation-projection)) and an
anonymous call-site representation for a private single-use function
([boundary](inlining-specialization-sharing.md#emission-only-single-use-function-expressions)).
Neither is a user-selectable semantic mode: the relevant proof and search gates
must pass before the form is scored. Ordinary-object backing for a surviving
`Record<T>` is deliberately not a production candidate.

Two interacting emission families are expanded atomically: pure-helper policies
`None`, `SingleStaticUse`, and `AllEligible`, crossed with dense string-return tables
off/on when their respective compression decisions are legal. This matters because
substituting helpers can expose or duplicate a table, while a table can make helper
substitution profitable; pruning either isolated proposal first can lose the joint
winner. The configured `None`/off combination remains in the family, and the whole
Cartesian expansion is charged to the shared candidate/count budget.
Narrow/family widths are `beam * 2/3` and `beam / 3`.

After the structural, layout, and name families, the
`fresh-literal-factory-inlining-variants` feature may re-emit at most the best two
complete option sets. Its whole-program proof accepts only private functions used
exclusively by zero-argument direct calls whose complete body is `return []`;
substitution preserves one fresh allocation per call and removes the unobservable
declaration. The configured raw/gzip/Brotli scorer compares these late proposals
with the full incumbent frontier and still returns one artifact from the same
compiler invocation. Explicit chunk plans disable the proposal conservatively until
chunk ownership/import planning carries the same proof.

`effective_candidate_limit` = min(`candidate_limit`, level cap, search cap):

| `optimization_level` | Level cap (if no exact `optimizations` list) |
|---|---|
| 0–2 | 1 |
| 3–4 | 16 |
| 5–6 | 64 |
| 7–8 | 192 |
| 9–10 | 384 |
| 11–12 | 768 |
| 13–14 | 1024 |
| 15 | unlimited (still min’d with search cap) |

| `candidate_search` | Search cap |
|---|---|
| `off` | 1 configured emission before finalization |
| `production` (default) | 384 |
| `always` | unlimited (still min’d with `candidate_limit`) |

Exact `javascript.optimizations = [...]` **replaces** level-derived features and does not apply the level cap (`level_limit = usize::MAX`). Production search still caps at 384 unless `always`.

`candidate_byte_budget` (default 1 MiB) ÷ baseline raw size limits how many full
emissions a variant may retain, at least 1. This is an approximate retained-artifact
budget, not a strict cap on total work: rejected duplicates, alternate-objective
scores used for frontier diversity, and entropy/name probes can consume additional
codec calls before the bounded pool is retained.

Within the emission beam, bounded retention is objective-stratified: rankings for
the selected model, raw, gzip, and Brotli are visited round-robin with the selected
model first, and duplicates are skipped. The same stratification feeds structural
finalists, entropy sources, and one-character identifier mappings. This preserves
promising cross-objective shapes that a later interaction may make best for the
selected model; it does not change the terminal winner, which is still ranked only
by the configured objective and priority. The IR-optimizer probe frontier remains a
separate selected-objective beam.

## Finalize

1. Optionally add parsed-peephole clone (`parsed-peephole`, level ≥ 9).
2. Drop candidates that fail startup guard or raw-growth / transfer allowance.
3. Sort by `javascript_candidate_rank`, startup, raw, lexical. For `size-first`,
   the primary rank is the exact transfer-byte count; performance only breaks an
   exact transfer tie. The other priorities use normalized ratios for their mixed
   objectives.

Transfer scores are reused if peephole leaves the artifact unchanged (no second Brotli-11).

## Codec

`compressed_size`: raw length; gzip through statically bundled upstream stock zlib C
1.3.1 at level 9 with deterministic `mtime = 0`; Brotli through statically bundled
official Google Brotli C 1.1.0 in generic mode, quality 11, `lgwin=22`. The compiler
and the `lilscript-codec` verifier call the same measurement functions. Merely using
the same nominal codec version or parameters is insufficient because patched/system
implementations can rank candidate strings differently.

Gzip/Brotli allowance: transfer ≤ baseline **or** raw within growth percent. Raw model: raw within growth only.

## What search is not

- Not a substitute for types.
- Not enabled in CLI `--mode development`; the CLI overrides every configured search
  value, including `always`, to `off`.
- Not run per chunk in `split` / `preserve-modules` (those score **plans**, not emission alphabets), except joint chunk/symbol search when that feature is on.
- Not allowed to enable compression tactics absent from the allowlist.

The proposal space, IR finalist beam, objective-stratified emission frontier,
identifier trials, and byte/count budgets are all bounded. Except for deliberately
small exhaustive test oracles, the result is the best artifact found under those
budgets, not a mathematical global optimum.

Here “not enabled” means the multi-IR optimizer variants and emission-beam families
do not expand. The configured pipeline/emission still reaches finalization. An
independently configured parsed-peephole can still compete with the untouched form,
and configured profile/startup/performance features still run. `candidate_search`
does not currently act as a master switch for every finalization feature.
