# JavaScript shape and ABI controls

Parent: [config](README.md). Language consequences:
[aggregates](../language/aggregates.md) and
[packages/exports](../language/packages-exports-abi.md). Source anchor:
`JavaScriptConfig` and `ProjectConfig::js_options` in `src/config.rs`.

These keys can change observable JavaScript shape; do not treat them as invisible
minifier flags.

The planned migration normalizes them into an immutable compilation contract before
optimization. `cost_model`, `priority`, and candidate-search effort consume that
contract but cannot change it. Every raw/gzip/Brotli artifact for one contract
must pass the same expected-versus-observed ABI checks. The current source-derived
manifest is only the first slice.

| Key | Default | Observable effect |
|---|---|---|
| `public_aggregate_abi` | `named` | `positional` exports opaque array handles instead of named public fields |
| `aggregate_layout` | `positional` | internal/escaping instance backing; named objects trade transfer for runtime storage shape |
| `function_spelling` | unset | legacy combined public/private callable policy; planned normalization freezes public callable kind in ABI and searches only private spelling |
| `strip_console` | `true` | removes `print`/`debugLog` but retains their argument effects; tests set false |
| `pool_numeric_literals` | `true` | allows repeated-number aliases when candidate search finds a win |
| `local_name_reserve` | 16 struct default | reserves short module spellings for reuse inside lexical functions |
| `stable_local_names` | `true` | uses source-local affinity to stabilize same-scope colors |
| `local_name_coalescing` | `true` | in identifier-mangled output, reuses a local binding for proven noninterfering SSA values; `false` retains distinct declarations |
| `function_layout_exact_limit` | 13 | exact declaration-order search cutoff, max 18 |

`[mangle]` then controls identifiers, owned properties, exports/public fields, and
string pooling with highest precedence. `exports=false` protects reusable names but
does not force `properties=false`; internal owned fields can still mangle.

## Boundary recipes

- Reusable ESM/script-tag library: named public ABI, `mangle.exports=false`, leave
  public function spelling constructible unless the API explicitly forbids `new`.
- Closed LilScript app: `mangle.exports=true` and positional public handles are
  available only when all consumers are linked and field opacity is part of the
  contract.
- Host objects/records: `Record<T>` keys stay exact. `extern class` members stay
  exact when `mangle.extern_fields` is unset/true; explicit `false` is a legacy
  coordinated closed-key mode, not a browser-host setting. Constructor-value
  export and proof-marked named class emission are implemented
  ([compressor surface](../language/compressor-surface.md),
  [class identity](../compilation/class-identity.md)).

In the planned model, compilation world, artifact format, and public roots are
separate. Both application and library recipes still optimize a closed internal `.lil` graph.
“Reusable” means the root manifest has unknown JavaScript consumers; it does not mean private
identifiers, owned properties, closure captures, or internal layouts stay
unoptimized. Conversely, a codec objective must never silently switch a public
ordinary function to a non-constructible arrow or change a public aggregate
shape. Such choices are ABI, not profitability. The target model therefore
splits public callable kind/constructibility from private `function` versus arrow
spelling; the latter remains a legal search family.

Two internal alternatives are deliberately absent from this ABI table. Closed
record observations may be projected away in a separately scored IR candidate, but
any surviving `Record<T>` keeps null-prototype backing; a private one-call script
function may be emitted anonymously only after proving its identity is not reusable.
Both retain the configured baseline and are selected by complete-artifact scoring.
They are not TOML shape promises. See
[aggregate lowering](../compilation/aggregate-lowering.md#closed-record-observation-projection)
and [inlining](../compilation/inlining-specialization-sharing.md#emission-only-single-use-function-expressions).

Every ABI-changing variant needs descriptor/name/arity/constructibility tests, not
only stdout and size.
