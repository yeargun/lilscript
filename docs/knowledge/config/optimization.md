# `[optimization]`

Parent: [Config](README.md). Pass order: [IR optimizer](../compilation/ir-optimizer.md).

Semantic IR policy for **both** backends. Optional keys override `preset` independently.

## Presets

| Preset | Meaning |
|---|---|
| `maximum` (default) | Optional transforms on (except those whose code default is off: subsumption, outlining, and several compress/merge flags until JS/priority re-enables them) |
| `none` | Optional transforms off; mandatory normalization and correctness analyses remain |

`none` is for debugging pass regressions, not a different language.

## Keys

| Key | Role |
|---|---|
| `constant_folding` | Const prop / fold in scalar fixed point |
| `algebraic_simplification` | Algebraic identities |
| `common_subexpression_elimination` | GVN/CSE; phase-order search may disable it |
| `finite_value_propagation` | Bounded bool/string/null/owned-field lattice (≤4 alts) |
| `global_optimization` | Entry-global internalization, immutable global prop |
| `inlining` | Expression + CFG inlining; false disables all inline variants |
| `inline_closure_factories` | If false, factories that return closures stay outlined |
| `constant_parameter_specialization` | Clone callees for constant args |
| `specialize_tagged_constants` | Include boxed/union constants; JS `js_optimizer_options` defaults this **true** if unset |
| `scalar_replacement` | Dissolve `LocalOnly` structs/classes. `false` is a hard off; size-first library search may add a `keep-object` clone when `joint-representation-search` is admitted. |
| `dead_store_elimination` | Overwritten field stores |
| `dead_code_elimination` | Dead ops, mutation graphs, dead functions (tree shake) |
| `call_site_specialization` | PGO/byte-budget clones; also needs JS feature for JS |
| `capture_signature_cloning` | Clone closures by constant capture set |
| `identical_function_folding` | Late private identical CFG merge |
| `function_subsumption` | Proof-driven extra-parameter sharing; `false` hard off; unset → native off, size-first JS may search |
| `pipeline_fusion` | See [compress passes](../compilation/compress-passes.md) |
| `partial_escape_sinking` | |
| `region_outlining` | Default **false** even on maximum |
| `expression_superopt` | |
| `path_sensitive_propagation` | |
| `parameterized_function_merging` | Permuted/single-operand private merge |
| `profile_guided` | Load `[profile]` data; JS also needs `profile-guided-optimization` |

Inline **numeric** limits are **not** in this table. They come from `javascript.priority` / `javascript.inline_*` / `max_inline_growth` via `js_optimizer_options`. Native keeps `OptimizationOptions` defaults (12 / 30 / no growth cap) unless you only compile JS.

## JS AND-gates

Even if `[optimization]` enables a pass, JS may still disable it:

- call-site specialization, capture cloning, identical folding — require the matching `javascript.optimizations` / level
- subsumption — `js_function_subsumption_variants_enabled` (size-first or explicit)
- compress passes and parameterized merging — require the matching `javascript.compression` decision

## `specialize_tagged_constants` quirk

`OptimizationOptions::default()` has `specialize_tagged_constants: false`. `js_optimizer_options` sets it to `optimization.specialize_tagged_constants.unwrap_or(true)`. Native follows `resolve()` (false unless you set the TOML key or maximum’s resolve path — currently the struct default in `resolve` uses `base.specialize_tagged_constants` which is false). If a tagged-constant clone should exist on native, set the key explicitly.
