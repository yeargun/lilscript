# recover-03 — Marked selected-metric incumbent

Parent: [ledger](../LEDGER.md). Status: landed.

## Question

Does Marked's pinned Brotli package artifact regress against a reproducible legal
compiler incumbent, or is the provisional +38-byte row another unmatched build?

## Current hypothesis

The current Brotli lane is stable at 9,506 bytes across `06b89aa` and the V-01
candidate. Every committed historical shipped ESM is larger, so no selected-
metric incumbent regression exists.

## Constraints specific to this task

Selected Brotli bytes govern. Raw/gzip diagnostics cannot offset a Brotli loss.
Do not reuse stale `dist`, post-minify output, or accept the old raw/gzip artifacts
that fail the 660-case semantic harness.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-29 | Current exact pair | gate-02 `migration,candidate` run | Brotli lane 9,506 on both exact compilers; current candidate passes 660 corpus cases | gate |
| 2026-08-29 | Committed artifact audit | canonical codec over `3a540ac`, `31acd0d`, `7ed4f7d`, and current `dist/marked.esm.js` | Brotli 9,589 / 9,515 / 9,517 / 9,517; every committed artifact is larger than current exact 9,506 | gate |

## Log

- 2026-08-29 — Started after all seven direct Motion boundaries reached or beat exact `2d2268`. — **OPEN**
- 2026-08-29 — The provisional +38 row is not reproduced by any committed selected artifact. Current exact output is nine bytes smaller than the best historical committed artifact and passes all 660 cases. — **LANDED**

## Next step

Keep raw/gzip semantic regressions ineligible and preserve 9,506 as the selected
Brotli incumbent.
