# LilScript documentation

Read from general intent to specific implementation. Do not load history or
research when a contract or current-architecture page answers the question.

There is one compiler. The route that preceded it was deleted on 2026-09-23
(plan phase M1); the pages that describe it are in
[knowledge/history](knowledge/history/README.md) and never describe current
behavior.

## Authority

1. **Language and configuration contracts** define supported interfaces.
2. **Source and tests** define implemented behavior.
3. **[migration/plan.md](migration/plan.md) is the active architecture and plan**:
   the objective, TOML contract, mechanisms, dependencies and acceptance rules.
   [Current architecture](knowledge/compilation/current-architecture.md) maps
   the implementation without overriding source and tests.
4. **[migration/index.md](migration/index.md) is progress**.
   [migration/history.md](migration/history.md) records new batches;
   [coverage.md](migration/coverage.md) assigns unfinished legacy work.
   Previous plans and evidence are preserved in [old-history](old-history/README.md)
   and do not prescribe current work.
5. **[testing.md](testing.md) is the verification tools**: the case runner, the port
   runner and their expected-failure ledgers. Tracked generated reports define
   numerical evidence.
6. **[Current status](current-status.md)** describes the checkout.
7. **History, research, journals and landed notes** are historical evidence only.

If two pages disagree, use the higher authority and fix the lower one.

## Start Here

| Need | Read |
|---|---|
| Product intent and non-goals | [Why LilScript](../why-lilscript.md) → [mission](knowledge/mission.md) |
| What exists and what is green | [Current status](current-status.md) |
| The 2026-09-24 release: sizes against the last release and against Terser, Oxc/Rolldown and esbuild, plus compile times | [Release report](reports/2026-09-24-release.md) |
| Syntax or semantics the compiler accepts today | [Language v0.1](language-v0.1.md) |
| The target language contract (version 1: rules R1–R18, each clause tagged with its task and status) | [Language v1](language.md) |
| TOML behavior | [Configuration](configuration.md) |
| Why a design choice exists | [Design decisions](knowledge/decisions/README.md) |
| How the compiler works now | [Current architecture](knowledge/compilation/current-architecture.md) |
| Architecture, independent objectives, configurable behavior, mangling and bounded search | [Active design and plan](migration/plan.md) |
| Where the migration stands: every task done or open, and what is next | [Migration progress](migration/index.md) |
| Implement a migration task: what it is, its rules and its order | [Migration plan](migration/plan.md) |
| What a new migration batch landed, measured and left open | [Migration history](migration/history.md) |
| Previous plans, batches through Y5 and their unfinished-task descriptions | [Old history](old-history/README.md) |
| Check a compiler binary: the case runner, the port runner and their expected-failure ledgers | [Testing](testing.md) |
| Whether a size claim is valid | [Verification](knowledge/verification/README.md) → [evidence](knowledge/evidence/README.md) |
| How the deleted route did something | [History](knowledge/history/README.md) |
| Full linked map | [Knowledge tree](knowledge/README.md) |

## Normative Contracts

| Contract | Owns |
|---|---|
| [language-v0.1.md](language-v0.1.md) | Syntax, types, evaluation, target behavior, as the compiler implements them today |
| [language.md](language.md) | The target language contract (version 1); a clause governs once its task lands |
| [configuration.md](configuration.md) | `lilscript.toml` schema and defaults |
| [modules-and-delivery.md](modules-and-delivery.md) | Imports, chunks, lockfiles, Lilpack |
| [web-platform.md](web-platform.md) | Host and `extern` boundary |
| [differential-testing.md](differential-testing.md) | Independent semantic oracle |

Keep bounded work in the active plan, progress in its checklist and evidence in
its history. The coverage map preserves ownership without creating another
execution order. Update these documents together when a batch closes work.
