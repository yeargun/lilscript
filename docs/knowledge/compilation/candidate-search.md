# Candidate search

Parent: [Compilation](README.md). Philosophy: [objectives](objectives.md),
[global optima](global-optima.md). Knobs: [cost model](../config/cost-model.md), [JS optimizations](../config/javascript-optimizations.md). Code: `optimize_and_select_javascript`, `select_javascript_candidate`, `finalize_javascript_candidates`, `extend_javascript_candidate_beam` in `src/compiler.rs`.

## Two levels

### Level 1 — IR optimizer variants

Always first: `config.js_optimizer_options()`. Then opportunistic clones from
`SCORED_IR_VARIANTS` (each must still compile):

| Variant | Gate |
|---|---|
| `inline_closure_factories = false` | `ir-closure-factory-variants` + compression decision |
| inlining fully off | `ir-inlining-variants` |
| scalar-replacement off (`keep-object`) | `js_keep_object_variants_enabled` (`joint-representation-search`) |
| no constant-parameter specialization | `ir-specialization-variants` |
| no call-site specialization | `call-site-specialization` |
| no capture-signature cloning | `capture-signature-cloning` |
| reusable helpers (inline off + specialization off) | helper when both families exist |
| function subsumption on and off | `js_function_subsumption_variants_enabled` |
| phase-order: no early CSE, aggressive inline, both | `ir-phase-ordering-variants`; **broad modules** get one combined probe |
| compress passes all-off; outlining contrast; fusion off; merging off | `ir-compress-pass-variants` |
| strongest bounded aggressive-inline phase probe + outlining | phase-order variants + outlining are both legal |

Each variant: clone IR → optimize → emit with **configured** `js_options()` → score
transfer → retain byte-equal seeds from distinct IR contexts because performance
provenance and later emissions may differ → keep top `candidate_beam_width`
finalists. Identical option plans within one context are deduplicated. The
configured optimizer is fixed first. With at least two optimizer slots,
the outlining contrast is reserved second; with at least three, the strongest bounded
aggressive-inline + outlining interaction is reserved third. This lets aggressive
inlining expose a repeated composite without crossing outlining with every optimizer
tuple.

A closed script whose policy enables `pure-helper-inlining` may give an IR probe one
additional emission before structural pruning. An IR whose
`repeated-region-outlining` report changed is pre-scored with `AllEligible`; an IR
with ordinary inlining disabled is pre-scored with `SingleStaticUse`. The probe rank
uses the better configured/interaction emission, while raw-growth admission still
uses the configured optimizer emission as its baseline. Distinct IR contexts
remain live even when configured bytes match because their performance
provenance and later emissions may differ; identical option plans within a
context are deduplicated. Emission failure simply omits this optional probe.

If that interaction makes the IR a finalist, its already rendered code and exact
options seed the terminal selector and consume a slot in that finalist's ordinary
candidate budget. This matters when a small budget cannot reach the late helper
family independently. For outlined IR, provenance keeps a multi-use outlined helper
as the shared composite while `AllEligible` substitutes its eligible leaf helpers;
an outline reduced to one call may still be substituted normally.

Retained IR contexts seed one aggregate cross-context emission arena with global
count and byte caps; the budget is not partitioned independently per finalist.
At least the configured output of each retained IR variant is measured.

### Level 2 — global emission arena seeded by IR finalists

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

The level is an effort tier as well as a feature gate. Configured count, byte,
beam, proposal, and terminal values are ceilings; the effective value is their
minimum with this table. Proposal and terminal columns show the omitted-value
defaults for artifacts through 16 KiB:

| `optimization_level` | candidates | retained bytes | beam | structural proposals | terminal work |
|---|---:|---:|---:|---:|---:|
| 0–2 | 1 | 64 KiB | 1 | 0 | 0 |
| 3–4 | 16 | 128 KiB | 2 | 16 | 0 |
| 5–6 | 64 | 192 KiB | 3 | 64 | 0 |
| 7 | 192 | 256 KiB | 4 | 192 | 0 |
| 8 | 192 | 256 KiB | 4 | 192 | 24 |
| 9–10 | 384 | 384 KiB | 6 | 384 | 64 |
| 11–12 | 768 | 512 KiB | 8 | 768 | 128 |
| 13 | 1024 | 768 KiB | 10 | 1024 | 192 |
| 14 | 1024 | 896 KiB | 11 | 1024 | 256 |
| 15 | unlimited | unlimited | unlimited | candidate/search cap | 384 |

| `candidate_search` | Search cap |
|---|---|
| `off` | 1 configured emission before finalization |
| `production` (default) | 384 |
| `always` | unlimited candidate cap (still min’d with `candidate_limit`); terminal level limits are 4× production, reaching 1536 at level 15 |

Exact `javascript.optimizations = [...]` **replaces** level-derived features,
but it does not bypass the level effort tiers. The allowlist chooses which
behaviors may run; `optimization_level` still bounds their work. Production
search still caps candidates at 384 unless `always`.

