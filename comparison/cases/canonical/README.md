# Canonical paired cases

Parent: [verification](../../../docs/knowledge/verification/README.md).
Migration: [phase 00](../../../docs/knowledge/migration/00-canonical-runner.md).

Each folder is one LilScript program and one independently authored JavaScript
program with the same stdout contract. The runner minifies the JS (Terser, Oxc,
esbuild) and compiles the LilScript with raw/gzip/Brotli gold configs. LilScript
must be no larger than the best valid JS artifact in each gated metric.

```sh
node comparison/cases/run.mjs --canonical-only
```

Families: `scalars`, `strings`, `control`, `functions`, `aggregates`,
`collections`, `effects`, `host`, `wins`.

A `--canonical-only` run on this tree must keep every case `le` or `lt` under
raw, gzip-9, and Brotli-11 versus the best valid Terser/Oxc/esbuild artifact.
The latest local run was 47/47 with strict wins in all three lanes.
