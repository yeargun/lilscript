# Compilation

The compiler turns a closed, typed module graph into JavaScript (and optionally
C/native). Contested choices are **supposed** to be decided by complete-artifact
codec scores under a configured objective
([objectives](objectives.md)). [Current architecture](current-architecture.md)
records where that is true; [goal architecture](goal-architecture.md) defines
the replacement decision system. [Decision registry](decision-registry.md)
catalogs known varying behavior and gaps in that catalog.

Parent: [Mission](../mission.md). Language: [Language](../language/README.md).
Knobs: [Config](../config/README.md). Plan:
[07](../migration/07-global-compressor.md).

## Overview

- [Current architecture](current-architecture.md) — implemented pipeline and gaps
- [Goal architecture](goal-architecture.md) — solver model, pseudocode, and guarantees
- [Objectives](objectives.md) — size/performance × raw/gzip/Brotli; exact vs heuristic
- [Decision registry](decision-registry.md) — proof, incumbent, scored, or heuristic
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
- [Peephole](peephole.md) — parsed JS; scored in production, unscored when search is off
- [Chunk planning](chunk-planning.md)

## Native and correctness

- [Native backend](native-backend.md)
- [Correctness and fallbacks](correctness-fallbacks.md)

## Invariants

1. The configured optimizer and emission are always a candidate. Experimental variants that fail to compile are dropped.
2. Search **usually** does not enable a tactic omitted from the exact `javascript.compression` allowlist. Exceptions: size-first search-only names (`indexed-char-at`), and the unconditional `elide_length_tonumber` flip.
3. `[optimization] foo = false` is a hard off.
4. Type checking and mandatory IR normalization do not depend on `priority` or `optimization_level`.
5. Split/preserve-modules still whole-program optimize before any chunk boundary.

Layout search is off in root `lilscript.toml` (no `joint-representation-search`). Brotli packing cannot be re-enabled by the Cartesian beam.
