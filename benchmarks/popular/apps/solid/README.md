# Solid / solidlil LSX

This directory currently vendors a historical measurement from the sibling
`lilscript-solid-lab` worktree. The active integrated `labs/solid-client`
checkout does not yet contain the `.lilx` pipeline, so this row is snapshot
evidence—not a result rebuilt by the current single-repository gate.

## Fair primary lane

| Side | Pipeline |
| --- | --- |
| Solid | shared todolist JSX → babel-preset-solid → solid-js + solid-js/web |
| solidlil | `main.lilx` → LilScript reactive + LilScript DOM |

Same todo interaction contract, no framework-identifying UI strings. Size is the
full served app JS after Vite minify.

## Numbers

`benchmarks/popular/run.mjs` uses the vendored `size-report.json` first so the
historical row is stable without fetching another checkout. A fresh LSX result
must not replace it until the integrated parser/Vite path also owns the application, behavior
checks, and performance/memory gates all live under `labs/solid-client`.

Do not refresh this snapshot from the current runtime-only lab: its artifact
schema and tested surface are different.

Primary brotli comparison (LSX): Solid 5479 vs solidlil 3722 (−32.1%).
