# Configuration behavior matrix

Parent: [config](README.md). Tests to own:
[verification config matrix](../verification/config-matrix.md).

| Layer | Changes semantics/type safety? | Changes JS ABI? | Changes selected bytes? | Changes native? |
|---|---|---|---|---|
| `[optimization]` | no; optional legal rewrites only | no; public boundary remains protected | yes | yes |
| `javascript.priority` | no | no public change by default; may mangle owned internals, never exports | yes; rank + inline/default tactics | no |
| `javascript.compression` | no | may permit property/export/function representations | yes; exact legal set | no |
| `optimization_level` / `optimizations` | no | only through searched legal alternatives | yes; search dimensions/budget | no |
| `cost_model` / candidate budgets | no | winner may have a different legal shape | yes | no |
| `[mangle]` | no language semantics, but public compatibility can change | **yes** | yes | no |
| JS shape keys | no language semantics, but public compatibility can change | **yes** | yes | no |
| startup/performance | no | winner may differ within allowed shapes | yes | no |
| `[profile]` | no | private specialization only; public contracts remain | yes/runtime shape | yes when native PGO gate is on |
| `[bundle]` | module/lazy delivery contract | artifact/export/chunk boundary | deploy bytes | no |
| `[native]` | no | no | no JS effect | storage placement |
| `[lint]` / `[format]` | diagnostics/source only | no | no compiler codegen effect | no |

Precedence is not symmetric: explicit `[optimization].x=false` is a hard off;
`[mangle]` explicit booleans override compression/priority for their four fields;
exact compression and optimization lists replace defaults rather than extending them;
CLI development mode forces multi-IR/emission candidate expansion off (not every
configured finalization feature); delegate bundling forces single LilScript output.

The matrix describes scope, not monotonicity. More optimization level, a wider beam,
more inlining, property mangling, string pooling, or an extra chunk can all lose a
different codec/runtime boundary. Preserve configured baselines and measure.
