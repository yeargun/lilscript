# recover-04 — jQuery compiler incumbent

Parent: [ledger](../LEDGER.md). Status: landed.

## Question

Did the migration regress jQuery's selected compiler artifact before work on its
separate competitive gap?

## Current hypothesis

No. The exact `06b89aa` and current candidate artifacts are byte-identical. The
remaining loss to official jQuery/Terser belongs to `jquery-01`, not incumbent
recovery.

## Constraints specific to this task

Do not turn an unchanged compiler result into a competitive win. Package and
post-minified comparisons remain separate.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-29 | Exact migration pair | gate-02 `migration,candidate` run | both fresh semantic artifacts pass at Brotli-11 30,275; tie | gate |

## Log

- 2026-08-29 — No legal compiler incumbent was lost. The older competitive jQuery gap remains active under `jquery-01` and does not block phase-1 recovery. — **LANDED**

## Next step

Pursue the independent JavaScript-baseline gap only through `jquery-01` after
phase-2 consolidation.
