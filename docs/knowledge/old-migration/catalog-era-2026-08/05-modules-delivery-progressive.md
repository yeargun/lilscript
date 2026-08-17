# Phase 05 — modules, delivery, and progressive enhancement

Parent: [migration](README.md). Delivery model:
[delivery](../delivery/README.md). Bundle config: [`[bundle]`](../config/bundle.md).

## Objective

Measure what users download at equivalent application boundaries. A tiny entry chunk
that merely moves bytes into an uncounted lazy chunk is not a win.

## Required lanes

| Boundary | Compare |
|---|---|
| Single closed program | LilScript `single` vs JS bundler single-file production output |
| Static module graph | Full reachable artifact set, entry behavior, tree shaking |
| Manual/preserve modules | Matching public ESM files and stable export contract |
| Automatic split | Entry + eager dependencies + manifest; weighted full deploy plan |
| Typed dynamic import | Initial transfer, lazy transfer, request count/depth, runtime behavior |
| Lilpack mixed app | Vite/Rolldown output for equivalent `.lil` and JS/TS application graphs |
| Progressive enhancement | Bytes and host work before enhancement plus eventual behavior |

## Work

- Define artifact-set measurement rather than summing unrelated per-file minima.
- Verify lazy-only modules have no eager top-level effects.
- Cover dead dynamic imports, live imports, cycles, shared chunks, preload policies,
  max/min chunk limits, foreign imports, CSS/assets/workers, and cache-oriented
  preserve-module builds.
- Pin planner edge cases: a costlier first optional eager chunk is rejected, mandatory
  lazy chunks over `max_chunks` fail in `split`, `preserve-modules` remains exempt,
  and joint chunk/symbol winners use the scored emission options in final files.
- Run public-name and closed-app configurations separately.
- Record request/dependency-depth weights alongside raw/gzip/Brotli bytes.

## Exit criteria

- Every bundle mode and preload policy has behavior and deploy-cost fixtures.
- The report names initial, lazy, total, and weighted deploy sizes; no one number is
  presented as all four.
- Manual bundling and progressive enhancement are first-class gates, not prose-only
  promises.
- Lilpack comparisons attribute what LilScript, Vite/Rolldown, and any selected
  minifier each changed.
