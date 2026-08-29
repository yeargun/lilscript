# Brief — gate-04

For a subagent. Written before spawning. The agent reads
[mission](../../../mission.md), this brief, and [notes/gate-04.md](../notes/gate-04.md) —
nothing else unless this brief names it.

## Task

Audit production JavaScript codec calls and design the smallest fail-closed
admission primitive that guarantees generated syntax and binding validation
before scoring. Identify what existing data can and cannot yet prove for
property categories, module links, ABI, and lowering obligations. Do not edit.

## Why this matters to the objective

Invalid candidates must never win an exact size objective. Central admission
also prevents later target-JS migration from inheriting scattered checks.

## Read

- `docs/knowledge/mission.md`
- `docs/knowledge/migration/board/notes/gate-04.md`
- `src/compiler.rs` lines 4000-6000 and 6900-8650
- `src/js_peephole/mod.rs` lines 290-335 and 900-985
- `src/compilation_contract.rs`
- `src/ir.rs` lowering-obligation helpers

## May touch

- Nothing; this is a read-only design audit.

## Must not

- The [standing refusals](../README.md#standing-refusals).
- Do not treat final-byte parsing as proof of source identity or general behavior.
- Do not propose package-specific validation or silently skip the incumbent.

## Prove it

```sh
cargo test --release --lib generated_javascript
```

Expected: return a concrete call-site inventory, a minimal API shape, and named
tests that prove invalid candidates cannot increment the codec-call counter.

## Report

Do not edit files. Return at most 20 lines: call-site inventory, proposed change,
known proof gaps, and the single next step.
