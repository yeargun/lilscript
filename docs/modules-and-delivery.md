# Modules and Delivery

LilScript resolves a closed, typed module graph before SSA optimization. Static
imports remain the default because they permit cross-file inlining, scalar
replacement, and complete tree shaking without a runtime loader.

## Dynamic modules

`import()` accepts a compile-time string specifier and returns a typed
`Task<module>` value. It is not an untyped JavaScript `Promise` boundary. The
module namespace exposes the target module's runtime exports with their declared
LilScript signatures, and unused namespace properties are not retained as chunk
exports.

```lilscript
import("./feature")
  .then((auto feature) => print(feature.answer(40)))
  .catch((auto error) => print(`${error.specifier}: ${error.message}`));
```

`Task<T>` provides `then`, `catch`, and `finally`. A `then` callback receives
`T`; a `catch` callback receives `ModuleLoadError`, whose stable fields are
`specifier` and `message`. Promise flattening is reflected in the result type.
`auto` is permitted for arrow parameters only when the callback has a contextual
type.

Dynamic specifiers must be string literals. This keeps the graph deterministic,
lets the type checker load the exact export interface, and prevents a runtime
filesystem search. Dynamic cycles are legal; static import cycles remain a
compile error.

Lazy-only modules are initialization-free: they may declare functions, structs,
and classes, but may not contain top-level executable statements or variables.
The compiler rejects such modules instead of silently running a supposedly lazy
initializer in the entry artifact. Put initialization in an exported function.

Dynamic module tasks are JavaScript-only. Native targets report a source
diagnostic because LilScript does not claim that a JavaScript chunk has a
portable C ABI.

## Chunk planning

`bundle.mode = "single"` keeps the asynchronous type but lowers dynamic imports
to `Promise.resolve(namespace)` inside one artifact. `split` and
`preserve-modules` emit real ESM chunks. Lazy roots and their lazy-only static
dependencies are mandatory chunks; optional shared chunks are selected after
whole-program optimization.

The split planner measures every complete candidate deployment. Its score uses:

- exact raw, gzip, and Brotli bytes with configurable weights;
- request overhead and dependency depth;
- static and dynamic reachability;
- module-preload request discounts;
- shared-source reachability as a long-term cache reuse benefit;
- `min_chunk_bytes`, `shared_min_imports`, and `max_chunks` constraints.

`preload = "none"`, `"entry"`, or `"all"` controls deterministic
`modulepreload` link creation. The emitted guard is inert outside browsers.

Manifest version 2 records a SHA-256 build id, preload files, aggregate deploy
cost, and, for every chunk, its kind, source modules, raw/gzip/Brotli sizes,
static and dynamic dependencies, SHA-256 cache key, and deploy cost. Chunk names
use a stable source-path identity; content changes are represented by cache keys
without renumbering unrelated chunks.

## Packages and lockfiles

Bare imports resolve through `[dependencies]`. The current package transport is
an explicit local path, which keeps resolution auditable and works for monorepos
without a registry protocol.

```toml
[dependencies]
mathkit = { path = "../mathkit", version = "^1.2", abi = 1 }
```

Each dependency has package metadata:

```toml
[package]
name = "mathkit"
version = "1.2.0"
abi = 1
entry = "src/lib.lil"
```

Generate or refresh the lockfile with:

```sh
lilscript src/main.lil --write-lock -o build/app.js
```

`lilscript.lock` is deterministic and portable. It pins the complete transitive
graph, semver versions, compiler ABI, relative source roots, entries, dependency
edges, and a SHA-256 hash over every `.lil` file plus the package manifest.
Normal compilation never rewrites it. A missing/stale lock, changed source,
version mismatch, ABI mismatch, path escape, symlink, or conflicting package
resolution is a hard error.

Dependency visibility is scoped to the importer. Root modules may import only
root dependencies, and package modules may import only dependencies declared by
that package. A transitive package therefore cannot become an undeclared,
accidental dependency.

Bare package subpaths such as `mathkit/vector` resolve inside the locked package
root. Absolute paths and package-root escapes are rejected.
