# `[native]`

Parent: [Config](README.md). Language: [JS vs native](../language/js-vs-native.md). Code: `src/codegen_native.rs`, `ProjectConfig::native_options()`.

JavaScript `priority` / compression / candidate search **do not apply**.

| Key | Default | Meaning |
|---|---|---|
| `partial_escape_analysis` | true | Conservative PEA for storage |
| `stack_allocation` | true | Frame storage for eligible values |
| `region_allocation` | true | Per-function region for larger bounded arrays |
| `stack_array_element_limit` | 64 | Above this, region or heap |

Heap remains for: returned values, globals, unsafe capture, unknown calls, phi-merged values, resized arrays. Regions released on every generated return. Source-visible ownership does not change.

Native IR optimization uses `[optimization]` via `optimizer_options()`, including `profile_guided` without the JS effort AND-gate.
