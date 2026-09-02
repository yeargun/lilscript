# LilScript documentation

Read from general intent to specific implementation. Do not load migration notes
or research when a contract or current-architecture page answers the question.

## Authority

1. **Language and configuration contracts** define supported interfaces.
2. **Source and tests** define implemented behavior.
3. **Current architecture** explains that implementation without overriding it.
4. **Tracked generated reports** define numerical evidence.
5. **Current status and ledger** define live work.
6. **Planned architecture and migration** define intended changes.
7. **Research, journals, and landed notes** are historical evidence only.

If two pages disagree, use the higher authority and fix the lower one.

## Start Here

| Need | Read |
|---|---|
| Product intent and non-goals | [Why LilScript](../why-lilscript.md) → [mission](knowledge/mission.md) |
| What exists and what is green | [Current status](current-status.md) |
| Syntax or semantics | [Language v0.1](language-v0.1.md) |
| TOML behavior | [Configuration](configuration.md) |
| Debug optimized/mangled JavaScript | [JavaScript source maps](source-maps.md) |
| Why a design choice exists | [Design decisions](knowledge/decisions/README.md) |
| How the compiler works now | [Current architecture](knowledge/compilation/current-architecture.md) |
| Where the architecture is going | [Planned architecture](knowledge/compilation/planned-architecture.md) |
| Visual explanation of future compression work | [Future direction](future-direction.html) |
| How to execute the change | [Planned migration](knowledge/migration/planned-migration.md) |
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

`optimization-coverage.md` and `roadmap.md` are descriptive indexes. They do not
override source, tests, current status, or the active migration plan.
