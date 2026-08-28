# Delivery

How compiled LilScript reaches a browser or Node process: compiler artifacts, Lilpack, lazy loading, and progressive enhancement.

Parent: [tree](../README.md). Language: [modules](../language/modules-lazy.md). Config: [`[bundle]`](../config/bundle.md). Contract: [`docs/modules-and-delivery.md`](../../modules-and-delivery.md).

## Pages

### Compiler vs app graph

- [Lilpack](lilpack.md) — Vite-backed app graph; LilScript still owns `.lil`
- [Reusable library vs closed app](library-vs-app.md)
- [Manual bundling](manual-bundling.md) — single / preserve-modules / split

### Lazy and first bytes

- [Typed lazy loading](lazy-loading.md)
- [Progressive enhancement](progressive-enhancement.md)
- [Chunk cost, cache, and preload](chunk-cache-preload.md)

## Two bundlers, one language

| Tool | Owns | Chunking |
|---|---|---|
| `lilscript` | Closed `.lil` world, SSA, codec search | `bundle.mode` single / split / preserve-modules |
| `lilpack` | Application graph: `.lil` + JS/TS/CSS/assets/npm | Vite/Rollup after `--delegate-bundling` (LilScript emits one ESM) |

LilScript at the **root** of a mixed app is intentional. JS is not the source of truth; it is a host and a foreign leaf.

## Manual vs automatic

The language is built so authors can:

- ship **one** compressed file (`single`);
- keep **source module** identity (`preserve-modules`) for cache granularity;
- ask the compiler to **search** shared/lazy splits under deploy cost (`split`);
- write `import("./feature")` for a **typed** lazy boundary the compiler must honor;
- keep a **public facade** (`mangle.exports = false`) or a fully mangled app.

Automatic split applies `min_chunk_bytes` / `shared_min_imports` to optional eager
chunks and enforces `max_chunks` across mandatory lazy plus selected optional chunks;
too many mandatory lazy chunks are a compile error. Preserve-modules intentionally
ignores those split filters because source identity is its contract.
