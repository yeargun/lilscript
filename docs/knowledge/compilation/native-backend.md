# Native backend

Parent: [compilation](README.md). Language portability:
[JS vs native](../language/js-vs-native.md). Config: [`[native]`](../config/native.md).
Source anchors: `compile_program_to_c_configured` in `src/compiler.rs` and
`emit_native_c_with_options` in `src/codegen_native.rs`.

JavaScript and native share parsing, semantics, linking, typed control-flow IR, and
the core optimizer. `--target all` clones that IR and applies backend-specific
optimizer/options separately; JavaScript priority/search never changes C policy.

The native backend emits C and, for `native`, invokes the configured system C
toolchain. Scalar primitives use C representations; arrays, strings, records,
collections, unions, closures, and generic boundaries use the checked LilScript
runtime representations documented in the language contract.

`[native]` controls conservative storage placement:

- fixed small local arrays and eligible nonescaping aggregates/closures may use the
  function frame;
- larger bounded locals may use a function region;
- returned, global, unknown-call, unsafe-capture, phi-merged, or resized values stay
  heap allocated;
- every region is released on every generated return path.

This changes allocation placement, not source ownership or identity.

JavaScript-only boundaries/features—`JsValue`, `Regex`, tasks/async, generators,
dynamic/foreign imports, host objects, and currently inheritance—produce diagnostics.
Native must reject them rather than silently approximate browser semantics.

Portable conformance requires generated JS, generated C compiled independently, and
native execution to match the same oracle. C compilation success alone is not a
semantic result.
