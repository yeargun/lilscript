# gate-03 — pin sibling corpus inputs

Parent: [ledger](../LEDGER.md). Status: landed.

## Question

Which committed MotionLil and MobXLil revisions are the intended immutable inputs
for the expanded five-fork compression matrix?

## Current hypothesis

The intended inputs are the current working trees, but MotionLil's expanded
entries/build and MobXLil's production-min config/build are not represented by
their current Git objects. The LilScript repository cannot make those external
states reproducible without the maintainer committing them or selecting another
existing revision.

## Constraints specific to this task

Do not commit, reset, or modify sibling repositories. Do not copy dirty source
into LilScript or pin working-tree-only hashes.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-29 | MotionLil repository state | `git status --short`; `git rev-parse HEAD HEAD^{tree}` in `/Users/yeargun/motionlil` | HEAD `dcbc09d`; expanded entries and build are modified/untracked | diag |
| 2026-08-29 | MobXLil repository state | `git status --short`; `git rev-parse HEAD HEAD^{tree}` in `/Users/yeargun/mobxlil` | HEAD `e14a5a0`; `config/production.min.toml` and supporting build/source work are untracked/modified | diag |
| 2026-08-29 | MotionLil immutable source | `git status --short`; `git rev-parse HEAD HEAD^{tree}` | clean at `fde1aedfa2e8c84c33375df10ebc4b8be8d1b156`, tree `59d0cc60efb3dc2dd0d0acf314a686ab8d2a4c26` | gate |
| 2026-08-29 | MobXLil immutable source | `git status --short`; `git rev-parse HEAD HEAD^{tree}` | clean at `820c9a8210c8d5489fc0a86a2ca46ecb9259cd5e`, tree `7477c96b401a857d9805443a1e5b45d0fcc47623`; lockfile synchronized | gate |

## Log

- 2026-08-29 — Canonical matrix expansion cannot proceed from dirty external inputs without violating source/config fingerprint reproducibility. — **OPEN**
- 2026-08-29 — With explicit authorization, committed the intended sibling states and a separate Motion evidence hook; both repositories are clean and provide immutable Git objects for every new matrix input. — **LANDED**

## Next step

Gate-02 pins these revisions, adds direct artifact lanes, and records fresh
baseline/checkpoint or successor-compiler evidence.
