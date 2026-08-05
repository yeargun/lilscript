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

`mangle.properties` renames fields that cross an untyped JavaScript boundary.
It is off by default because external JavaScript must otherwise use the renamed
ABI. Internal struct and class fields already lower to scalar values or numeric
slots. `mangle.exports` removes stable public ESM names and is intended for
LilScript-only applications whose static imports are linked before codegen.

The bundle policy is separate from optimizer policy. `single` performs the
current whole-program build and permits maximum cross-module inlining and DCE.
The config parser reserves validated `split` and `preserve-modules` policies,
including minimum shared-chunk size, chunk-count cap, and minimum importer
count. The CLI currently rejects those modes because emitting one file while
claiming split output would be incorrect; the chunk planner and manifest are a
separate implementation stage.
