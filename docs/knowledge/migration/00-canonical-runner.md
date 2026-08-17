# Phase 00 — canonical folder runner

Parent: [migration](README.md). Layout:
[case layout](../verification/case-layout.md). Runner:
[`comparison/cases/README.md`](../../../comparison/cases/README.md).

## Objective

Make a case a directory a human can open, not only a generated string in
`catalog.mjs`. The daily command is:

```sh
nvm use
node comparison/cases/run.mjs --canonical-only
node comparison/cases/run.mjs --only aggregates/
```

Catalog cases still run in `node comparison/cases/run.mjs` (full suite) and in
release check.

## Folder

```text
comparison/cases/canonical/<family>/<id>/
  case.toml      # expect = "le" | "lt"
  main.lil
  main.js
  README.md      # required for lt and for any non-obvious contract
```

The runner compiles `main.lil` with the three gold configs, minifies `main.js` with
Terser, Oxc, and esbuild, executes every artifact, and gates each LilScript
objective against the metric-specific JS minimum.

## Exit

- `--canonical-only` discovers every `case.toml` under `canonical/`.
- `--only` matches catalog names and canonical paths.
- A missing or drifting `expected.txt` is not required; stdout comes from the
  original JavaScript, same as the catalog.
- Artifact paths replace `/` so build outputs stay flat.