`candidate_byte_budget` (default 1 MiB) is an aggregate retained-artifact
ledger, with the configured incumbent as a mandatory floor. A small terminal
slice is reserved before structural retention so an incumbent naming/declaration
challenger is not starved by a byte-full structural arena. Rejected structural
proposals can still consume work before retention. `candidate_limit` controls
the retained frontier, not attempted work. `candidate_proposal_limit` closes
that gap: projection, entropy preparation/mapping, and each new
structural plan consume the shared ledger before whole-artifact work. Failed,
invalid, and rejected proposals still count. Already-scored optimizer context
seeds are reported separately. A terminal plan tail remains reserved after the
structural ledger fills. When omitted, the proposal limit also honors
`candidate_limit`; an explicit value can exceed the survivor count and bypass
artifact scaling, but not the level/search tier.

`terminal_codec_probe_limit` is a shared terminal-search work budget for optional
parsed-peephole preparation, repair/validation, cleanup, terminal naming, and
binding remaps. The current post-selection canonical peephole can perform one
additional comparison outside this ledger. Families receive deterministic
prefixes before expensive validation or parallel scoring, so invalid proposals
and Rayon scheduling cannot multiply work beyond the ledger. Actual codec calls
are counted separately and cannot exceed consumed work units. Default limits in
the table scale to one quarter for 16–64 KiB artifacts and one twelfth above
64 KiB. Explicit values bypass artifact scaling but cannot raise the level or
search tier.
When exhausted, search keeps the best
already-scored incumbent. The configured incumbent's mandatory score is outside
this optional budget; `candidate_search = "off"` always forces zero.

`--explain` reports `search_guarantee = best-observed`, a stop reason
(`search-disabled`, `work-budget-exhausted`, or `portfolio-exhausted`), and the
decision-registry version. These labels deliberately do not claim global
optimality.

Split/preserve bundle manifests separately record the configured JavaScript
codec total and the mixed deployment-cost weights, request/depth penalties, and
cache/preload discounts under a deterministic objective fingerprint. A mixed
deployment score is never labeled as raw, gzip, or Brotli bytes.

Within the emission beam, bounded retention is objective-stratified: rankings for
the selected model, raw, gzip, and Brotli are visited round-robin with the selected
model first, and duplicates are skipped. The same stratification feeds structural
finalists, entropy sources, and one-character identifier mappings. This preserves
promising cross-objective shapes that a later interaction may make best for the
selected model; it does not change the terminal winner, which is still ranked only
by the configured objective and priority. The IR-optimizer probe frontier remains a
separate selected-objective beam.

## Finalize

1. Optionally add a parsed-peephole clone (`parsed-peephole`, level ≥ 9) when
   the shared terminal codec budget admits its exact score.
2. Drop candidates that fail startup guard or raw-growth / transfer allowance.
3. Sort by `javascript_candidate_rank`, raw length, top-level declaration
   preference, startup, lexical output, then stable plan identity. Terminal
   topology-preserving search may first prefer more resolved one-byte bindings.
   For `size-first`, the primary rank is exact transfer bytes and performance
   breaks an exact transfer tie. Other priorities use normalized ratios for
   their mixed objectives.

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
- Not run per chunk in `split` / `preserve-modules`. Split scores bundle plans,
  not the single-file identifier-alphabet beam; `joint-chunk-symbol-search`
  adds only `function_layout` and `local_name_reserve` variants. Preserve-modules
  uses fixed source partitions and configured emission.
- Not a closed allowlist in practice: omitted non-search-only names usually stay
  off, but `elide_length_tonumber` is flipped unconditionally.

The proposal space, IR finalist beam, objective-stratified emission frontier,
identifier trials, and byte/count budgets are all bounded. Except for deliberately
small exhaustive test oracles, the result is the best artifact found under those
budgets, not a mathematical global optimum. Sequential flips are coordinate
descent on a non-monotone *T*: a pair of 0-delta flips can jointly win
([objectives](objectives.md)). Declared joint families (pure-helper × dense
tables is the model) are the designed fix, not a 2⁷⁴ Cartesian.

Here “not enabled” means the multi-IR optimizer variants and emission-beam families
do not expand. The configured pipeline/emission still reaches finalization.
`candidate_search = off` zeros the optional terminal codec budget, so scored
peephole **leaves** do not run. If `parsed-peephole` is still configured
(level ≥ 9), `apply_search_off_declaration_peephole` exactly measures one parsed,
function-preserving challenger against the untouched emit. Configured
profile/startup/performance features still run. `candidate_search` is not a
master switch for every finalization feature.

Emission search now iterates cartesian axes and scored families from
`src/decision_registry.rs`. Entropy mapping and several terminal neighborhoods
remain specialized coordinator stages; late families can still lose when the
work ledger exhausts. See [current architecture](current-architecture.md),
[goal architecture](goal-architecture.md), and
[decision registry](decision-registry.md).
