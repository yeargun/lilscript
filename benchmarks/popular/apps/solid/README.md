# Solid / solidlil LSX

Measured in the integrated `labs/solid-client` Git submodule (formerly the
sibling `lilscript-solid-lab` repository), not compiled inside this
popular-corpus runner.

## Fair primary lane

| Side | Pipeline |
| --- | --- |
| Solid | shared todolist JSX → babel-preset-solid → solid-js + solid-js/web |
| solidlil | `main.lilx` → LilScript reactive + LilScript DOM |

Same todo interaction contract, no framework-identifying UI strings. Size is the
full served app JS after Vite minify.

## Numbers

`run.mjs` uses the vendored `size-report.json` in this directory first so the
row remains reproducible without fetching submodules. It can also read
`labs/solid-client/artifacts/size-report.json`, then a legacy sibling checkout.
If none exists, the Solid row is skipped with a console note.

Refresh the snapshot after rebuilding the lab:

```sh
cp ../../../../labs/solid-client/artifacts/size-report.json ./size-report.json
```

Primary brotli comparison (LSX): Solid 5479 vs solidlil 3722 (−32.1%).
