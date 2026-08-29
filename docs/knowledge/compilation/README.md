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
- [Planned architecture](planned-architecture.md) — smallest intended replacement
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

- [Planned migration](../migration/planned-migration.md) is the executable order.
- [Ledger](../migration/board/LEDGER.md) is live task state.
- [Goal architecture](goal-architecture.md) and
  [phase 07](../migration/07-global-compressor.md) are archived design history;
  they do not override the smaller current plan.
