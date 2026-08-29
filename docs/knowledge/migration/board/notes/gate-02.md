# gate-02 — five-fork corpus readiness

Parent: [ledger](../LEDGER.md). Status: active.

## Question

Can the canonical large-library matrix cover MotionLil and MobXLil's true
`production-min` output using immutable source/config inputs and direct compiler
artifacts?

## Current hypothesis

The source/config pinning and first two new lanes are implemented. The historical
checkpoint compiler times out on both current source trees, so fresh before/after
evidence still needs an appropriate newer pinned compiler pair.

## Constraints specific to this task

Do not commit or edit sibling repositories. Do not pin working-tree hashes,
copy dirty source into this repository, treat bundled/Terser output as direct
compiler output, or weaken a missing artifact into a passing row.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-29 | Existing matrix contract | `node --test comparison/large-libraries/contract.test.mjs` | 11 passed, 0 failed | gate |
| 2026-08-29 | Existing immutable evidence | `node comparison/large-libraries/run.mjs --check` | 14 observations, 12 metric rows valid | gate |
| 2026-08-29 | MotionLil isolated current-tree preflight | copy to isolated temp; `MOTIONLIL_LILSCRIPT_BIN=... MOTIONLIL_BUILD_MODE=production npm run build`; `npm test` | 9 passed, 0 failed; diagnostic because source tree is dirty/unpinned | diag |
| 2026-08-29 | MarkedLil isolated current-tree preflight | copy to isolated temp; `LILSCRIPT_COMPILER=... node scripts/build.mjs --compile`; `npm test` | 29 passed, 0 failed; diagnostic because source tree is dirty/unpinned | diag |
| 2026-08-29 | MobXLil isolated current-tree preflight | copy to isolated temp; `LILSCRIPT_COMPILER=... npm run build`; `npm test`; `node scripts/package-smoke.mjs` | 769 passed, 11 skipped; package smoke passed with 78 exports and 11 files; diagnostic because source/config are dirty/unpinned | diag |
| 2026-08-29 | jQueryLil isolated current-tree preflight | copy to isolated temp; `LILSCRIPT_COMPILER=... node scripts/build.mjs --compile`; `npm test` | 6 passed, 0 failed; diagnostic because source tree is dirty/unpinned | diag |
| 2026-08-29 | SolidLil isolated current-tree preflight | copy to isolated temp; production build and `npm test`, then isolated `node --test test/jfb.test.mjs` rerun | 49/50 initially passed with one JFB locator timeout; isolated JFB rerun passed; diagnostic, not one authoritative clean run | diag |
| 2026-08-29 | Pinned matrix inputs | `node comparison/large-libraries/run.mjs --check-inputs` | six boundaries valid: SolidLil, MotionLil direct animate, MarkedLil, MobXLil regular, jQueryLil, and MobXLil production-min | gate |
| 2026-08-29 | New artifact semantic harnesses | `node comparison/large-libraries/semantic/motion-lane.mjs ...`; `node comparison/large-libraries/semantic/mobx-lane.mjs ...` | Motion direct artifact passed with 47 exports; MobX production-min passed with 78 exports and state smoke | gate |
| 2026-08-29 | Historical checkpoint on current MotionLil | `node comparison/large-libraries/run.mjs --run --compiler checkpoint --only motionlil ...` | build exceeded the declared 30-minute timeout; no artifact inherited | gate |
| 2026-08-29 | Historical checkpoint on current MobXLil production-min | `node comparison/large-libraries/run.mjs --run --compiler checkpoint --only mobxlil-production-min ...` | build exceeded the declared 30-minute timeout; no artifact inherited | gate |

## Log

- 2026-08-29 — The five current sibling worktrees compile and their maintained behavior checks pass in isolated diagnostics. Matrix promotion is blocked: MotionLil's expanded entries/build are modified or untracked at `dcbc09d`, and MobXLil's `config/production.min.toml` plus supporting build changes are untracked/modified at `e14a5a0`. Dirty files cannot become canonical evidence. — **OPEN**
- 2026-08-29 — Pinned MotionLil `fde1aed` and MobXLil `820c9a8`; added direct Motion animate and true MobX production-min matrix lanes plus artifact semantic checks. Matrix/schema/hash gates pass. Both historical checkpoint builds time out cleanly, so successor compiler pins and remaining Motion boundaries are still open. — **OPEN**

## Next step

Pin a newer legal before/current LilScript compiler pair, add the remaining
Motion direct-output boundaries, and run the first complete five-fork G2
checkpoint without timeout or stale artifacts.
