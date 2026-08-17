# LilScript vs web minifiers

This is the compression regression gate for small, semantically paired web
programs. Hand-authored pairs live in `canonical/<family>/<id>/`. Parameterized
variants still come from `catalog.mjs` and are materialized under `generated/`.

```sh
node comparison/cases/run.mjs --canonical-only
node comparison/cases/run.mjs --only aggregates/
node comparison/cases/run.mjs
```

`--canonical-only` still verifies the catalog oracle, then runs only folder cases.
`--only` matches catalog names and canonical paths. The full command runs both.

## Hard gate

1. The original JavaScript defines the stdout oracle.
2. Every Terser, Oxc, and esbuild candidate must preserve that stdout.
3. LilScript is compiled three times with the checked-in `raw`, `gzip`, and
   `brotli` gold configs. Each uses `candidate_search = "always"`, so the stated
   1536 limit is real rather than production mode's 384-candidate cap. Every
   artifact must preserve the same stdout.
4. The raw lane must be no larger than the smallest raw JavaScript candidate.
5. The gzip lane must be no larger than the smallest gzip-9 candidate.
6. The Brotli lane must be no larger than the smallest Brotli-11 candidate.
7. A case marked `"expect": "lt"` must be strictly smaller in all three lanes.

Those are three independent compilations, not a Pareto requirement on one file. The
raw artifact gates only raw, the gzip artifact only gzip-9, and the Brotli artifact
only Brotli-11. Cross-metric sizes are recorded for diagnosis and may lose.

There is no small-file exemption. Codec framing effects are real served bytes,
so a one-byte compressed loss is a regression. Baselines are chosen separately
per metric; LilScript cannot pass by comparing against a convenient but larger
JavaScript artifact. LilScript output is not post-minified.

```sh
node comparison/cases/run.mjs --canonical-only
node comparison/cases/run.mjs --only struct
node comparison/cases/run.mjs
```

Generated cases, emitted artifacts, and reports are ignored build products.
`catalog.mjs`, `canonical/`, `oracle-manifest.json`, the three files under `configs/`, and this
runner are the durable source of truth. The checked-in oracle is one digest for
the complete generated catalog. Canonical folders are reviewed in git. Every run,
including `--only` and `--canonical-only`, verifies the catalog oracle before selecting cases.

After intentionally reviewing a reference-program or expected-output change,
update the oracle explicitly:

```sh
node comparison/cases/run.mjs --update-oracles
```

All baseline lanes target ES2022. Terser gets three compression passes and
top-level compression/mangling; Oxc gets top-level mangling; and esbuild is
measured both as a semantics-preserving script transform and as a closed-world
IIFE that permits top-level name mangling. The wrapper is not assumed to win:
the best valid artifact is selected independently for each metric.

The runner loads the pinned Terser, Rolldown/Oxc, and esbuild versions from
`benchmarks/popular/package-lock.json`. Set them up with:

```sh
nvm use
npm ci --prefix benchmarks/popular
```

The repository pins Node 24 in `.nvmrc`; Vite/Oxc's native tooling rejects older
Node 20 releases even when plain JavaScript execution still works.

Unless `LILSCRIPT` and `LILSCRIPT_CODEC` name an explicit compiler/scorer pair,
each run invokes
`${CARGO:-cargo} build --release --bin lilscript --bin lilscript-codec` before
testing. Supplying only one override is rejected, so a report cannot mix
unrelated compiler and codec builds. This avoids silently benchmarking a stale
`target/release/lilscript`. CI installs the
minifier dependencies, and `comparison/run-all.sh` invokes this suite, making it
part of `scripts/release-check.sh`.

The schema-5 `summary.json` records each candidate's semantic validity, sizes,
artifact digest, and duration; the exact minifier options and ES target;
metric-specific winners; separate failed-case and failure-event counts; Node,
codec, platform, Oxc binding, and compiler provenance; and digests for every
source, oracle, corpus, runner, and lane config. A baseline candidate that
changes stdout fails its case and is excluded from size selection. Runtime
checks time out after 10 seconds and each LilScript case compilation after 120
seconds, so a hanging optimizer or program is a gate failure instead of a
stalled release job.

See the linked verification policy in
[`docs/knowledge/verification/README.md`](../../docs/knowledge/verification/README.md).
