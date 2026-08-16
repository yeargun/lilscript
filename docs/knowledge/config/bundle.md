# `[bundle]`

Parent: [Config](README.md). Language: [modules](../language/modules-lazy.md). Algorithm: [chunk planning](../compilation/chunk-planning.md).

Separate from optimizer policy. Every mode links and optimizes the **complete** static graph first.

## Keys

| Key | Default | Meaning |
|---|---|---|
| `mode` | `single` | `single` \| `split` \| `preserve-modules` |
| `min_chunk_bytes` | 16384 | Eager shared chunks below this stay in the entry (`split`) |
| `max_chunks` | 32 | Cap on mandatory + selected chunks (`split`); compile error if lazy requirements alone exceed it |
| `shared_min_imports` | 2 | Eager module must be imported by at least this many modules |
| `preload` | `none` | `none` \| `entry` \| `all` |

`split` / `preserve-modules` require `--output`. They write the entry, sibling chunks, and `<stem>.manifest.json`. Use `.mjs` when running in Node without `"type": "module"`.

`--delegate-bundling` (Lilpack) forces `single` so Vite owns mixed-graph chunking.

## `[bundle.cost]`

Integer **policy** weights, not runtime measurements. At least one of `raw_weight` / `gzip_weight` / `brotli_weight` must be nonzero.

Defaults: raw 0, gzip 1, brotli 2 — Brotli bytes contribute twice the gzip weight,
while raw bytes contribute nothing unless configured. This is a weighted sum, not a
lexicographic “Brotli, then gzip” ordering.

Request/depth values are byte-equivalent penalties. Preload and cache values are 0–100 percents.

This score is **independent** of `javascript.cost_model`, but both exist so transfer-size philosophy applies to multi-file deploys too (requests and cache reuse are part of “what the user downloads over time”).

## Lazy vs optional

- Lazy `import("./x")` → **mandatory** chunk for a lazy-only module.
- Shared eager modules → **optional**; every one, including the first, must strictly
  lower complete deploy cost.
- `preserve-modules` honors source-module identity and is exempt from the split-mode
  size/count filters.
