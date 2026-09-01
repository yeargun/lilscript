# Brief — recover-01

For a subagent. Written before spawning. The agent reads
[mission](../../../mission.md), this brief, and [notes/recover-01.md](../notes/recover-01.md) —
nothing else unless this brief names it.

## Task

Read-only attribution of the MobX production-min size change between LilScript
`2d2268` and `06b89aa`. Identify the earliest generic option, recipe, or scheduling
change likely to explain the same-source/config Brotli increase from the committed
15,083-byte artifact toward 15,491 bytes. Return concrete source locations and the
smallest ablation to confirm it.

## Why this matters to the objective

Phase 1 must recover a legal incumbent before new optimizer infrastructure. An
existing registry option is preferable to a new pass or a package-specific fix.

## Read

- `docs/knowledge/mission.md`
- `docs/knowledge/migration/board/notes/recover-01.md`
- `src/compiler.rs`, `src/config.rs`, `src/decision_registry.rs`,
  `src/codegen_ir_js.rs`, `src/optimizer.rs`
- Git diff `2d2268..06b89aa` for those files
- `/home/azureuser/mobxlil/config/production.min.toml`

## May touch

- Nothing; this is a read-only attribution.

## Must not

- No package matcher, unsafe getter assumption, wider budget as a fix, or
  aggregate result that hides regular MobX.
- Do not assume the pre-`1c59b75` synthesized min artifact is the same boundary.

## Prove it

```sh
git diff 2d2268..06b89aa -- src/compiler.rs src/config.rs src/decision_registry.rs src/codegen_ir_js.rs src/optimizer.rs
```

Expected: one ranked shortlist with exact generic ablations and source references.

## Report

Do not edit files. Return at most 20 lines with the likely first divergent
decision, alternatives considered, and one minimal confirmation command.
