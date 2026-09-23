# jquerylil differential harnesses

Each harness loads official jquery@3.7.1 (from `~/jquerylil/node_modules`) and a built `jquery.esm.js` into jsdom and compares their behaviour case by case. They were written by the module-group rewrites of 2026-09-23 (013 batch 24), one per group. Run them with Node 24 against a build of the port:

| Group | Command | Cases |
|---|---|---|
| ajax | `node ajax/harness.mjs <esm> --json out.json` | 43 scenarios |
| core | `node core/harness.mjs <esm> out.json` | 187 cases |
| css | `node css/diff.mjs <esm> out.json` | 1,086 cases |
| dom | `node dom/diff.mjs <esm> out.json` | 5,630 cases |
| effects | `node effects/diff.mjs <esm> [--show]` | 183 cases |
| event | `node event/diff.mjs <base-esm> <variant-esm>` | 291 scenarios |

A result counts as a regression only when a case that matched official on the previous build fails on the new one.
