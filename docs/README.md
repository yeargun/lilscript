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
3. **[future-architecture.md](future-architecture.md) is the architecture** of the
   compiler and the size-relevant language design.
   [Current architecture](knowledge/compilation/current-architecture.md) describes
   today's code against it, without overriding the source.
4. **[migration/index.md](migration/index.md) is the plan**: every numbered step and
   its progress. There is no other plan, board or packet.
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
| Syntax or semantics | [Language v0.1](language-v0.1.md) |
| TOML behavior | [Configuration](configuration.md) |
| Why a design choice exists | [Design decisions](knowledge/decisions/README.md) |
| How the compiler works now | [Current architecture](knowledge/compilation/current-architecture.md) |
| The compiler's architecture and the language designed for size | [Future architecture](future-architecture.md) |
| Implement a bounded migration step or check progress | [Single migration plan](migration/index.md) |
| Check a compiler binary: the case runner, the port runner and their expected-failure ledgers | [Testing](testing.md) |
| Whether a size claim is valid | [Verification](knowledge/verification/README.md) → [evidence](knowledge/evidence/README.md) |
| How the deleted route did something | [History](knowledge/history/README.md) |
| Full linked map | [Knowledge tree](knowledge/README.md) |

## Normative Contracts

| Contract | Owns |
|---|---|
| [language-v0.1.md](language-v0.1.md) | Syntax, types, evaluation, target behavior |
| [configuration.md](configuration.md) | `lilscript.toml` schema and defaults |
| [modules-and-delivery.md](modules-and-delivery.md) | Imports, chunks, lockfiles, Lilpack |
| [web-platform.md](web-platform.md) | Host and `extern` boundary |
| [differential-testing.md](differential-testing.md) | Independent semantic oracle |

Do not create separate migration packets or boards. Add bounded work under its
phase in the single plan. The architecture owns the target; the plan owns how to
reach it and how replacement is verified.
