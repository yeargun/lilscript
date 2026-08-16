# Frontend, linking, and lowering

Parent: [compilation](README.md). Language contract:
[`docs/language-v0.1.md`](../../language-v0.1.md). Source anchors:
`src/lexer.rs`, `src/parser.rs`, `src/module.rs`, `src/semantic.rs`, `src/lower.rs`,
and IR definitions in `src/ir.rs`.

## Stage contract

1. `lex` / `parse_source` turn UTF-8 LilScript into its own AST. JavaScript and
   TypeScript are never accepted as LilScript grammar.
2. `discover_modules_configured` resolves relative modules, locked packages, foreign
   edges, and literal dynamic imports. It rejects static cycles and invalid lazy
   initializers.
3. `link_modules` validates imports/exports, gives private bindings stable
   module-qualified identities, preserves initialization order, and records reusable
   export/dynamic-module boundaries.
4. `semantic::analyze` resolves nominal/generic types, calls, overload-free members,
   narrowing, defaults, effects contracts, and backend-independent diagnostics.
5. `lower_to_control_flow` plans functions/closures/captures and emits typed basic
   blocks, locals, phis, structured-region metadata, globals, exports, lazy modules,
   and foreign imports.
6. `promote_locals_to_ssa` handles eligible mutable locals. Exception-sensitive and
   shared-capture locals remain mutable so observation order is exact.

Linking before semantics is intentional: every cross-file binding has one identity,
so type/effect/escape/call-graph analysis sees the complete static world. Source
modules do not become JS closure wrappers.

## Failure boundaries

Lex, parse, module, semantic, lowering, and backend errors keep a source span and are
mapped back through `locate_linked_span`/compiler diagnostics. A failed optional
optimizer candidate may be discarded, but a failure in the configured frontend or
lowering path is a compilation error; search is never a recovery mechanism for an
invalid program.

The AST-direct `src/codegen_js.rs` path exists for legacy source helpers. Configured
path compilation and compression work must use typed control-flow IR plus
`src/codegen_ir_js.rs`.
