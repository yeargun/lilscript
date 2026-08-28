# Cost model and search budgets

Parent: [Config](README.md). Why: [objectives](../compilation/objectives.md),
[global optima](../compilation/global-optima.md). Mechanics: [candidate search](../compilation/candidate-search.md).

## `javascript.cost_model`

| Value | Objective |
|---|---|
| `raw` | emitted bytes |
| `gzip` | statically bundled upstream stock zlib C 1.3.1, level 9, deterministic `mtime = 0` |
| `brotli` (default) | statically bundled official Google Brotli C 1.1.0, generic mode, quality 11, window 22 |

This is what “smaller” means for IR-variant and emission selection. Chunk **deploy** cost uses its own `[bundle.cost]` weights and always measures gzip-9 and Brotli-11 of chunk text.

Brotli canonical emission **disables** `local_phi_expression_regions` and
`phi_edge_value_forwarding` because they tend to break large-context matches;
search can still score the opposite when those optimization features are on.
Gzip/raw keep them on at sufficient level.

The same `js_options()` function also forces `pack_string_arrays = false` and
`pool_identifier_strings = false` under Brotli, and selects `Function` spelling
when `function_spelling` is unset. Packing’s search Cartesian cannot turn
packing back on if the incumbent is already false. Identifier-string pooling
is never flipped. Full table:
[codec-conditioned incumbents](../compilation/decision-registry.md#codec-conditioned-incumbents).

Function-layout window clustering uses 32 KiB (raw/gzip) vs 2²² (brotli) history.
Encoder identity is normative: another implementation may rank equal-input
artifacts differently even at the same quality/window settings.
Compiler selection and hard-gate verification use the same statically linked
measurement functions; `lilscript-codec` is the batch interface and Node/system
codec sizes are diagnostic only.

## `candidate_search`

| Value | When it runs | Cap |
|---|---|---|
| `off` | no multi-IR/emission-beam expansion | 1 configured emission before finalization |
| `production` (default) | not in `--mode development` | 384 |
| `always` | all normal compilation modes | unlimited (min with `candidate_limit`) |

CLI `--mode development` overrides every configured value to `off` before
compilation, so both `production` and `always` are disabled there. Lilpack `dev` uses
that mode. `always` means “do not apply the production 384-candidate cap,” not “ignore
the CLI development override.”

`off` retains the configured optimizer, emission, mandatory codec score, and
validation, but forces the optional terminal codec budget to zero. It therefore
skips parsed-peephole leaves, cleanup neighborhoods, and live-letter remapping
instead of spending minutes in Brotli-11 after a one-candidate build. Configured
startup/performance analysis and profile-guided optimizer passes remain active.

## Budgets

| Key | Default | Role |
|---|---|---|
| `candidate_limit` | 1536 | Retained whole-artifact frontier count; omitted proposal work also honors it |
| `candidate_byte_budget` | 1 MiB | Aggregate retained whole-artifact bytes, with the configured incumbent as a mandatory floor |
| `candidate_beam_width` | 12 | How many leading layouts survive each structural decision |
| `candidate_proposal_limit` | level/artifact-derived (384 at level 15 production through 16 KiB) | Shared structural work ledger charged before projection, entropy mapping, and optional plan emission |
| `terminal_codec_probe_limit` | level/artifact-derived (384 at level 15 through 16 KiB) | Shared terminal-search work ceiling; the current post-selection canonical peephole can perform an additional codec comparison outside it |
| `max_candidate_raw_growth_percent` | 0 | Raw-side admission allowance vs configured baseline (max 1000) |

Raise `candidate_byte_budget` and the explicit work ceilings up to their
level/search tiers for slower
maximum-compression releases. Tiny outputs hit the count cap; huge outputs retain
fewer complete artifacts. Explain output reports registered plans, attempted
optimizer/structural emissions, structural proposal work, terminal work units,
actual terminal codec calls, both effective limits, and exhaustion. Defaults
scale to one quarter for 16–64 KiB artifacts and one twelfth above 64 KiB.
Explicit work ceilings bypass that artifact scaling but cannot raise the
level/search tier.
At level 15, `candidate_search = always` uses a 1536-unit terminal tier;
production remains 384.
The shared-ledger cap is hard for work charged to it, but the current
post-selection canonical peephole sits outside that ledger. It is not a complete
compilation-wide codec-call, wall-time, or RSS ceiling. The goal architecture
routes every challenger through one evaluator and budget.

Intermediate emission retention is objective-stratified across selected, raw, gzip,
and Brotli rankings, selected objective first. That bounded diversity protects
interactions without changing the final configured-objective rank. It is not an
exhaustive cross-product and does not prove a global minimum.

For `raw`, a candidate must remain within this percentage of baseline raw bytes. For
`gzip` and `brotli`, admission is `transfer <= baseline_transfer OR raw <= allowed_raw`.
At the default `0`, a transfer non-regression can therefore enter even with raw
growth. Raising the percentage admits additional raw-growing candidates that can be
selected by balanced or performance-oriented ranking. It does not change what bytes
the codec measures.

## Determinism

After the priority rank tuple, ties use startup score, raw length, then lexical JS.
Fixed compiler build, sources, config, and profile data produce a deterministic
winner. Selection does not depend on wall-clock timing or randomized hash order.
