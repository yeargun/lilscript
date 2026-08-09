# solidlil Client-Runtime Evidence

The separate
[`lilscript-solid-lab`](https://github.com/yeargun/lilscript-solid-lab)
hosts **solidlil**. The primary todolist lane uses **LSX** (`.lilx`) — LilScript's
JSX-shaped UI syntax — compiled through LilScript reactive + LilScript DOM, with
a Solid-like API (`createSignal`, `For`, `Show`, …). A secondary lane keeps one
shared JSX source and the same `babel-preset-solid` DOM path as SolidJS so the
reactive core can be isolated.

App UI and HTML carry **no framework-identifying strings** (no eyebrow, no
`data-runtime`, identical `<title>`), so size deltas are runtime and compiler
output.

## Current verified scope

- LilScript reactive core + Solid API facade + Solid web DOM module;
- LSX app path (`.lilx` → LilScript modules → Vite);
- adapted runtime behaviors executed through JavaScript, emitted C, and native
  backends;
- unchanged official Solid reference suite remains green upstream;
- Vite todolist builds and core-probe bundles pass fairness gates before size
  publication;
- Solid, LSX, and identical-JSX bundles pass the same interaction-state
  contract. A separate jsdom benchmark is retained as a regression proxy, not
  as a browser-performance claim.

The executable LilScript compatibility slice currently passes 109 adapted
behaviors in each of maximum/disabled optimization × JavaScript/emitted-C/native
(654 executions). The unchanged upstream baseline passes 469/469 tests across
26 files. These counts are separate inventories, not a 109/469 compatibility
percentage: the adapted cases are selected behavior ports and the remaining
Solid surface is not implemented.

Checked snapshot from `lilscript-solid-lab/artifacts/size-report.md`:

### Primary: Solid JSX vs solidlil LSX (full served app)

| Artifact | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Solid todolist | 15,456 | 6,077 | 5,479 |
| solidlil LSX todolist | 10,590 | 4,226 | 3,722 |

solidlil LSX vs Solid: raw −31.5%, gzip −30.5%, brotli −32.1%.

### Secondary: identical JSX + Solid DOM (reactive swap)

| Artifact | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Solid todolist | 15,456 | 6,077 | 5,479 |
| solidlil babel todolist | 13,636 | 5,474 | 4,894 |

DOM/CSSOM: `solid-js/web` vs `solidlil/web` body identical (26,839 normalized
chars). Babel-lane DOM call counts identical.

### Core runtime (minified used Solid API surface)

| Artifact | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Solid core | 8,616 | 3,494 | 3,201 |
| solidlil core | 8,483 | 3,393 | 3,082 |

The 15-sample jsdom interaction median is 1.028× Solid and the median retained
heap across 9 isolated processes is 1.047× Solid. Both are within the lab's 5% material
regression gate; these are Node regression proxies, not browser-performance claims.

These rows are useful implementation evidence, but they are not a claim of full
Solid compatibility. Stores, errors and guaranteed cleanup, promises/resources,
transitions, Suspense, hydration, and SSR remain out of scope.

## Reproduction

```sh
cd ../lilscript-solid-lab
npm ci
npm run setup
npm run build
```

`npm run build` rebuilds the reactive module, Solid/LSX/babel Vite apps,
core-probe bundles, checks fairness gates, and writes `artifacts/size-report.md`.
