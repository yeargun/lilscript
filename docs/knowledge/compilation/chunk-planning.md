# Chunk planning

Parent: [Compilation](README.md). Language: [modules](../language/modules-lazy.md). Config: [`[bundle]`](../config/bundle.md). Code: `plan_javascript_chunks`, `score_javascript_chunk_plan` in `src/compiler.rs`. Contract: [`docs/modules-and-delivery.md`](../../modules-and-delivery.md).

## Order of operations

1. Link and **whole-program optimize** (exports preserved when emitting a reusable/split graph).
2. Decide which functions may move (`ir_function_can_move_to_chunk`: not global writers).
3. Build per-source-module candidate chunks.
4. Lazy-only modules are **mandatory** chunks.
5. `preserve-modules`: return all movable per-module chunks. Limits do not override.
6. `split`: form mandatory lazy chunks, then consider eager modules imported by ≥
   `shared_min_imports`; drop eager candidates smaller than `min_chunk_bytes`; greedily
   add an optional eager/shared chunk only when it **strictly lowers** the complete
   deploy cost. The first optional chunk receives no exception. If mandatory lazy
   chunks alone would exceed `max_chunks`, compilation fails instead of violating the
   configured cap.
7. Emit ESM + `<entry-stem>.manifest.json`.

`single` returns no extra chunks.

`preserve-modules` deliberately ignores `min_chunk_bytes`, `shared_min_imports`, and
`max_chunks`: preserving source-module identity is the selected contract. `split`
enforces `max_chunks` across both mandatory lazy and selected optional chunks.

## Deploy cost (not the JS `cost_model` alone)

`[bundle.cost]` weights integer byte-equivalents:

- `raw_weight`, `gzip_weight`, `brotli_weight` — at least one nonzero (defaults 0 / 1 / 2)
- `request_overhead_bytes` (default 1000)
- `dependency_depth_penalty_bytes` (default 160)
- `preload_request_discount_percent` (default 70)
- `cache_reuse_discount_percent` (default 20)

Candidate chunk **code** is always measured gzip-9 and Brotli-11 regardless of
`javascript.cost_model`, using the same statically linked stock-zlib/official-Brotli
functions as single-artifact selection and `lilscript-codec`. Preload policy changes
whether extra requests are discounted.

## Joint chunk/symbol search

`joint-chunk-symbol-search` (size-first, level ≥ 14) scores chunk plans together with
layout and name-reserve emission variants, and the winning `IrJsOptions` are used for
the final emitted chunks. Split mode still does **not** rerun the full single-file
two-level IR/emission beam per chunk; this joint search is a narrower bundle-specific
frontier.

## Search limits

Split partitioning is deterministic and bounded, not an exhaustive global partition
solver. Eligible optional eager modules are ordered by provisional emitted raw size
(then module identity), the frontier is capped at
`max(max_chunks * 8, 32)`, and the planner greedily adds the best single next chunk
whose complete plan strictly improves deploy cost. It does not backtrack across
combinations where two chunks win only together. Each retained trial is still scored
as a complete deployment; “greedy” describes which partitions are proposed, not a
local byte estimate.

## Manifest v2

Build id, preload files, aggregate deploy cost; per chunk: kind, source modules, raw/gzip/brotli, static and dynamic deps, SHA-256 cache key, deploy cost. Names are stable source-path identities; content changes show up in cache keys without renaming unrelated chunks.

## Lilpack vs LilScript chunks

Lilpack production still compiles `.lil` as **single** ESM (`--delegate-bundling`) and lets Vite chunk the mixed JS/TS/CSS graph. Use LilScript `split` / `preserve-modules` when the deployable graph is LilScript-only (or when you want the compiler’s lazy `import()` chunks explicitly). See [Lilpack](../delivery/lilpack.md).
