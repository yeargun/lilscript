# Closed world

Parent: [Language](README.md). Related: [modules](modules-lazy.md), [boundaries](boundaries-escape.md).

## Compilation unit

The entry `.lil` file plus every **transitive static import** is one compilation unit. Module discovery and parsing come first; one module-graph checker then resolves every import and export and checks the whole unit, which is compiled as one program. Source modules are not JavaScript wrappers in the generated bundle.

```
discover → parse → check the module graph → elaborate once → optimize the whole program → emit / deliver
```

Private functions, variables, structs, classes and enums in different files have distinct module identities. Side-effect-only `import "./startup.lil"` still runs in dependency-first order. Static cycles may link when the checker's initialization and use rules permit them; dynamic import cycles are allowed.

## Two worlds

| Target | World | What `export` means |
|---|---|---|
| `js`, `c`, `native`, `all` | Closed executable | Accessibility for other `.lil` files. **Not** a DCE root. Unused exports die. |
| `js-module` | Reusable library | Root **runtime** exports are retention roots. Internals still mangle. Compact `export{b as square}`. Type-only struct/class exports emit no JS binding. |

This is the opposite of TypeScript `export` which is both a type and a JS binding unless `import type` is used. LilScript does not need that glue: structs/classes are type exports; functions/globals are runtime.

Classic application scripts default to a private IIFE. Separate loads have separate root bindings, and generated names cannot collide with another script's roots. The wrapper preserves the entry's strictness and lexical `this`; closed-world compilation does not make a script strict. Its bytes participate in the selected objective's exact score. Explicit `format = "bare"` is for an embedding that supplies the private root scope, not a promise to publish globals or preserve their names.

## Why closed world exists

Cross-file inlining, scalar replacement, function folding, and tree shaking are unsound if an unseen JS file can reach into a module’s privates. The closed world makes “unseen JS” require `extern` or a `js-module` export.

Foreign JS/TS is not inside the world. `import extern` plus a matching `extern` contract is an explicit hole. Lilpack/Vite owns that hole after LilScript finishes. See [delivery](../delivery/lilpack.md).

## Lockfile

`lilscript.lock` pins the transitive path-dependency graph, semver, ABI, and SHA-256 of every `.lil` plus package metadata. Normal builds never rewrite it. Stale hashes, ABI mismatch, and path escape are hard errors. That prevents open-world drift that would silently disable whole-program proofs.

## Config that changes the world

- `[delivery].mode` — still optimizes the full graph first; only delivery changes. `[bundle]` is a diagnosed compatibility alias.
- `[target.javascript].format` — the delivered container; `auto` resolves from the target
- `--target js-module` vs executable targets — retention roots
- `[package]` / `[dependencies]` — what bare imports may see
- `mangle.preserve_properties` and declared boundaries (D2) — how much of the library world is a public JS ABI. `public_aggregate_abi` and `mangle.exports` are retired: public aggregates are plain objects with named fields, and a library keeps its export names
