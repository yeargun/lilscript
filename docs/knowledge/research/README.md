# Compression research

Parent: [knowledge tree](../README.md). Active work:
[migration](../migration/README.md). Test contract:
[verification](../verification/README.md).

This folder records external ideas worth testing and the limits on transferring them
into LilScript. It is not an implementation checklist and it does not turn another
compiler’s marketing or source code into LilScript evidence. Every imported idea must
be expressed as a semantic proof, an eligible candidate, and a reproducible ablation.

How those ideas must enter the compiler (registry row, not a peephole special
case): [goal architecture](../compilation/goal-architecture.md),
[objectives](../compilation/objectives.md),
[migration 07](../migration/07-global-compressor.md).
What the language must state so Closure/Terser cannot uniquely guess it:
[compressor surface](../language/compressor-surface.md).

## Pages

### Toolchains and codecs

- [Closure ADVANCED](closure-advanced.md)
- [Terser, Oxc, esbuild, and Vite](terser-oxc-vite.md)
- [Gzip and Brotli](gzip-brotli.md)

### Labs

- [Brotli, the whole machine](brotli-machine.html) — RFC 7932 encode/decode
  (rebuild with `node docs/knowledge/research/brotli-machine/render.mjs`)
- [Aligned mangling](aligned-mangling/README.md)
- [Brotli mangling lab](brotli-global-mangle/lab.html) — dictionary, transforms, spelling quirks
  (tiny-file generator: `brotli-mangle-lab.mjs` + `render-brotli-mangle-lab.mjs`)
- [Brotli global-mangle playbook](brotli-global-mangle/README.md)

## Research-to-compiler rule

1. Name the source behavior and assumption.
2. Write the smallest paired semantic case.
3. Express legality from LilScript types/effects/escape facts.
4. Add both transformed and untransformed complete artifacts to codec selection.
5. Measure raw, gzip-9, and Brotli-11; record compilation/runtime cost.
6. Keep a regression or ablation even when the idea is neutral or loses.

An idea seen in Closure/Terser/Oxc is evidence that a pattern matters, not proof that
the same spelling wins under LilScript’s Brotli-default objective.
