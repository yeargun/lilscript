# Progressive enhancement

Parent: [Delivery](README.md). Language: [modules](../language/modules-lazy.md). Lint: [`docs/configuration.md`](../../configuration.md) `[lint]`.

## First bytes

The size mission is **what is served before the app is usable**, not only the fully loaded SPA.

Tools the language gives you:

1. **Dead code in the closed world** — unused routes never exist as JS if they are not imported.
2. **Typed `import()`** — enhancement / admin / animation code becomes a mandatory lazy chunk in `split` mode; in `single` it is still tree-shaken if the `import()` is dead, or inlined as `Promise.resolve` if live.
3. **Init-free lazy modules** — forbids top-level host work in a file you thought was lazy.
4. **`web/eager-host-access`** — lint for top-level host operations that run before a PE boundary. Enable the `web` provider.
5. **Zero-wrapper DOM** — the eager path does not pay for a compatibility runtime.
6. **`preload`** — `entry` or `all` can modulepreload lazy roots when the extra request is cheaper than a later round-trip (`[bundle.cost]` discounts).

## Manual control

Authors choose boundaries. The compiler will not invent a PE strategy. Put host `document` work behind a function, import that function from a lazy module, and keep the entry as types + a small boot. Config then decides whether that lazy module is a network boundary (`split`) or part of one file (`single`).

## What PE is not

- Not a LilScript runtime that hydrates islands.
- Not Closure `ADVANCED` `MODULE_*` sugar.
- Not “split everything”: `min_chunk_bytes` exists because extra requests have a deploy-cost penalty (default 1000 byte-equivalents each).
