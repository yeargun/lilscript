# Compilation

Parent: [knowledge tree](../README.md). Intent: [mission](../mission.md).
Durable rationale: [design decisions](../decisions/README.md). Live state:
[`docs/current-status.md`](../../current-status.md).

The compiler turns a closed typed module graph into JavaScript and, for the
portable subset, C/native. Correctness and boundary contracts constrain legal
artifacts; the configured objective ranks only legal alternatives.

## Overview

- [Architecture router](architecture.md) — authority and reading order
- [Current architecture](current-architecture.md) — implemented pipeline and gaps
- [Proposed design](../../compiler-design.md) — objective, target contracts and open language decisions
- [Single migration plan](../../migration/index.md) — all bounded steps and verified progress in one file
- [Objectives](objectives.md) — size/performance × raw/gzip/Brotli; exact vs heuristic
- [Decision registry](decision-registry.md) — implemented choice census
- [Global optima](global-optima.md) — why local “smaller” can lose gzip/Brotli
- [Pipeline](pipeline.md) — stages, single vs split vs native

## Frontend

- [Linking and lowering](frontend-linking-lowering.md) — AST to typed CFG
- [Analyses](analyses.md) — effects, escape, ranges, alias/call facts

## IR

- [Optimizer](ir-optimizer.md) — pass order and `[optimization]` gates
- [DCE and tree shaking](dce-tree-shaking.md)
- [Inlining, specialization, sharing](inlining-specialization-sharing.md)
- [Aggregate lowering](aggregate-lowering.md) — scalar, positional, named
- [Class identity](class-identity.md) — when a constructor must stay ES `class`
- [Compress passes](compress-passes.md) — fusion, sinking, outlining

## JavaScript

- [Emission](javascript-emission.md) — spelling, `IrJsOptions`
- [Mangling, layout, pooling](mangling-layout-pooling.md)
- [Candidate search](candidate-search.md) — two-level search, beam, budgets
- [Peephole](peephole.md) — parsed generated-JS migration layer
- [Chunk planning](chunk-planning.md)

## Native and correctness

- [Native backend](native-backend.md)
- [Correctness and fallbacks](correctness-fallbacks.md)

## Planning

The old migration is retired and archived outside the repository.
[Compiler design](../../compiler-design.md) owns the proposed target;
[migration/index.md](../../migration/index.md) owns the new implementation sequence.
The first steps freeze evidence and settle remaining language decisions. No
implementation milestone is complete merely because a plan exists.
