# Migration board — how work survives a lost context

Parent: [migration](../README.md). Mission: [mission](../../mission.md).
Measurement meaning: [verification](../../verification/README.md).

The [planned migration](../planned-migration.md) says what order. This board says
where work currently is, what has already been tried, and what a new agent must read.
It exists so a session that loses its context resumes from three files instead of
re-deriving the compiler. Architecture sequence:
[planned migration](../planned-migration.md).

The board is authoritative for status. If a status here disagrees with a claim in
prose, chat, or a summary, the board is what gets fixed — or the board is what was
wrong, and it gets corrected in the same edit that discovers it.

## Read order — the context budget

1. This file.
2. [LEDGER.md](LEDGER.md) — every task, one line each.
3. `notes/<id>.md` for **the one task you are doing**.
4. For architecture work, read
   [planned-architecture.md](../../compilation/planned-architecture.md) and the
   relevant phase in [planned-migration.md](../planned-migration.md).

Stop there. [JOURNAL.md](JOURNAL.md) and the other notes are cold storage. Open a
second note only when you are about to redo something that may already have a
verdict — that is what the `REJECTED` entries are for.

## Files

| File | Holds | Written by |
|---|---|---|
| [LEDGER.md](LEDGER.md) | Task id, lane, status, gate, one-line intent | Orchestrator only |
| [JOURNAL.md](JOURNAL.md) | Append-only, newest first: date, task, what changed, commit | Whoever lands something |
| `notes/<id>.md` | Hypothesis, evidence with commands, log of tries and verdicts | Anyone working that task, including subagents |
| `briefs/<id>.md` | The context packet a subagent is given | Orchestrator, before spawning |
| `templates/` | `note.md`, `brief.md` — copy, do not improvise a shape | — |

One writer per file avoids races: subagents append to **their own note**, never to
the ledger. The orchestrator moves ledger rows after reading the note.

## Statuses

`todo` · `active` · `blocked(<id>)` · `landed` · `rejected` · `parked` · `ongoing`

`landed` means a gate ran and passed, with the command recorded. Anything else is
`open` in spirit, whatever it feels like. `rejected` is as valuable as `landed`: it
is the only thing stopping the next context from retrying a known dead end. `ongoing`
is for a standing lane that never completes, only stays green.

## Checkpoint rule — write during, not after

- After **any** measurement or test run: append the numbers **and the exact command**
  to the active note's Evidence table.
- After **any** decision, dead end, or backed-off approach: append one Log line ending
  in `LANDED`, `REJECTED`, or `OPEN`, with the reason. A backed-off approach without a
  recorded reason will be tried again by the next context; that is the failure mode
  this board is built against.
- Before a long or risky operation, and whenever the session has run long: update the
  ledger row and add one journal line. Assume the next turn starts cold.
- Never make a status optimistic. `unknown` plus the reason beats a guess.

## Number provenance — `gate` vs `diag`

Every size in a note carries a tag.

- `gate` — produced by `target/debug/lilscript-codec --json <artifact>...`, the only
  authority ([codec measurement](../../verification/codec-measurement.md)).
- `diag` — anything else: CLI `brotli`, Node `zlib`, an editor's byte count.

This is not bookkeeping ceremony. On 2026-08-19 the CLI `brotli -q 11` read
`marked.esbuild.js` as 10,173 B and `lilscript-codec` read 10,174 B. One byte is a
regression under [the gate](../../../../comparison/cases/README.md); a `diag` number
that leaks into a claim is how a false win gets published.

## Subagents

Never spawn one without a brief file on disk. Re-explaining the mission in a prompt
is how the invariants get paraphrased into something weaker.

1. Write `briefs/<id>.md` from [templates/brief.md](templates/brief.md).
2. Spawn with a prompt that says only: read `docs/knowledge/mission.md`,
   `docs/knowledge/migration/board/briefs/<id>.md`, and
   `docs/knowledge/migration/board/notes/<id>.md`; do the task; append your evidence
   and log lines to the note; return a summary of at most 20 lines.
3. On return, read the **note**, not the summary, before moving the ledger row.

A brief must name the files the agent may touch, the command that proves the work,
and the refusals ([the standing ones](#standing-refusals)). An agent that cannot
state its gate before starting has not been briefed.

## Standing refusals

These hold for every task on this board and every agent spawned from it.

1. No glue. No host trampolines (`pickRegex3`, `stringTrim`), no grouping helper
   invented to dodge a fold, no per-library special case in the compiler.
2. No post-minifying LilScript output to pass a gate, ever. Terser/Oxc/Closure on
   an already-scored LilScript artifact is diagnostic, not a tactic.
3. No weakening or reinterpreting a gate because the current build loses it.
4. No `diag` number in a claim, a doc, or a ledger row.
5. Semantic mismatch is red before size is even read.
6. Classify a port loss before coding: compiler bug, unreachable incumbent,
   search miss, missing language proof, glue-shaped `.lil`, or legitimate hatch
   ([compressor surface](../../language/compressor-surface.md)). A missing proof
   is language/analysis work, not a peephole.
7. Do not widen candidate search while syntax, binding, identity, ABI, or
   obligation validation is red. A beam that ranks invalid JS is worse.
8. Size-first library compiles are the product
   ([contract](../../mission.md#user-intent)). A
   cleaner coordinator that still starves 18 KiB modules or hides families
   behind the root TOML subset has not landed.

## Bootstrap

```sh
export PATH="$HOME/.cargo/bin:$PATH"   # cargo is not on PATH in non-interactive shells
node scripts/board.mjs status          # ledger + active tasks + note heads
node scripts/board.mjs check           # every task has a note, next step, and gate
node scripts/board.mjs new <lane>-<nn> "title"   # scaffolds note + brief + ledger row
```
