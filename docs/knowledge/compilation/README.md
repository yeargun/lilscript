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
| [Active design and plan](../../migration/plan.md) | What architecture, objective policies and configuration behavior are we building? |
| [Migration plan](../../migration/index.md) | Which bounded steps get there, and how is each verified? |
| [History](../history/README.md) | How did the deleted route work (typed CFG/SSA IR, optimizer, emitter, text peephole, decision registry, search)? |

The history pages and [previous migration documents](../../old-history/README.md)
are prior art. Read the relevant evidence when replacing a mechanism; use the
active plan and current source for implementation decisions.
