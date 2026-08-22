# Compression research

Parent: [knowledge tree](../README.md). Active work:
[migration](../migration/README.md). Test contract:
[verification](../verification/README.md).

This folder records external ideas worth testing and the limits on transferring them
into LilScript. It is not an implementation checklist and it does not turn another
compiler’s marketing or source code into LilScript evidence. Every imported idea must
be expressed as a semantic proof, an eligible candidate, and a reproducible ablation.

## Pages

- [Closure ADVANCED](closure-advanced.md) — closed-world boundaries, externs/exports,
  whole-program responsibilities, and gzip-aware lessons
- [Terser, Oxc, esbuild, and Vite](terser-oxc-vite.md) — baseline roles, assumptions, and
  options that should become cases rather than copied unsafe rewrites
- [Gzip and Brotli](gzip-brotli.md) — why raw-local choices are not transfer optima
- [Brotli mangling lab](brotli-mangle-lab.html) — static dictionary, transforms, and measured JS spelling quirks
- [Brotli global-mangle playbook](brotli-global-mangle/README.md) — hundred-KB artifacts, reuse / alphabet / color-merge, gzip vs q11 fights

## Research-to-compiler rule

1. Name the source behavior and assumption.
2. Write the smallest paired semantic case.
3. Express legality from LilScript types/effects/escape facts.
4. Add both transformed and untransformed complete artifacts to codec selection.
5. Measure raw, gzip-9, and Brotli-11; record compilation/runtime cost.
6. Keep a regression or ablation even when the idea is neutral or loses.

An idea seen in Closure/Terser/Oxc is evidence that a pattern matters, not proof that
the same spelling wins under LilScript’s Brotli-default objective.
