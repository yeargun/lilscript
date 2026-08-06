# Compiler Configuration

The CLI discovers `lilscript.toml` by walking from the input module toward the
filesystem root. Pass `--config path/to/config.toml` to select one explicitly.
Unknown keys and invalid numeric limits are errors.

```toml
[optimization]
preset = "maximum" # maximum | none
constant_folding = true
algebraic_simplification = true
common_subexpression_elimination = true
global_optimization = true
inlining = true
scalar_replacement = true
dead_store_elimination = true
dead_code_elimination = true

[javascript]
priority = "realistic-performance-first"
compression = [
  "identifier-mangling",
  "string-pooling",
  "size-aware-inlining",
]
# inline_instruction_limit = 18
# inline_control_flow_limit = 45
# max_inline_growth = 16

[mangle]
# identifiers = true
# properties = false
# exports = false
# pool_strings = true # optional explicit override

[bundle]
mode = "single" # single | split | preserve-modules
min_chunk_bytes = 16384
max_chunks = 32
shared_min_imports = 2
```

Every optional optimization key overrides its preset independently. The
`none` preset disables optional transforms but retains mandatory IR
normalization and correctness analyses. This makes it useful for debugging and
for isolating pass regressions without changing language semantics.

`javascript.priority` is a JavaScript-target policy. It never weakens semantic
checks, mandatory IR normalization, DCE correctness, or host-boundary rules:

- `performance-first` uses straight-line/control-flow limits of `24`/`60`, has
  no inline-growth cap, and disables automatic string pooling.
- `realistic-performance-first` is the default. It uses limits of `18`/`45`,
  allows up to `16` estimated additional IR instructions from repeated-call
  inlining, and enables profitable string pooling.
- `balanced` uses limits of `12`/`30`, permits up to `4` estimated additional
  instructions, and enables profitable string pooling.
- `size-first` uses limits of `12`/`30`, permits no positive estimated inline
  growth, and enables profitable string pooling.

`javascript.compression` is an optional exact allowlist of contested JavaScript
size tactics. If omitted, the selected profile supplies the list. If present,
only listed tactics are enabled; `compression = []` disables all of them:

- `identifier-mangling` assigns short names by whole-program use frequency.
- `property-mangling` renames LilScript-owned boundary-visible properties.
- `export-mangling` permits public ESM export names to be shortened.
- `string-pooling` aliases repeated strings only when the emitter's raw-size
  model predicts a reduction.
- `size-aware-inlining` applies the profile's positive-growth limit to repeated
  straight-line calls.

The numeric `inline_instruction_limit`, `inline_control_flow_limit`, and
`max_inline_growth` keys override the selected profile. Setting
`max_inline_growth` explicitly also enables the growth guard, even when
`size-aware-inlining` is absent from the allowlist. These are IR instruction
budgets, not output-byte limits.

The priority is applied after `[optimization]`: setting `inlining = false`
disables inlining in every profile. Explicit `[mangle]` values have the highest
precedence, followed by the exact compression allowlist, then profile defaults.
The aliases `realisticperf-first` and `realistic-perf-first` are accepted for
`realistic-performance-first`. Raw, gzip, and Brotli sizes can disagree, so
size policy is a compiler cost-model preference rather than a universal
guarantee for every compressor and workload. Measure release artifacts with
the intended transport compression.

The policy affects only JavaScript. A configured `--target all` build shares
parsing and semantic analysis, then optimizes separate JavaScript and native IR
copies. Changing `javascript.priority` therefore does not change generated C or
the native executable's optimizer policy.

`mangle.properties` renames LilScript-owned fields that cross an untyped
JavaScript boundary. It is off by default because external JavaScript must
otherwise use the renamed ABI. Members declared by `extern class` are host ABI
names and are never renamed, regardless of this setting. Internal struct and
class fields already lower to scalar values or numeric slots. `mangle.exports`
removes stable public ESM names and is intended for LilScript-only applications
whose static imports are linked before codegen.

The bundle policy is separate from optimizer policy. Every mode first links and
optimizes the complete static module graph, so cross-file inlining, scalar
replacement, and DCE happen before a chunk boundary is selected.

- `single` emits one whole-program artifact and has no chunk overhead.
- `preserve-modules` emits surviving, movable dependency functions in one
  static ESM chunk per source module. Root functions and functions that assign
  module globals remain in the entry chunk. Size/import/count limits do not
  override source-module preservation.
- `split` considers modules imported by at least `shared_min_imports` distinct
  modules, measures their emitted chunk bytes, rejects chunks smaller than
  `min_chunk_bytes`, and retains at most `max_chunks` candidates (largest
  first, with deterministic module-order tie breaking).

`split` and `preserve-modules` require `--output`. They write the entry module,
sibling chunks, and `<entry-stem>.manifest.json`. Chunk imports are static ESM
imports and therefore load eagerly; LilScript does not yet have a dynamic
`import()` expression or lazy runtime chunk loader. Use an `.mjs` output when
running directly in Node without a `"type": "module"` package boundary.
