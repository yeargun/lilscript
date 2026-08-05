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

[mangle]
identifiers = true
properties = false
exports = false
pool_strings = true

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
