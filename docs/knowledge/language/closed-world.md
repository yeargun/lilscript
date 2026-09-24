# Closed world

Parent: [Language](README.md). Related: [modules](modules-lazy.md), [boundaries](boundaries-escape.md).

## Compilation unit

The entry `.lil` file plus every **transitive static import** is one compilation unit. Module discovery and parsing come first; one module-graph checker then resolves every import and export and checks the whole unit, which is compiled as one program. Source modules are not JavaScript wrappers in the generated bundle.

```
discover → parse → check the module graph → elaborate once → optimize the whole program → emit / deliver
```

Private functions, variables and structs in different files cannot collide. **Until plan M4.1**, two modules' private classes or enums with the same name are refused, because the checker still keys classes and enums by name. Side-effect-only `import "./startup.lil"` still runs in dependency-first order. Static import cycles are errors. Dynamic import cycles are allowed.

## Two worlds

| Target | World | What `export` means |
|---|---|---|
| `js`, `c`, `native`, `all` | Closed executable | Accessibility for other `.lil` files. **Not** a DCE root. Unused exports die. |
| `js-module` | Reusable library | Root **runtime** exports are retention roots. Internals still mangle. Compact `export{b as square}`. Type-only struct/class exports emit no JS binding. |

This is the opposite of TypeScript `export` which is both a type and a JS binding unless `import type` is used. LilScript does not need that glue: structs/classes are type exports; functions/globals are runtime.

## Why closed world exists

Cross-file inlining, scalar replacement, function folding, and tree shaking are unsound if an unseen JS file can reach into a module’s privates. The closed world makes “unseen JS” require `extern` or a `js-module` export.

Foreign JS/TS is not inside the world. `import extern` plus a matching `extern` contract is an explicit hole. Lilpack/Vite owns that hole after LilScript finishes. See [delivery](../delivery/lilpack.md).

## Lockfile

`lilscript.lock` pins the transitive path-dependency graph, semver, ABI, and SHA-256 of every `.lil` plus package metadata. Normal builds never rewrite it. Stale hashes, ABI mismatch, and path escape are hard errors. That prevents open-world drift that would silently disable whole-program proofs.

## Config that changes the world

- `[bundle].mode` — still optimizes the full graph first; only delivery changes. [`[bundle]`](../config/bundle.md)
- `--target js-module` vs executable targets — retention roots
- `[package]` / `[dependencies]` — what bare imports may see
- `mangle.preserve_properties` and declared boundaries (D2) — how much of the library world is a public JS ABI. `public_aggregate_abi` and `mangle.exports` are retired: public aggregates are plain objects with named fields, and a library keeps its export names
