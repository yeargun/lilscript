# Manual bundling and partition control

Parent: [delivery](README.md). Algorithm:
[chunk planning](../compilation/chunk-planning.md). Config: [`[bundle]`](../config/bundle.md).

LilScript does not require one automatic partition policy. Authors choose semantic
boundaries in source and deployment policy in TOML:

- static imports maximize whole-program folding and do not force network files;
- literal dynamic imports declare mandatory lazy roots;
- `single` deliberately collapses delivery into one artifact;
- `preserve-modules` keeps movable source-module identity for manual cache/deployment
  granularity and ignores split size/count filters;
- `split` keeps mandatory lazy boundaries and greedily considers eligible shared eager
  modules under explicit thresholds and deploy cost.

“Manual” means the source graph and selected mode own boundaries. There is currently
no arbitrary named-chunk directive or user-written runtime loader. Chunk filenames
derive deterministically from source identity, and the manifest records content
hashes.

In `split`, an eager module must meet `shared_min_imports` and `min_chunk_bytes`, and
each optional chunk—including the first—must strictly reduce complete deploy cost.
Mandatory lazy chunks count toward `max_chunks`; overflow is an error. The optional
partition frontier is bounded and greedy, so it is a best-found plan, not proof of a
global partition optimum.

Mixed JS/TS/CSS/assets should normally use `--delegate-bundling`/Lilpack, which forces
one LilScript ESM artifact and lets Vite own the full application graph.
