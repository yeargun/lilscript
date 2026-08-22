# Brotli global-mangle playbook

Parent: [research](../README.md). Tiny-file dictionary lab:
[../brotli-mangle-lab.html](../brotli-mangle-lab.html).

This folder is **reasoning plus measured mutations** on hundred-kilobyte
LilScript / JS artifacts. It does not change the compiler. Several mutations
are illegal at runtime; they are labeled.

Start here: [00 — thesis](00-thesis.md).

## Pages

1. [Thesis](00-thesis.md) — local vs global optimum
2. [Corpora](01-corpora.md) — files and baselines
3. [Reuse](02-reuse.md) — cross-scope short names
4. [Alphabet](03-alphabet.md) — `e` vs `q` at equal raw length
5. [Dictionary as names](04-dictionary-as-names.md) — why ROM locals lose
6. [Literals](05-literals.md) — quotes, booleans, pooling
7. [Declarations](06-declarations.md) — `var` / `let` / `const`
8. [Layout](07-layout.md) — function order; gzip vs Brotli
9. [Bait and glue](08-bait-and-glue.md) — preambles, `.length`
10. [In-tree audits](09-audits.md) — jQuery compiler knobs already emitted
11. [Monaco](10-monaco.md) — megabyte-scale check
12. [Codec disagreement](12-codec-disagreement.md) — gzip / q5 / q11 inversions
13. [Windows and chunks](13-window-chunks.md) — 32K vs whole-file
14. [Quirk catalog](14-quirk-catalog.md) — the ugly wins
15. [Color merge](15-color-merge.md) — hottest local → `e`/`t`
16. [Identifier cultures](16-ident-cultures.md) — `abc` vs `etn`
17. [Playbook](11-playbook.md) — candidate-search heuristics

Tables: [lab.html](lab.html). Raw rows: [results.json](results.json),
[extra.json](extra.json).

## Reproduce

```bash
node docs/knowledge/research/brotli-global-mangle/harness.mjs
node docs/knowledge/research/brotli-global-mangle/extra.mjs
```

Scorer is Node zlib Brotli 1.1.0 generic q11 / gzip-9, the same diagnostic
family as the tiny lab, not `lilscript-codec`. Several mutations are
semantically illegal and are labeled as gravity probes.
