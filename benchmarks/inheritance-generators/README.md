# Inheritance and generators gate

This gate exercises generic single inheritance, base-first flattened field layouts, explicit
`super(...)` chaining, subtype upcasts, inherited direct method calls, generator functions,
`yield`, `yield*`, generator methods, iterator closing through `finally`, and native JavaScript
`for...of` emission.

The `compact-generator-star` compression decision controls only the grammar-equivalent spelling
`function*name` versus `function* name`. The gate requires the enabled spelling to be strictly
smaller for the selected gzip and Brotli artifacts. Runtime and retained-memory gates compare the
enabled Brotli-objective artifact with the disabled Brotli spelling and a hand-written JavaScript
implementation; the machine report labels that runtime artifact `brotli-on`.
The hand-written reference is already manually minified and uses native classes, `extends`,
generator syntax, and method calls; the gate also requires Lilscript's flattened output to be
strictly smaller in raw, gzip, and Brotli form. Those three claims use independent
raw-, gzip-, and Brotli-cost-model builds and gate only the matching metric. The
other metrics of each artifact are diagnostic and may lose.

Run with:

```sh
node benchmarks/inheritance-generators/run.mjs
```
