# recover-02 — Motion direct-boundary incumbents

Parent: [ledger](../LEDGER.md). Status: landed.

## Question

Do any pinned direct Motion boundaries regress against the first reproducible
legal compiler incumbent, and if so which generic decision first caused it?

## Current hypothesis

The provisional losses mixed package-bundled measurements with direct compiler
output. On exact direct boundaries, four tie and two improve; only animateMini
lost five Brotli bytes through a bounded naming miss, now recovered generically.

## Constraints specific to this task

Use only direct compiler output from immutable Motion source entries. Do not
label esbuild/Terser package output as a compiler artifact or aggregate one
boundary's win against another boundary's loss.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-29 | Current migration checkpoint | gate-02 tracked `migration,candidate` run | all seven direct Motion candidate cells passed semantics and tied `06b89aa` | gate |
| 2026-08-29 | Exact `2d2268` direct boundaries | compile pinned Motion entries with exact `2d2268`; canonical codec | Brotli: animateMini 2,325; animate 24,232; animate+stagger 24,389; lab 31,042; export 33,405; mini 11,052; full 51,009 | gate |
| 2026-08-29 | Current direct boundaries before recovery | gate-02/current direct artifacts | Brotli: animateMini 2,330; animate 24,232; animate+stagger 24,389; lab 31,042; export 33,405; mini 10,459; full 50,940 | gate |
| 2026-08-29 | Naming recovery | template-aware one-byte binding test; contextual replacement ordering test; compile direct animateMini; canonical codec | animateMini raw unchanged at 7,315; gzip 2,563; Brotli 2,324, one byte below exact `2d2268` | gate |
| 2026-08-29 | Recovered artifact semantics | `node comparison/large-libraries/semantic/motion-lane.mjs .../.__compiled-animate-mini.mjs` | passed with one exported boundary | gate |

## Log

- 2026-08-29 — Started after MobX production-min was classified as an unmatched historical comparison rather than a legal regression. — **OPEN**
- 2026-08-29 — Direct evidence refuted the old package-level loss: animate/stagger/lab/export tie and mini/full improve. animateMini differed only by one binding spelling (`v` versus `J`). — **OPEN**
- 2026-08-29 — Replaced artifact-wide template refusal with expression-aware identifier protection and reserved 28 final probes for small-artifact unused-name recovery. The old `v` spelling became reachable and wins by one Brotli byte. — **LANDED**

## Next step

Keep the seven direct boundaries separate and retain the template-aware naming
regression in focused tests.
