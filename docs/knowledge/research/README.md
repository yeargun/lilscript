# Compression research

Parent: [knowledge tree](../README.md). Active work:
[migration](../migration/README.md). Test contract:
[verification](../verification/README.md).

This folder is non-authoritative history and experimental evidence. It is not a
default LLM retrieval set, implementation checklist, source of live status, or
numerical release authority. Every imported idea must become a reusable semantic
proof, registered candidate, and reproducible ablation before it enters the
canonical architecture.

How those ideas must enter the compiler (registered recipe, not a package-shaped
special case): [planned architecture](../compilation/planned-architecture.md),
[objectives](../compilation/objectives.md),
[planned migration](../migration/planned-migration.md).
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

An idea seen in Closure/Terser/Oxc or in a small corpus is evidence that a pattern
matters, not a universal rule or proof that the same spelling wins under
LilScript's configured objective. Exact complete-artifact scoring remains final.
