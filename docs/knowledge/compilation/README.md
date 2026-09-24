# Compilation

Parent: [knowledge tree](../README.md). Intent: [mission](../mission.md).
Durable rationale: [design decisions](../decisions/README.md). Live state:
[current status](../../current-status.md).

There is one compiler. It turns a closed, checked module graph into JavaScript
and, for the portable subset, C. Correctness and boundary contracts decide which
artifacts are legal; the configured objective ranks only legal alternatives.

| Page | Question |
|---|---|
| [Current architecture](current-architecture.md) | What does the code do today, stage by stage, and which plan task closes each gap? |
| [Future architecture](../../future-architecture.md) | What is the compiler's architecture, and the size-relevant language design? |
| [Migration plan](../../migration/index.md) | Which bounded steps get there, and how is each verified? |
| [History](../history/README.md) | How did the deleted route work (typed CFG/SSA IR, optimizer, emitter, text peephole, decision registry, search)? |

The history pages are prior art: plan rule 7 asks each batch to read the old
route's version of what it rebuilds. They never describe current behavior.
