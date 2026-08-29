# Large-library before/after evidence

This directory is the reproducible, fail-closed replacement for ad-hoc size
claims about the real sibling projects. It compares the same pinned library
source against two exact LilScript revisions:

- before: `5245f1790a9ee3d29e54fe72282da700dcc045d2`
- frozen checkpoint: `979dc90d5c10fddb1328ea3f707cd17d3869a3fe`

The checkpoint is a reproducible after-snapshot, not a claim that optimization
work is finished. When a final candidate lands, update the `checkpoint` Git
object and source hash in `matrix.json`; the runner itself has no revision
hard-coded outside that matrix.

The matrix currently has 15 pinned boundaries across five projects: packaged
SolidLil core, seven source-level MotionLil entry slices plus the retained broad
animate lane, historical and current MarkedLil/MobXLil inputs, MobXLil's true
production-min ESM, and jQueryLil's shipped ESM. No library source or artifact is
copied into this repository. At
run time the harness requires a Git repository containing each pinned commit,
verifies the commit and tree, and exports that exact tree into a temporary
directory with `git archive`.
Dirty files, untracked files, and a sibling repository's checked-out branch
therefore cannot enter a measurement.

## Evidence classes

`results/seed.json` deliberately separates:

- `published`: immutable artifacts committed by the pinned sibling projects;
- `comparison`: fresh exact-baseline/exact-checkpoint artifacts or failures;
- `diagnostic`: deliberately non-production probes, which cannot shadow a
  comparison row.

The checkpoint compiler digest `d5e2abee...` was independently rebuilt from an
archive of exact commit `979dc90` and was byte-identical to the earlier captured
executable. The ledger therefore attributes the binary to the exact checkpoint
source (`src/compiler.rs` SHA-256 `607ac880...`), while a regression test ensures
the unrelated captured working-file digest is never used as its source identity.
The checkpoint codec was also built from that archive (digest `b3d93da8...`).

`results/seed-source.mjs` is the reviewed evidence ledger and `seed.json` is its
canonical immutable rendering. A test regenerates the JSON and requires a
byte-for-byte match, including its matrix digest and evidence fingerprint.

Timing is never a size gate. Contended, paused, rounded, or unavailable timing
is recorded explicitly. A timeout, crash, or compile error has no artifact
sizes; the harness never falls back to a pre-existing `dist/` file.

Semantic eligibility is artifact-level. MarkedLil has raw, gzip, and Brotli
objective files, while the pinned SolidLil, MobXLil, and jQueryLil configs are
Brotli-only lanes. Incidental raw/gzip measurements of a Brotli artifact remain
diagnostic and cannot become wins or regressions. The runner executes the full
corpus/API lane separately for every configured artifact, and a metric can be a
win only when both exact artifacts for that objective have fresh `passed`
semantic evidence.

## Validate without building

```sh
node --test comparison/large-libraries/contract.test.mjs
node comparison/large-libraries/run.mjs --check
```

These commands validate the matrix, schema contract, seed observations,
ordering, and evidence fingerprint. They do not inspect siblings, install npm
packages, or compile anything. `--check` is also the default when no mode is
provided.

To hash every pinned compiler and sibling archive without installing or
building anything:

```sh
node comparison/large-libraries/run.mjs --check-inputs
```

To regenerate measurements for already committed sibling artifacts without a
compile or semantic run:

```sh
node comparison/large-libraries/run.mjs --record-existing \
  --output /absolute/path/published-observations.json
```

`--record-existing` labels semantic status `not-run` and compiler provenance
`published-unknown`; it is useful for hash/codec drift checks, not before/after
win claims.

## Run the matrix

The long run is intentionally opt-in:

```sh
node comparison/large-libraries/run.mjs --run --compiler both
```

By default sibling repositories are resolved next to this repository. Override
them without weakening the commit checks:

```sh
SOLIDLIL_REPO=/path/to/solidlil \
MARKEDLIL_REPO=/path/to/markedlil \
MOBXLIL_REPO=/path/to/mobxlil \
JQUERYLIL_REPO=/path/to/jquerylil \
node comparison/large-libraries/run.mjs --run --compiler checkpoint
```

Useful selectors are `--only solidlil,mobxlil-current`, `--compiler baseline`,
`--compiler migration,candidate`, and
`--output /absolute/result.json`. Per-metric policy can be overridden explicitly,
for example `--max-regression raw=0,gzip9=4,brotli11=0`; a tolerated regression
still remains labelled a regression rather than being presented as a win.

Runs are sequential so CPU contention does not turn the comparison into a race.
Each project is installed with its pinned lockfile in the temporary archive via
`npm ci --ignore-scripts`; external dependencies are never copied into this
repository. Before compilation, every configured output is deleted from the
archive, so timeout/error rows cannot inherit committed `dist/` bytes.

The JSON result is durable only when `--output` is supplied (otherwise it is
printed to stdout). Source archives and emitted JavaScript live in an isolated
temporary directory that is deleted after the run. Pass `--keep-temp` to retain
that directory and its build artifacts; the runner prints its exact path. Do
not point `--output` or retained artifacts into this repository when they
contain third-party project output.

The result records source tree, entry, config, compiler binary, codec binary,
and artifact hashes; canonical raw/gzip-9/Brotli-11 sizes; semantic status;
wall/user/system time where the platform reports it; timeout/crash details; and
a deterministic evidence fingerprint. The canonical scorer is the repository's
`lilscript-codec` contract: stock zlib 1.3.1 and Google Brotli 1.1.0.
