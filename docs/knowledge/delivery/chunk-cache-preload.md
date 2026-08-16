# Chunk cost, cache identity, and preload

Parent: [delivery](README.md). Planner:
[chunk planning](../compilation/chunk-planning.md). Config: [`[bundle]`](../config/bundle.md).

`[bundle.cost]` turns a complete emitted plan into deterministic byte-equivalent cost:
weighted raw/gzip/Brotli sizes plus request overhead and dependency depth, adjusted by
preload and shared-reachability/cache-reuse discounts. These are deployment policy
weights, not measured network timings.

Candidate chunk code is independently measured as raw, gzip-9, and Brotli-11 through
the shared canonical scorer functions. HTTP normally compresses files independently;
a concatenated stream cannot replace the artifact-set result.

`preload` policy:

| Value | Emission |
|---|---|
| `none` | no generated modulepreload links |
| `entry` | preload lazy roots requested directly by entry |
| `all` | preload every lazy root |

The small generated guard is inert without `document`. Preload can reduce depth but
increases eager requests/bytes; its configured discount lets plan scoring express
that trade.

Manifest v2 records build id, aggregate deploy cost, preload files, and each chunk's
kind, source modules, static/dynamic dependencies, exact transport sizes, cache key,
and deploy cost. Filenames use stable source-path identity; content changes update the
SHA-256 cache key without renumbering unrelated chunks.

Joint chunk/symbol search compares a narrow set of function layout/name-reserve
options and carries the winning options into final emission. It does not run the full
single-artifact two-level search per chunk.
