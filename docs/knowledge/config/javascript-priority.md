# `javascript.priority`

Parent: [Config](README.md). Ranking: [global optima](../compilation/global-optima.md). Inline budgets feed [IR optimizer](../compilation/ir-optimizer.md).

Never weakens types, mandatory IR, DCE correctness, or host-boundary rules. Does not affect C/native.

Aliases: `realisticperf-first`, `realistic-perf-first` → `realistic-performance-first`.

## Policies

| Priority | Inline instr / CFG / growth | Rank key | Compression defaults |
|---|---|---|---|
| `size-first` (default) | 12 / 30 / 16 | transfer first | Broadest: packing, property mangling, IR variants, loop/mutation spelling, joint search, parameterized merge, scalar phi copies, … |
| `balanced` | 12 / 30 / 4 | `3*transfer + 2*shape` | Superopt + path-sensitive + loop spelling; **no** packing, property mangling, pipeline fusion, joint search, IR inlining variants |
| `realistic-performance-first` | 18 / 45 / 16 | over-limit bucket + transfer ratio, then shape ratio | Like balanced on many tactics; still no packing / size-first-only searches |
| `performance-first` | 24 / 60 / none | shape first | Identifier mangling + grammar elision + unused-catch + generator-star + phi affinity; **no** pooling, entropy alphabet, packing, IR search variants |

`enables_compression` in `src/config.rs` is the exact matrix. Size-first-only decisions: `string-array-packing`, `scalar-phi-copies`, `ir-*-variants`, `mutation-spelling-selection`, `property-mangling`, pipeline fusion, partial-escape sinking, region outlining, joint representation/chunk search, parameterized merging. `loop-spelling-selection` is enabled for size-first and balanced.

`expression-superoptimization` and `path-sensitive-propagation` are on for size-first **and** balanced.

`export-mangling` is **never** implied by priority. Opt in via compression list or `[mangle].exports`.

`|0` is not a compression tactic. Size-first and balanced drop proven-redundant `|0`.
`performance-first` and `realistic-performance-first` keep it. Set
`javascript.integer_coercions = true` to keep it on size-first or balanced.

## Numeric overrides

```toml
[javascript]
inline_instruction_limit = 7
inline_control_flow_limit = 9
max_inline_growth = 3
```

These override the profile. Setting `max_inline_growth` also **enables** the growth guard even if `size-aware-inlining` is missing from the compression allowlist.

These are IR instruction budgets, not output-byte caps. jQuery’s inline TOMLs exist because raising them is **not** monotonically smaller — see [jQuery](../evidence/jquery.md).

## ABI-ish JS keys (not priority, but sit beside it)

| Key | Meaning |
|---|---|
| `function_spelling` | omit = private arrows may be searched, public functions stay constructible; `"function"` force; `"arrow"` allows public arrows (drops `new`/`prototype`) |
| `strip_console` | default **on**. Root test oracle / `print` tests set **off**. Drops `print` / `debugLog`, keeps `console.warn`. |
| `public_aggregate_abi` | `named` (default) vs `positional` opaque handles |
| `aggregate_layout` | instance backing; default `positional` |
| `pool_numeric_literals` | default true |
| `integer_coercions` | omit = drop proven `|0` on size-first/balanced, keep on performance-first and realistic-performance-first; `true` keeps `|0` |
| `local_name_reserve` | 0–256; production search also tries 0/8/16/32 |
| `stable_local_names` | default true |
| `local_name_coalescing` | default true; in identifier-mangled output, reuse bindings only for proven noninterfering SSA live ranges; maximum SSA-destruction search may score both regimes |
| `function_layout_exact_limit` | Held-Karp cutoff 0–18, default 13 |

## Startup and performance subtables

`[javascript.startup]` — parse/compile/memory weights and overhead **reject** percents; optional `max_nesting`.

`[javascript.performance]` — deopt / allocation / indirect-call / hot-code weights; `max_regression_percent` for realistic-performance-first (default 25). At least one weight must be nonzero.

The realistic-performance threshold is a ranking bucket, not a rejection gate. An
over-limit candidate remains eligible, but normally ranks behind candidates in the
within-limit bucket before the performance ratio breaks an otherwise equal rank.
