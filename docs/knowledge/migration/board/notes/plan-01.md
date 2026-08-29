# plan-01 — architecture and language review

Parent: [ledger](../LEDGER.md). Status: landed.

## Question

Does the compression migration use language/compiler co-design to expose reusable
proofs without turning source syntax into minifier glue or objective-dependent ABI?

## Current hypothesis

The initial plan has the correct direction but a fresh architecture review may
find proposed surfaces that duplicate existing semantics, lack a complete
unoptimized meaning, or are sequenced before the proof/target infrastructure
needed to implement them safely.

## Constraints specific to this task

Edit only the migration plan and this note. Proposed syntax remains provisional.
Preserve the canonical migration's authority, retained incumbent, exact selected-
codec scoring, and no-glue rules.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-29 | Board metadata gate after the architecture review | `node scripts/board.mjs check` | `board check passed: 38 tasks, 30 notes.` (exit 0) | gate |
| 2026-08-29 | Documentation-link gate after the architecture review | `node scripts/check-doc-links.mjs` | `documentation graph valid: 190 Markdown files, 114 canonical pages reachable` (exit 0) | gate |

## Log

- 2026-08-29 — Initial plan awaits fresh architecture and language review. — **OPEN**
- 2026-08-29 — Replaced the duplicate C0-C11 schedule with canonical phase dependencies, recorded landed syntax, made every language proposal carry semantic/ABI/lowering/test gates, and prohibited objective-selected observable IDs, reflected strings, and diagnostics; both required gates exited zero. — **LANDED**

## Next step

Follow the canonical migration's current phase; phase 3 target-JS work remains
blocked until phases 0-2 and their legality, incumbent, and consolidation exits
are complete.
