# Solid Client-Runtime Evidence

The separate
[`lilscript-solid-lab`](https://github.com/yeargun/lilscript-solid-lab)
measures a real LilScript client runtime against pinned, unchanged
`solid-js@1.9.13`. It is a behavioral port, not a TypeScript compatibility
layer and not a source-to-source rewrite.

## Current verified scope

- 2,355 lines of LilScript across reactive, flow, component, web, and app
  modules;
- 109 adapted runtime behaviors, each executed with maximum and disabled
  optional optimization through JavaScript, emitted C, and native backends;
- 654 successful LilScript compiler/runtime executions;
- 469/469 unchanged official Solid reference tests across 26 files;
- optimized Vite, Closure ADVANCED, and direct LilScript compiler artifacts all
  pass the interactive client contract before measurement.

The checked size snapshot is:

| Client artifact | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| Official Solid + Vite | 12,487 | 4,922 | 4,451 |
| Official Solid + Closure ADVANCED | 11,237 | 4,804 | 4,307 |
| Partial LilScript runtime + Vite | 7,909 | 3,245 | 2,901 |
| Partial LilScript runtime + Closure ADVANCED | 7,523 | 3,199 | 2,850 |
| Partial LilScript runtime + compiler | 5,609 | 2,265 | 2,001 |

These rows are useful implementation evidence, but they are not a compatible
library comparison yet. Only 109 of the 469 target behaviors have LilScript
ports. Stores, errors and guaranteed cleanup, promises/resources, transitions,
time-sliced scheduling, complete DOM insertion/reconciliation, Suspense,
hydration, and SSR remain out of scope.

## Reproduction

With this repository and `lilscript-solid-lab` as sibling directories:

```sh
cd ../lilscript-solid-lab
npm ci
npm run setup
npm run check
```

`npm run check` lints, runs the adapted and official suites, builds all five
artifacts, verifies behavior, and regenerates size and runtime reports. A size
or speed result is not publishable when any behavior gate fails.
