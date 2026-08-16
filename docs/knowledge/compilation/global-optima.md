# Global optima

Parent: [Compilation](README.md). Implementation: `src/compiler.rs` (`optimize_and_select_javascript`, `select_javascript_candidate`, `javascript_candidate_rank`). Config: [cost model](../config/cost-model.md), [priority](../config/javascript-priority.md).

## The problem

A transform can be correct and locally smaller and still **increase** the bytes that get served.

Examples encoded as current regressions or retained historical negative evidence in
this codebase (historical byte/count claims require a canonical rerun):

| Local choice | Why it can lose globally |
|---|---|
| Delete a semicolon / `new` parens (raw win) | Gzip/Brotli like repeated punctuation; candidate search keeps the punctuated variant |
| Outline a repeated region into a helper | Often wins raw, loses gzip/Brotli — the canonical pass defaults **off**, while allowed candidate search may probe it |
| Inline more (`inline-96` on jQuery) | Duplicated bodies hurt Terser/Brotli more than they help; audit: lean-balanced beat aggressive inline |
| Convert `bindMethod`+arrow to `this`-methods | Looked locally native; **regressed** jQuery terser size |
| Apply comma-conditionals on a 23 KiB surface | Displaced a stronger beam candidate; pass capped ~16 KiB |
| Run Oxc/Terser on already-scored LilScript JS | Shortened raw, **worsened Brotli** on several app rows |
| Early CSE | Materializes temporaries that later inlining would have duplicated into a more repetitive spelling |
| Phi expression recovery under Brotli | Can disrupt large-context repetitions; default **off** when `cost_model = brotli`, search may still try the opposite |

DEFLATE’s backward window is 32 KiB. Brotli combines LZ, context modeling, Huffman, and a static dictionary (quality 11, `lgwin` 22 here). Declaration order, identifier alphabet, and token distance matter even when raw length is equal. An entropy **proxy** is never accepted in place of final codec measurement.

## The rule

1. Prove semantic eligibility (types, effects, alias, identity, escape).
2. Emit explicit IR or JS-IR alternatives — do not rewrite generated strings.
3. Deduplicate identical emissions and retain structurally different **families** in a bounded beam (`candidate_beam_width`). Beam truncation is a search budget, not a proof of global dominance.
   Intermediate emission retention is objective-stratified across selected, raw,
   gzip, and Brotli rankings so a cross-metric shape can survive to a later
   interaction; the selected objective is visited first.
4. Score the **complete** artifact with `cost_model`; cross-objective retention never
   turns into cross-objective final ranking.
5. Apply configured startup gates and rank with the selected size/performance policy.
6. Keep the configured baseline in the candidate set.

`candidate_beam_width` exists specifically to recover interactions whose **first** step is not locally best.

## Candidate admission and raw growth

`optimizer_variant_candidate_allowed` applies different admission rules:

- `raw`: candidate raw bytes must be within the configured growth percentage of the
  configured baseline;
- `gzip` / `brotli`: the candidate is admitted when its transfer bytes are no larger
  than the baseline **or** its raw bytes are within the configured growth percentage.

Consequently the repo default `max_candidate_raw_growth_percent = 0` does **not**
forbid all raw growth: a codec non-regression is admitted regardless of raw growth.
Raising the percentage widens the pool of candidates that may trade transfer size
against the performance rank. The configured baseline remains present.

## Ranking is priority-dependent

`javascript_candidate_rank`:

| Priority | Primary key |
|---|---|
| `size-first` (default) | exact transfer bytes; performance only breaks an exact transfer tie |
| `balanced` | `3 * normalized transfer + 2 * normalized performance` |
| `realistic-performance-first` | add an over-limit bucket penalty to normalized transfer; performance ratio second |
| `performance-first` | performance-shape ratio first, transfer second |

`realistic-performance-first` is not a hard filter: its first key is
`over_limit * 1_000_000 + transfer_ratio`. For the mixed priorities, the rank tuple
also contains the secondary performance or transfer ratio shown by
`javascript_candidate_rank`. After the priority rank, selection sorts by startup
score, raw bytes, then lexical output. Startup limits can reject candidates before ranking when
`startup-cost-guard` is configured; `max_nesting` is always checked when set. See
[priority](../config/javascript-priority.md).

## Compile-time is part of the optimum

Unbounded Brotli-11 on every cross-product would be correct and unusable. Budgets:

- `optimization_level` 0–15 (feature gates + level cap)
- `candidate_search`: `off` / `production` (cap 384) / `always`
- `candidate_limit`, `candidate_byte_budget` (default 1 MiB aggregate)
- `candidate_beam_width` (default 12)
- Broad modules (>24 functions or >2048 IR ops) collapse phase-order probes to **one** combined variant

The configured pipeline always runs. Extra variants are opportunistic.
