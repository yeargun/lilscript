# `[lint]` and `[format]`

Parent: [Config](README.md). Schema: [`docs/configuration.md`](../../configuration.md). Delivery: [progressive enhancement](../delivery/progressive-enhancement.md).

These do not change emitted JS. They change what authors are allowed to ship, which **does** affect size when rules forbid eager host work or allocations that survive optimization.

## Lint

`lilscript-lint` runs module-aware semantic checks and inspects **optimized** IR (so DCE’d allocations do not warn).

| Key | Default | Meaning |
|---|---|---|
| `enabled` | true | CLI/LSP |
| `preset` | `recommended` | `minimal` errors only; `recommended` adds effect/perf warnings and size hints; `strict` promotes effects to errors and size to warnings |
| `deny_warnings` | false | CI gate; also `--deny-warnings` |
| `providers` | all built-ins if unset | exact namespaces: `correctness`, `effects`, `performance`, `size`, `web` |
| `exclude` | `[]` | globs |
| `pure_extern_allowlist` | `[]` | trusted `pure extern` names |
| `rules` | `{}` | `"namespace/id" = "off\|hint\|warn\|error"` |

`web/eager-host-access` flags top-level host work before a progressive-enhancement boundary. Embedders can add in-process `LintRuleProvider`s (`lint_path_with_providers`); not a dynamic plugin ABI.

Suppressions: `// lilscript-lint-disable RULE` and `-next-line`. Machine fix today: remove unreachable expression statements.

## Format

`lilscript-fmt`: deterministic, comment-preserving, optional import organize. `format.enabled = false` disables CLI/LSP format; `--force` overrides. `line_width` ≥ 40; `newline` `lf` \| `crlf`.
