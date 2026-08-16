# Cost model and search budgets

Parent: [Config](README.md). Why: [global optima](../compilation/global-optima.md). Mechanics: [candidate search](../compilation/candidate-search.md).

## `javascript.cost_model`

| Value | Objective |
|---|---|
| `raw` | emitted bytes |
| `gzip` | statically bundled upstream stock zlib C 1.3.1, level 9, deterministic `mtime = 0` |
| `brotli` (default) | statically bundled official Google Brotli C 1.1.0, generic mode, quality 11, window 22 |

This is what “smaller” means for IR-variant and emission selection. Chunk **deploy** cost uses its own `[bundle.cost]` weights and always measures gzip-9 and Brotli-11 of chunk text.

Brotli canonical emission **disables** `local_phi_expression_regions` and `phi_edge_value_forwarding` because they tend to break large-context matches; search can still score the opposite when those optimization features are on. Gzip/raw keep them on at sufficient level.

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

`off` is narrower than “disable every codec-aware action.” The configured optimizer
and emission still run, and finalization still analyzes/scores that artifact. If
`parsed-peephole` is enabled by level or an exact optimization list, the untouched and
parsed-peephole forms can still compete under the cost model. Configured startup/
performance analysis and profile-guided optimizer passes also remain active. The name
is therefore shorthand for turning off the multi-IR/emission candidate expansion; a
future config cleanup may give finalization variants a separate switch.

## Budgets

| Key | Default | Role |
|---|---|---|
| `candidate_limit` | 1536 | Hard count before other caps |
| `candidate_byte_budget` | 1 MiB | Approximate aggregate retained-artifact bytes; converted to a per-variant count from baseline size; does not count every rejected/alternate-objective probe |
| `candidate_beam_width` | 12 | How many leading layouts survive each structural decision |
| `max_candidate_raw_growth_percent` | 0 | Raw-side admission allowance vs configured baseline (max 1000) |

Raise `candidate_byte_budget` for slower maximum-compression releases. Tiny outputs
hit the count cap; huge outputs automatically retain fewer complete artifacts. It is
not presently a strict compile-time or total-codec-probe ceiling, so CI must also
record wall time and the actual evaluated/retained counts when available.

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
