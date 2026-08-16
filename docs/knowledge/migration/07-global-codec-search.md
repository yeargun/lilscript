# Phase 07 — global codec and configuration search

Parent: [migration](README.md). Compiler model:
[global optima](../compilation/global-optima.md). Matrix:
[config matrix](../verification/config-matrix.md).

## Objective

Prove the compiler chooses well when the locally shortest step is not the best gzip
or Brotli artifact, and show exactly where bounded search can miss an interaction.

## Required experiment types

- raw win / gzip loss / Brotli loss and each converse;
- candidate families where the first transform loses but a two-step combination wins;
- inlining vs outlining, early CSE vs repetition, pooling vs codec dictionary,
  punctuation/quotes, identifier alphabets, function order, named/positional layout,
  phi/loop/mutation spelling, and parsed peepholes;
- candidate count/byte/beam truncation boundaries, including broad modules;
- `raw`, `gzip`, and `brotli` cost models under every priority;
- startup/performance rank interactions and raw-growth admission;
- chunk plan × symbol/layout interactions;
- exact `compression` / `optimizations` allowlists, `[mangle]` overrides, and every
  hard-off `[optimization]` gate.

## Oracle for optimality

For small fixtures, enumerate the legal candidate cross-product in a test-only oracle
and compare the compiler winner with the exact minimum. For larger fixtures, record
the explored frontier, configured baseline, rejected candidates, and budget reason;
call the result “best found,” never “global optimum.”

## Exit criteria

- Small exhaustive fixtures prove ranking and codec measurement independently of the
  production beam.
- Every config key that changes search has an on/off or boundary case.
- Reports distinguish semantic ineligibility, admission filtering, budget truncation,
  startup rejection, and final rank.
- Raising a search budget cannot silently produce a worse `size-first` selected
  transfer score while the lower-budget baseline remains legal.
