# Modules and lazy loading

Parent: [Language](README.md). Contract:
[modules and delivery](../../modules-and-delivery.md). Configuration:
[configuration reference](../../configuration.md).

Static imports enter one checked program, allowing cross-module optimization.
`import("./feature")` requires a literal specifier and returns a typed
`Task<module>`. Its namespace has live exports and shared identity across loads.
Unused namespace members can be removed when the namespace does not escape.

Top-level lazy effects run on the first load, in source dependency order. Loads
share the initialization result, including a failure. Cycles expose function
declarations during instantiation and retain errors for early lexical reads.
Dynamic tasks currently require the JavaScript target.

| `delivery.mode` | Behavior |
|---|---|
| `single` | Includes code in each entry artifact; lazy lexical activations execute on demand. Each entry has an independent instance. |
| `preserve-modules` | Retains source module boundaries, native live linkage and lazy files. |
| `split` | Shares code across entries and forms lazy files subject to initialization proofs. |

The default remains `single`. Its lazy runtime adds bytes and some load-time
bookkeeping; it does not make `import()` a network boundary. Split or preserved
output can postpone loading the code itself. Each selected raw/gzip/Brotli
objective measures its own complete output, including runtime and linkage.

Foreign declarations keep their explicit contracts. `delivery.host_modules`
selects external, automatic or embedded delivery. The compiler lowers supported
host JavaScript/erased TypeScript through typed syntax; unsupported embedding
receives a diagnostic or the configured external fallback. It does not type
check TypeScript.
