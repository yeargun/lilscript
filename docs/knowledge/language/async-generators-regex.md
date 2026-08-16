# Async tasks, generators, and regular expressions

Parent: [language](README.md). Contracts: [tasks/exceptions](../../language-v0.1.md#async-tasks-and-exceptions),
[generators](../../language-v0.1.md#generators), and `Regex` in
[types](../../language-v0.1.md#types). Compiler anchors: semantic checks in
`src/semantic.rs`, IR operations in `src/ir.rs`, and direct JS rendering in
`src/codegen_ir_js.rs`.

These features use the JavaScript platform directly; LilScript does not ship a
scheduler, generator runtime, or regex engine.

- `async T f()` has `T` inside its body and returns `Task<T>` to callers. `await` is
  legal only in async code and accepts a task.
- `Task.resolve`, `reject`, `all`, `then`, `catch`, and `finally` are typed views of
  native promises. Rejections remain arbitrary non-`void` JS values.
- `generator T f()` returns `Generator<T>` and may `yield T` or `yield*` a compatible
  array, typed array, or generator. Async generators are not in the core.
- `Regex` preserves ECMAScript construction errors, flags, source metadata, and
  stateful `global`/`sticky` testing.

JavaScript emission uses native `async`/`await`, `Promise`, `function*`/`yield`, and
`RegExp`. C/native rejects all three feature families rather than approximating them.

The `regex-literals` decision handles only a conservative statically valid pattern
and flag subset, and is active only with the explicit
`javascript.assume_pristine_builtins = true` contract. Open-world library output
retains the constructor so an ambient `RegExp` replacement remains observable;
complex or invalid patterns also retain constructor timing. The compact
generator-star decision and regex representation remain complete-artifact codec
candidates. Exception and async effects prevent unsafe reordering or DCE.

Evidence must include settle/order traces, thrown and non-object rejection reasons,
`finally`, promise flattening, generator suspension/cleanup, `yield*`, regex invalid
construction, repeated stateful tests, and native rejection diagnostics.
