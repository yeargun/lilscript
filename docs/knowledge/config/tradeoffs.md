# Tradeoff matrix

Parent: [Config](README.md). Mission: [tradeoff triangle](../mission.md).
Objectives: [size/performance × codec](../compilation/objectives.md).

Use this when picking knobs. “Win” means typical direction, not a guarantee — **measure** under the intended codec.

## Size vs compile time

| Move | Transfer | Compile time |
|---|---|---|
| `candidate_search = off` / level 0 | may miss global wins | much faster |
| `production` + level 15 | default release | bounded (384 / 1 MiB) |
| `always` + high `candidate_byte_budget` + wide beam | best chance at codec optimum | can dominate CI |
| `ir-phase-ordering-variants` / compress-pass variants | sometimes large | extra full pipelines |
| `function_layout_exact_limit` → 18 | maybe better order | exponential in that cutoff |
| `--mode development` | no multi-IR/emission beam; configured finalization may remain | edit loop |

## Size vs runtime

| Move | Transfer | Runtime / startup |
|---|---|---|
| `priority = size-first` | primary | shape is tie-break only; packing/outlining can cost parse |
| `balanced` | mixed | more weight on deopt/alloc/indirect calls |
| `realistic-performance-first` | bucketed transfer objective | over-limit candidates are penalized in ranking, not rejected |
| `performance-first` | secondary | keeps `|0`, no pooling/packing, no IR search variants |
| `string-array-packing` | often smaller | extra split work at startup |
| `integer_coercions = true` | worse transfer | keep generated `\|0` for engines; live source `value \| 0` is already preserved |
| `aggregate_layout = named` | often larger JS | sometimes cheaper V8 instances |
| `public_aggregate_abi = positional` | smaller if opaque | breaks named-field JS consumers |
| Startup overhead percents | reject “tiny but unparsable” | hard ceiling |

## Size vs language/ABI stability

| Move | Smaller? | Cost |
|---|---|---|
| `mangle.exports = true` | yes for apps | public names change |
| `property-mangling` | yes internally | public named fields stay unless exports too |
| `function_spelling = arrow` on exports | maybe | not constructible |
| `JsValue` / `setProp` bags | **no** — blocks field deletion | needed at messy host APIs; internals as bags means the port is still JS ([compressor surface](../language/compressor-surface.md)) |
| `struct` instead of `Record` / bags | **maybe** — authorizes layout search; Brotli can still prefer `s.field` (md-01) | must not be JS-observable keys |
| More inlining | **maybe not** (jQuery) | code duplication vs call overhead |

## Codec disagreement

Always set `cost_model` to what you **serve**. A gzip-selected spelling can lose
Brotli and vice versa. Historical closure-factory and mutation-spelling ablations
retain that disagreement lesson, but their byte rows require canonical refresh before
being quoted as current. Root config chooses `brotli`. Producing independently
searched raw, gzip, and Brotli artifacts is **three compiles**; none is claimed
globally minimal unless its declared finite domain was exhausted
([objectives](../compilation/objectives.md)).

`cost_model` also changes **canonical emission**, not only ranking:
`js_options()` turns off string-array packing and identifier-string pooling
under Brotli, prefers `function` spelling, and disables local-phi / phi-edge
recovery. Search can reverse some of those priors and cannot reverse others
([registry](../compilation/decision-registry.md#codec-conditioned-incumbents)).
There is no implemented “do not inline on raw, inline on Brotli” switch;
inline **budgets** follow `priority`, and no-inline IR clones follow
`ir-inlining-variants`.

`[bundle.cost]` can weight brotli higher than gzip for multi-file deploys independently of `javascript.cost_model`.

## What not to do

- Do not post-minify LilScript output with Oxc/Terser/Closure as a “free” extra win. It has increased Brotli on real rows.
- Do not raise inline limits because a function “looks hot” without measuring the complete artifact.
- Do not enable `region-outlining` as a default because helpers look smaller in the editor.
- Do not describe `max_candidate_raw_growth_percent = 0` as a blanket refusal to
  inflate. Under gzip/Brotli, a transfer non-regression is admitted even with raw
  growth; see [cost model](cost-model.md).
