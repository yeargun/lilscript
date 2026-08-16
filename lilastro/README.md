# Lilastro

Motion **JS / DOM** lab comparing npm `motion@13.0.0` vs the LilScript port under
`benchmarks/popular/ports/motion`.

Upstream: [motiondivision/motion](https://github.com/motiondivision/motion) `v13.0.0`.

## What it measures

1. **Build-mode contracts** — reusable open-world API versus fully linked,
   mangled closed-world application (`npm run verify:modes`).
2. **Bundle size** — Vite 8 tree-shaken retained API surfaces (`npm run measure`).
3. **Playwright CSSOM correctness** — upstream-style `animate-play` / `animate-css-vars`.
4. **Statistical DOM/CSSOM/memory perf** — paired lil vs npm rounds with:
   - discarded warmup
   - **randomized lane order** each round
   - separate **cold** (fresh navigation) and **warm** (reused page + `__runPerfSample`) phases
   - mean / median / p95
   - paired-bootstrap **95% upper confidence** ratios from `benchmarks/statistics.mjs`
   - CDP `JSHeapUsedSize` when available

React / Vue Motion entrypoints are out of scope.

## Commands

```bash
cd lilastro
npm install
cd .. && cargo build --release --bin lilscript --bin lilscript-codec && cd lilastro
npm run verify:modes     # public ABI + closed-app behavior + size gates
npm run measure          # Vite size lab
npm run playwright       # CSSOM correctness + statistical warm/cold perf
npm run report           # measure + playwright + write ../report-motion-finer.html
npm run build:browser-fixtures # build and attest every npm/Lil browser lane
npm --prefix ../benchmarks/popular run publish:motion-lab # publish attested lanes
```

## Open world and closed world

The mode is an ABI decision, not another minifier label:

| Mode         | Compiler target                 | Public contract                                                               | Config                     |
| ------------ | ------------------------------- | ----------------------------------------------------------------------------- | -------------------------- |
| Open world   | `js-module`                     | ESM names and fields reachable through exported aggregates stay stable        | `config/open-world.toml`   |
| Closed world | `js` after linking the consumer | LilScript-owned identifiers, properties, and exports may be renamed or erased | `config/closed-world.toml` |

`npm run verify:modes` builds the same nine-function Motion core surface in
open-world npm and LilScript lanes, imports both output modules, checks the
complete export list and matching behavior, and requires LilScript to be
no larger in Brotli-11 bytes because `config/open-world.toml` selects the
Brotli objective. It separately builds the
`values-core` consumer as a closed application, checks identical output, and
applies the same matching Brotli-objective gate. Raw and gzip-9 sizes remain
visible diagnostics and may trade off; they would require separately compiled
raw- and gzip-selected artifacts to become gates. A diagnostic closed-module build also proves
that all selected public export names are mangled. Results and an accessible
HTML report are written to `build/modes/`.

All byte reports use the repository's `lilscript-codec` scorer: statically
bundled upstream zlib C 1.3.1 and official Google Brotli C 1.1.0. They never
fall back to Node's platform codec builds. Size and build-mode reports resolve
the compiler and config to their actual paths, require an explicit
`[javascript] cost_model = "brotli"`, and record compiler/config SHA-256
identities. The combined report refuses to mix build-mode and measurement
evidence produced by different compiler or scorer binaries.

Browser fixtures, the interactive lab, and `npm run measure` all default to the
closed-world config. `LILSCRIPT_CONFIG=/absolute/path/to/config.toml` remains an
explicit research override and is recorded in generated reports.

Env knobs (perf) — same statistical core as `benchmarks/statistics.mjs`:

| Variable                          | Default      | Meaning                                      |
| --------------------------------- | ------------ | -------------------------------------------- |
| `LILSCRIPT_STATISTICAL_SAMPLES`   | `401` (≥201) | Paired samples kept after warmup             |
| `LILSCRIPT_MOTION_PERF_WARMUP`    | `8`          | Discarded rounds before keep                 |
| `LILSCRIPT_MOTION_PERF_MAX_RATIO` | `1.15`       | Non-inferiority upper budget on median & p95 |
| `LILSCRIPT_MOTION_PERF_WARM`      | `1`          | Set `0` to skip warm                         |
| `LILSCRIPT_MOTION_PERF_COLD`      | `1`          | Set `0` to skip cold                         |

Each perf round randomizes lil-vs-npm order. Warm reuses pages; cold navigates fresh every sample. The workload schedules a 192-element, two-property WAAPI stagger. Metrics: sync `scheduleMs`, rAF `frameMean`/`frameP95`, and CDP `heapUsed` after an explicit browser GC.

Report: `../report-motion-finer.html`.
