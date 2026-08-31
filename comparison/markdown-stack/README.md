# React Markdown stack comparison harness

This directory is the durable source, package, test, and size contract for the
16 sibling LilScript ports used by the react-markdown stack.

`manifest.json` pins every official repository, release tag, commit, tree,
package version, all sibling package scripts and exports, evidence totals,
source roots/mappings, public measurement entries, and diagnostic artifacts.
The harness treats upstream clones and sibling worktrees as read-only inputs.
It records port Git state and rejects dirty upstream clones.

## Fast checks

Install the exact harness-only tools and run checks that need no sibling or
network access:

```sh
cd comparison/markdown-stack
npm ci --ignore-scripts
npm run check
```

The lockfile pins esbuild 0.28.1, Terser 5.51.2, all 16 official root packages,
and their complete transitive graphs.

## Input audit

Clone the pinned upstreams once, then verify all upstream and sibling inputs:

```sh
node comparison/markdown-stack/run.mjs --clone-upstreams
node comparison/markdown-stack/run.mjs --check-inputs \
  --json comparison/markdown-stack/.work/input-audit.json
```

An existing clone is never overwritten. The audit checks origin, tag, commit,
tree, clean state, package names and versions, exact port exports, lockfiles,
evidence, every configured upstream runtime mapping, and a hash inventory of
every runtime source under each declared sibling source root. Use
`--upstream-root` and sibling repository environment variables to override
locations.

Site evidence and full package runs are separate contracts. The 16
`site/results.json` records cover 5,595 passing tests/subtests. `npm test` covers
5,731: the manifest records each complete output summary, including KaTeX's 17
Node tests plus 1,230 Jest tests. Missing, duplicate, unknown, inconsistent, or
partially matching summaries fail closed.

## Measurements

```sh
cargo build --release --bin lilscript-codec
node comparison/markdown-stack/run.mjs --measure \
  --json comparison/markdown-stack/.work/measurements.json \
  --markdown comparison/markdown-stack/REPORT.md
```

There is one selectable lane per package. esbuild receives the exact official
public root entry directly, which preserves every public root export, bundles
the full runtime dependency graph, and emits ESM. The equivalent Lil lane uses
the standard public ESM export. A standalone Lil ESM is copied byte-for-byte;
`remark`, `unified`, and `react-markdown` are bundled because their public ESM
has runtime imports. No Lil lane receives post-minification. The official graph
then receives Terser 5.51.2 with exactly `module: true`, `compress: true`, and
`mangle: true`.

Neutral package resolution is used except for Unified and React Markdown, whose
published browser graphs select their browser VFile implementation. React and
`react/*` are external only for React Markdown and are external on both sides.
All CJS, UMD, alternate subpath, site, and closed artifacts are diagnostics;
none can be selected for a result.

Every graph, generated baseline, and diagnostic artifact is measured by
`lilscript-codec`: stock zlib 1.3.1 gzip level 9 (`mtime=0`) and official Google
Brotli 1.1.0 quality 11, window 22, generic mode. The JSON report records every
artifact and graph-input SHA-256, package-lock and harness hashes, tool versions,
and reproduction commands.

Historical raw, gzip, and Brotli values in site evidence are checked. Exact
matches are counted; every difference requires an explanation in the generated
report. Results always compare the single standard Lil lane with the single
official Terser lane and include raw, gzip, Brotli, aggregate bytes, and
win/loss counts.

The contradictory Micromark baselines are resolved explicitly. The canonical
neutral graph reproduces the retained `site/official.js` byte-for-byte and is
about 22.7 KB after Brotli. The roughly 13.0 KB number is reproducible only with
the browser condition, where `decode-named-character-reference/index.dom.js`
uses the host DOM instead of bundling `character-entities`. That narrower graph
is emitted and hashed as a diagnostic but is never selected.

Generated lanes default to `.work/`; use `--work-dir` to override that location.

## Full tests

```sh
node comparison/markdown-stack/run.mjs --run-tests \
  --json comparison/markdown-stack/.work/tests.json
```

This runs each exact `npm test` script and validates every declared summary.
The exact scripts for `remark-gfm`, `remark-breaks`, and `remark-math` begin with
`npm run build`, so a no-write audit must exclude those three with `--only` and
run their type/test tails separately. `--only id,id` works with all
input-dependent modes.

## Limitations

The upstream mapping audit covers runtime modules owned by each pinned upstream
package, while the generated graph metadata records every transitive runtime
input. Historical site rows do not bind their values to artifact or tool hashes;
retained site artifacts are therefore diagnostics, and all differences from the
fully pinned canonical lane are disclosed rather than silently treated as the
same baseline.
