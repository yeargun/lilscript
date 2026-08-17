# Active compression-verification migration

Parent: [knowledge tree](../README.md). Verification contract:
[verification](../verification/README.md). Archive:
[old migration](../old-migration/README.md).

The **main goal** is a growing set of independently authored LilScript/JavaScript
pairs that prove compressability. Each case is a folder. Each gated metric compares
the LilScript compiler's own JS (already mangled, DCE'd, searched) to the smallest
valid Terser / Oxc / esbuild artifact of the JavaScript source. LilScript must be
**no larger**. A strict-win case must be smaller. There is no "compare unminified JS
to mangled Lil" lane.

This replaces the catalog-era phase list. That plan is kept as
[catalog-era archive](../old-migration/catalog-era-2026-08/README.md).

## Invariant

For every eligible case and metric `m` in `{raw, gzip-9, Brotli-11}`:

```text
size[m](LilScript compiled with cost_model = m)  <=  min(size[m] of valid JS minifiers)
```

A semantic mismatch is red before size. A size loss is a compiler or LilScript-source
bug, not a reason to weaken the gate. Parameter copies of one fold do not count as
coverage; unique semantic families do.

## Phases

| Phase | Purpose | Exit signal |
|---|---|---|
| [00](00-canonical-runner.md) | Folder-per-case runner next to the generated catalog | `node comparison/cases/run.mjs --canonical-only` is the daily loop |
| [01](01-scalars-folding.md) | Integers, numbers, bools, strings as local rules | Scalar families have canonical folders; no gated loss |
| [02](02-control-functions.md) | Branches, loops, closures, defaults, DCE | Control/function families pass `le`; named wins pass `lt` |
| [03](03-aggregates-wins.md) | struct/class/enum vs ordinary JS objects | Typed layout cases are strict Brotli wins or documented missing proof |
| [04](04-collections-effects.md) | Arrays, records, maps, throw/finally, generators, tasks | Edge families pass; host cases stay explicit `extern` |
| [05](05-modules-search.md) | Modules, lazy, codec search, compiler bugs found by the suite | Failures become minimized compiler tests; search still scores complete artifacts |
| [06](06-scale-release.md) | Keep catalog + algorithms + canonical; promote gates | Canonical + catalog + algorithm lanes block release |

## Working rules

1. Add several related folders, run `--canonical-only` (or `--only family/`), then
   broaden. Do not stockpile unexecuted fixtures.
2. Always compare **compressed minified JS** vs **LilScript compiler output** under
   `lilscript-codec`. Never post-minify LilScript for the gate.
3. If LilScript is larger, first ask whether the `.lil` was written as glue-TS. If
   the pair is fair, fix the compiler.
4. `lt` is for a named typed advantage (scalar replacement, enum discriminant, DCE of
   a proven-dead helper, string pooling). Ordinary portable code is `le`.
5. The generated catalog (`catalog.mjs`, 549 parameterized cases) remains a
   regression net. Canonical folders are the reviewed source of truth for *why*.

## Ownership

- This folder owns order.
- [Verification](../verification/README.md) owns measurement meaning.
- `comparison/cases/canonical/` owns the hand-authored corpus.
- `comparison/cases/catalog.mjs` owns generated variants.
- `comparison/algorithms/` owns multi-function challenges.
