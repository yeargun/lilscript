# Semantic route against pinned competitors and the default route (2026-09-22)

Evidence for migration milestones 012 and 013 on the reference ports. The artifacts were built from source with the semantic route through each port's own build script (`scripts/build.mjs --compile`, compiler selected with `LILSCRIPT_COMPILER`, migration patches applied).

## Competitors (013)

`finer/tools/competitor-recipes.mjs <port> --ours <artifact>` bundles the upstream npm package with esbuild, leaving external what the port leaves external, and minifies it with Terser 5.51.2, esbuild 0.28.1 and Oxc through Rolldown 1.2.5. Comparability requires the same published names and raw size within 0.8–1.25. Receipts: [katexlil](katexlil/receipt.json), [zodlil](zodlil/receipt.json).

| Boundary | Semantic route (complete delivery) | Terser | esbuild | Oxc | Verdict |
|---|---|---|---|---|---|
| katexlil (`dist/katex.esm.js`) | 296,385 raw / 81,607 gzip / 67,061 Brotli | 63,044 | 63,767 | 63,253 | loss 4,017 Brotli against Terser (the default route's committed dist loses 1,576) |
| zodlil (`dist/zod.core.js`) | 127,782 / 32,848 / 28,326 | 52,561 | 55,133 | 54,819 | not comparable: 2 published names against upstream's 240 |
| markedlil | | | | | not comparable (8 names against 18, recorded in 001) |

## Compile time and memory (012)

`/usr/bin/time`, four worker threads, each port's own configuration, rounds alternating between routes on this host, which is a burstable B8als_v2 and throttles after about 30 minutes. The run was stopped after round 2, since the gaps are two orders of magnitude. Raw rows: [compile-pairs.txt](compile-pairs.txt) (wall, user, system seconds, peak RSS in KB, raw bytes, Brotli bytes).

| Port | Rounds | Default route, median wall / peak RSS | Semantic route, median wall / peak RSS | Brotli, default / semantic |
|---|---|---|---|---|
| probelil | 2 | 27.0 s / 32 MB | 0.27 s / 14 MB | 1,492 / 1,872 |
| markedlil | 2 | 174.8 s / 83 MB | 1.02 s / 21 MB | 9,360 / 9,398 |
| zodlil | 2 | 189.7 s / 244 MB | 3.00 s / 45 MB | 29,682 / 28,326 |
| katexlil | 1 | 352.1 s / 259 MB | 4.18 s / 68 MB | 55,404 / 57,589 |
