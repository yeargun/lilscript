# JavaScript emission

Parent: [Compilation](README.md). Source: `src/codegen_ir_js.rs` (`IrJsOptions`, `IrJsEmitter`). Related: [peephole](peephole.md), [`[mangle]`](../config/mangle.md), [compression decisions](../config/compression-decisions.md).

## Production path

Optimized SSA → `emit_optimized_ir_js_with_options_and_analysis`. Options come from `ProjectConfig::js_options()` (canonical state) plus **search mutations** in `select_javascript_candidate`.

`src/codegen_js.rs` is the legacy AST emitter (`CodegenOptions { mangle, dissolve_structs }`). Do not add new size tactics there.

## Canonical `IrJsOptions` highlights

Set in `js_options()` from config. Notable defaults / cost-model interactions:

| Field | Typical size-first | Notes |
|---|---|---|
| `mangle_identifiers` | on | `[mangle].identifiers` overrides |
| `mangle_properties` | on (size-first) | owned fields only |
| `mangle_exports` | off | reusable ABI |
| `pool_strings` | on except performance-first | `[mangle].pool_strings` overrides |
| `pool_numeric_literals` | `javascript.pool_numeric_literals` (default true) | |
| `elide_safe_integer_coercions` | on (size-first/balanced) | not searched; `|0` never helps transfer. Off for performance-first. `integer_coercions = true` keeps `|0` |
| `compact_boolean_literals` | on except performance-first | `!0`/`!1` vs `true`/`false` |
| `standard-grammar-elision` trio | on | ASI `;`, `new` parens, call-chain parens — still searched |
| `pack_string_arrays` | size-first only | `.split` tables vs arrays |
| `regex_literals` | off for open-world output | narrow valid subset; requires the explicit pristine-builtins contract |
| `inline_single_use_functions` | off in configured baseline | proof-gated script-only candidate when search + structured-closure compression are on |
| `pure_helper_inlining` | `None` in configured baseline | search-only `SingleStaticUse` / `AllEligible` policies under `pure-helper-inlining` |
| `dense_string_return_tables` | off in configured baseline | proof-gated complete lookup under `dense-string-return-tables` |
| `host_alias_spelling` | `Shared` in configured baseline | search-only `Direct` for proven direct-only static callees under `host-alias-spelling` |
| `ecmascript` | `es2022` | syntax floor; never raised by search; host aliases and `?.`/`??`/`Object.hasOwn`/`catch{}` lower or error |
| `indexed_char_at` | `false` in configured baseline | search-only `s[i]` under `indexed-char-at` when the index is proven in range |
| `effect_ternary` | `true` in configured baseline | existing discarded-if recovery; search may disable it only when `effect-ternary` is listed |
| `scalar_phi_copies` | size-first | vs tuple destructuring |
| `phi_affinity_mode` | Grouped if coalescing on | Conservative otherwise |
| `local_phi_expression_regions` | **off when `cost_model = brotli`** | search may enable |
| `phi_edge_value_forwarding` | **off when brotli** | same |
| `comma_expressions` | false in canonical | search feature |
| `function_spelling` | Arrow internally if unset; Function when `cost_model = brotli` | public arrows only if explicit `"arrow"`; captured functions and flattened methods may still be arrows because extra parameters are ordinary formals, not JS `this` |
| `function_layout` | Source | search: similarity / compression window |
| `identifier_alphabet` | canonical | entropy-aware search |
| `string_quote` | Double | quote-style search |
| `local_name_reserve` | config (repo 48, struct default 16) | production search also tries 0/8/16/32 |
| `named_aggregate_fields` | from `aggregate_layout` | |
| `public_aggregate_fields` | from `public_aggregate_abi` | |

## Name hygiene and mangling

Before assigning source, top-level, or local names, the emitter reserves every
runtime root it may synthesize: for example `Math`, `Array`, `Object`, `Promise`,
typed-array constructors, scalar constants, generated error constructors, and the
browser globals `document`, `window`, and `globalThis`. Every local allocator
inherits that inventory. Consequently a source binding cannot capture generated
`Array.isArray`, `Math.imul`, or `document.createElement` even when identifier
mangling is disabled.

