# Async and exception benchmark

This JavaScript-only gate exercises direct `async`/`await`, typed `Task<T>`
operations, `throw`, `try`/`catch`/`finally`, binding-free catches, thrown `null`,
rejected tasks, return-through-finally behavior, and mutable locals observed by a
catch. It separately selects gzip and Brotli configurations with
`unused-catch-binding-elision` enabled and disabled.

The optional rewrite removes a catch parameter only when semantic use counts
prove that its source binding is unused. The gate requires smaller gzip and
Brotli artifacts, identical output, no material runtime regression, and no
material retained-heap regression.

Run `node benchmarks/async-exceptions/run.mjs`.
