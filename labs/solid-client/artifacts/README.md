# Checked-in comparison artifacts

`generated/` contains the exact JavaScript measured by the reports in this
directory:

- `solid-vite.js`: official SolidJS v1.9.13 application bundled by Vite;
- `solid-closure-advanced.js`: that bundle processed by Closure ADVANCED;
- `lilscript-vite.js`: equivalent LilScript application bundled by Vite;
- `lilscript-closure-advanced.js`: that bundle processed by Closure ADVANCED;
- `lilscript-compiler.js`: raw whole-program output from the LilScript compiler.
- `solid-core-open.js`: reusable, differentially verified official Solid ESM surface;
- `solidlil-core-open.js`: the equivalent public SolidLil ESM surface;
- `solidlil-reactive-closed.js`: diagnostic runtime with every export mangled.

`npm run build` regenerates all bundles and size reports. `npm run verify`
executes every artifact against the same interaction contract in Playwright
Chromium. `npm run benchmark` regenerates the randomized, balanced Chromium
CPU, wall-time, and memory report.

`toolchain.json` records the exact Node, compiler, framework source, and npm
package versions used for the current snapshots.

`build-modes.json` and `build-modes.html` record exact public exports, the
cross-runtime behavior digest, open-world size metrics, and closed-world export
mangling. `performance-report.json` includes both interaction timings and
retained-memory evidence from fresh Chromium contexts. It records CDP
main-thread task time, JavaScript and Oilpan heap, DOM/listener counts, and
summed Chromium-process RSS after forced collection.

`api-parity.json`, `api-parity.md`, and `api-parity.html` are the exact public
surface ledger for core, web, and store. Unlike the equivalent-slice build
report, this ledger stays incomplete until all Solid exports have differential
behavior evidence.

`upstream-solid-tests.json` and `upstream-solid-tests.md` record the unchanged
official runtime-suite baseline used as the compatibility denominator.

`lilscript-compat.json` and `lilscript-compat.md` record the adapted LilScript
behavior ports under maximum and disabled optimization pipelines.

These numbers cover the exact client-runtime scope documented in
`../PORT_STATUS.md`. SSR and hydration remain explicitly outside the declared
implementation target even though the full browser-entry compatibility surface
is retained as a separate diagnostic.