`windowDocument()` emits the `document` global so it shares spelling with DOM
method calls. `windowSelf()` still uses `typeof window<"u"?window:globalThis`,
pooled when that window root is repeated.

Foreign-import external names remain ABI. Their local aliases receive hygienic
emitted spellings through the same top-level allocator, and the matching extern
function/global uses that mapping. `import extern { host as Array }` therefore keeps
the imported name `host` while preventing the local binding from shadowing the
runtime `Array` root.

Frequency-ranked base-54/base-64 names. Extern and referenced globals are reserved.
`entropy-aware-mangling` compares canonical alphabet vs emitted-character ranking
plus a bounded permutation search over one-character names; every trial is re-emitted
through the scope-aware mangler. Budget scales down with artifact size so quality-11
probes stay practical.

`local_name_reserve` holds the first N short spellings for lexical locals so similar functions share a local alphabet (better cross-function gzip/Brotli). `stable_local_names` assigns colors by source-local affinity without changing liveness.

`function_layout`: unchanged source order stays in the beam. Similarity order uses Held-Karp up to `function_layout_exact_limit` (default 13, max 18) then insertion. Window order discounts matches beyond gzip 32 KiB or Brotli 4 MiB history.

## Spelling families the beam may try

Pooling (string/number), packing, boolean literals, grammar elision (independently — comment: raw punctuation deletion can lose codec), structured closures, proof-gated single-use function expressions, pure-helper substitution, dense string-return tables, host-alias spelling, regex literals, unused catch binding, generator star spacing, callee default arguments, SSA destruction (scalar vs tuple phi, affinity modes), control flow (structured vs state machine), loops (`while` vs `for(;cond;)` vs `do`), mutation (`=`, prefix, postfix, compound), conditionals/commas, `var` vs `let` top-level, function arrow vs `function`, quote style, identifier alphabet, local-name reserve, declaration order.

Phi-affinity exploration retains Grouped, Direct, and Conservative modes. Liveness
interference includes local-phi expression incoming dependencies for the result's
live range, so a reused name cannot overwrite an incoming value still needed by a
nested/parallel phi. The three modes are compression alternatives, not weaker
correctness levels.

Search **disables** enabled canonical tactics to compare. Size-first search-only
spellings such as `indexed-char-at` still compete from the priority matrix when
an explicit `compression` list is a non-empty overlay; `compression = []` is the
off switch. Listing a name that the profile would have left off still opts that
decision in.

The single-use function-expression proposal is the narrow exception to “canonical
option then disable”: its canonical/configured value is deliberately `false`, while
candidate search may introduce `true` when `structured-closure-inlining` is enabled.
The emitter first performs the whole-program eligibility proof described under
[inlining and sharing](inlining-specialization-sharing.md#emission-only-single-use-function-expressions).

Pure-helper substitution and dense string-return tables are another search-only
family, but not a semantic mode. The ordinary named helper/guard ladder is the
configured baseline and remains mandatory. Their proof boundaries are documented in
[inlining and sharing](inlining-specialization-sharing.md#pure-helper-expression-substitution)
and [mangling, layout, and pooling](mangling-layout-pooling.md#dense-string-return-tables).

## Static host-alias spelling

Known JavaScript host aliases normally emit one shared top-level binding. With
candidate search and `host-alias-spelling`, the emitter may instead place the native
dotted spelling at each call site, and raw/gzip/Brotli complete-artifact cost chooses
between those forms. `Direct` suppresses a binding only for the static `Callee`
convention after whole-module use analysis proves that no function value is
observable. Detached/address-taken aliases, eager or lazy exports, method/bound
conventions, and constructor uses retain their binding and calling convention.

## Public ABI traps

`function_spelling = "arrow"` on **exported** functions removes `prototype` and `new`. Benchmark verifiers check arity and constructibility. Use only when the published API is nonconstructible.

`public_aggregate_abi = "positional"` requires opaque handles. JS must not inspect fields.

Default arguments on exported functions keep full-arity JS signatures (`Function.length`, omitted-call behavior).
