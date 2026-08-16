# Typed lazy loading

Parent: [delivery](README.md). Language contract:
[modules and lazy loading](../language/modules-lazy.md). Source anchors: dynamic
import discovery/linking in `src/module.rs`, lazy-module IR in `src/ir.rs`, and chunk
planning/emission in `src/compiler.rs` and `src/codegen_ir_js.rs`.

`import("./feature")` accepts a compile-time literal and returns a
`Task<module>`. The module namespace exposes declared runtime exports with their
LilScript types; unused namespace properties can tree-shake. Literal identity keeps
the graph deterministic and prevents runtime filesystem/package search.

A module reached only lazily may declare functions, structs, and classes, but no
top-level executable statement or variable. This avoids promising a lazy boundary
while running initialization eagerly. Put initialization in an exported function.

Delivery depends on mode:

- `single`: live dynamic imports lower to a resolved typed namespace in the same
  artifact; dead imports may disappear;
- `split`: lazy-only roots/dependencies are mandatory ESM chunks;
- `preserve-modules`: source modules remain separate where movable;
- Lilpack/Vite: foreign and mixed-graph dynamic imports follow Vite semantics after
  LilScript emits its linked ESM side.

Dynamic cycles are legal; static cycles are not. Loader-generated failures expose
stable nullable `specifier`/`message`, while user task rejections remain arbitrary
`JsValue`. Native rejects dynamic tasks because no portable C chunk ABI is claimed.

Verification reports initial requests/bytes, each lazy load, full reachable bytes,
execution order, failure behavior, and unused-export pruning.
