# Closed world

Parent: [Language](README.md). Related: [modules](modules-lazy.md), [boundaries](boundaries-escape.md).

## Compilation unit

The entry `.lil` file plus every **transitive static import** is one compilation unit. Module discovery, export validation, and private-name linking happen **before** semantic analysis. Module boundaries are erased before SSA. They are not JavaScript wrappers in the generated bundle.

```
discover → parse → link (rename to $m{id}$name) → analyze → lower → optimize whole program → emit / partition
```

Private names in different files cannot collide. Side-effect-only `import "./startup.lil"` still runs in dependency-first order. Static import cycles are errors. Dynamic import cycles are allowed.

## Two worlds

| Target | World | What `export` means |
|---|---|---|
| `js`, `c`, `native`, `all` | Closed executable | Accessibility for other `.lil` files. **Not** a DCE root. Unused exports die. |
| `js-module` | Reusable library | Root **runtime** exports are retention roots. Internals still mangle. Compact `export{b as square}`. Type-only struct/class exports emit no JS binding. |

This is the opposite of TypeScript `export` which is both a type and a JS binding unless `import type` is used. LilScript does not need that glue: structs/classes are type exports; functions/globals are runtime.

## Why closed world exists

Cross-file inlining, scalar replacement, identical-function folding, and tree shaking are unsound if an unseen JS file can reach into a module’s privates. The closed world makes “unseen JS” require `extern` or a `js-module` export.

Foreign JS/TS is not inside the world. `import extern` plus a matching `extern` contract is an explicit hole. Lilpack/Vite owns that hole after LilScript finishes. See [delivery](../delivery/lilpack.md).

## Lockfile

`lilscript.lock` pins the transitive path-dependency graph, semver, ABI, and SHA-256 of every `.lil` plus package metadata. Normal builds never rewrite it. Stale hashes, ABI mismatch, and path escape are hard errors. That prevents open-world drift that would silently disable whole-program proofs.

## Config that changes the world

- `[bundle].mode` — still optimizes the full graph first; only emission/partitioning changes. [`[bundle]`](../config/bundle.md)
- `--target js-module` vs executable targets — retention roots
- `[package]` / `[dependencies]` — what bare imports may see
- `javascript.public_aggregate_abi` / `[mangle].exports` — how much of the library world is a public JS ABI
