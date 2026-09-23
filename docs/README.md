# LilScript documentation

Read from general intent to specific implementation. Do not load migration notes
or research when a contract or current-architecture page answers the question.

## Authority

1. **Language and configuration contracts** define supported interfaces.
2. **Source and tests** define implemented behavior.
3. **Current architecture** explains that implementation without overriding it.
4. **Tracked generated reports** define numerical evidence.
5. **Current status** describes the checkout; [future architecture](future-architecture.md) is the architecture of the one compiler and the size-relevant language design.
6. **Migration plan and progress** live only in [migration/index.md](migration/index.md), including all numbered steps. Retired plans are archived outside the repository. A proposal or checkbox without verified evidence does not establish implementation.
7. **Research, journals, and landed notes** are historical evidence only.

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
| Whether a size claim is valid | [Verification](knowledge/verification/README.md) → [evidence](knowledge/evidence/README.md) |
| Full linked map | [Knowledge tree](knowledge/README.md) |

## Normative Contracts

| Contract | Owns |
|---|---|
| [language-v0.1.md](language-v0.1.md) | Syntax, types, evaluation, target behavior |
| [configuration.md](configuration.md) | `lilscript.toml` schema and defaults |
| [modules-and-delivery.md](modules-and-delivery.md) | Imports, chunks, lockfiles, Lilpack |
| [web-platform.md](web-platform.md) | Host and `extern` boundary |
| [differential-testing.md](differential-testing.md) | Independent semantic oracle |

There is one compiler. The route that existed before 2026-09-18 is being deleted
(plan phase M1); pages that still describe it are history until M1.7 rewrites them.

Do not create separate migration packets or boards. Add bounded work under its
phase in the single plan. The architecture owns the target; the plan owns how to
reach it and how replacement is verified.
