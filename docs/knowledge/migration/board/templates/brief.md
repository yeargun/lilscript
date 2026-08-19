# Brief — {{id}}

For a subagent. Written before spawning. The agent reads
[mission](../../../mission.md), this brief, and [notes/{{id}}.md](../notes/{{id}}.md) —
nothing else unless this brief names it.

## Task

<One paragraph. What to do and what "done" looks like.>

## Why this matters to the objective

<Two sentences tying it to: smallest correct artifact under the configured codec,
via typed web syntax and direct JS. If this cannot be written, the task is not ready.>

## Read

- `docs/knowledge/mission.md`
- `docs/knowledge/migration/board/notes/{{id}}.md` — including every REJECTED line
- <specific source files, with line numbers where known>

## May touch

- <exact paths. Everything else is read-only for this agent.>

## Must not

- The [standing refusals](../README.md#standing-refusals) — no glue, no post-minify,
  no weakened gate, no `diag` number in a claim, semantics before size.
- <task-specific refusals: approaches already REJECTED in the note, files to leave alone.>

## Prove it

```sh
<exact command(s)>
```

Expected: <what a pass looks like, in numbers or exit codes.>

## Report

Append to `docs/knowledge/migration/board/notes/{{id}}.md`:
one Evidence row per command run (with the `gate`/`diag` tag), and one Log line ending
in OPEN, LANDED, or REJECTED. Then return at most 20 lines: what you changed, what the
gate said, and the single next step. Do not edit `LEDGER.md`.
