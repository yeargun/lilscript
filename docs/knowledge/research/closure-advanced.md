# Google Closure Compiler ADVANCED

Parent: [research](README.md). Local mapping:
[`docs/optimization-coverage.md`](../../optimization-coverage.md). Maintained lane:
[`comparison/`](../../../comparison/README.md).

## Why it is the serious reference

Closure ADVANCED is a whole-program JavaScript optimizer, not only a whitespace
minifier. Its value as a reference is the discipline around compilation-unit
ownership, reachability, exports/externs, type-assisted property reasoning, repeated
optimization, renaming, and module/chunk processing. The official
[flags reference](https://github.com/google/closure-compiler/wiki/Flags-and-Options)
documents ADVANCED mode, dependency pruning, externs, entry points, chunks, type-based
optimization, and export generation.

Closure’s [FAQ](https://github.com/google/closure-compiler/wiki/FAQ) makes two lessons
especially relevant here:

- uncompressed bytes can mislead, because repeated strings may compress better when
  left inline for gzip;
- the largest ADVANCED wins depend on an accurate external boundary: code not used or
  exported can be removed, while externs describe APIs outside the compilation.

That aligns with LilScript’s served-byte and closed-world goals. It does **not** mean
the compilers have the same semantics or search strategy.

## Responsibility mapping

| Closure responsibility | LilScript analogue | Difference to preserve |
|---|---|---|
| dependency/entry pruning | typed static graph + DCE | LilScript imports disappear before SSA |
| externs/exports | typed `extern`, escape, public ABI config | boundary is a language construct, not JSDoc/header JS |
| function inlining/specialization | typed IR variants | alternatives can be complete-artifact codec candidates |
| property collapse/rename/disambiguation | scalar replacement, positional fields, owned-property mangle | names may never exist for internal aggregates |
| remove unused code | effect-aware IR DCE | purity is inferred/declared before JS spelling |
| variable/property renaming | frequency/entropy/layout search | gzip/Brotli scorer may choose a different alphabet/layout |
| chunks/modules | LilScript split/preserve/Lilpack | compare equivalent artifact sets and public surfaces |

## Boundaries before optimizations

Closure documents externs as declarations for APIs outside the compilation and warns
that ADVANCED can break code whose dynamic access/export assumptions are not modeled.
That is the right pressure for LilScript tests:

- closed-app pairs may mangle every internal name;
- public ESM/script-tag pairs must expose the same names and object fields;
- quoted/dynamic host keys must not become owned properties;
- jQuery cannot use a no-export ADVANCED result as its public-library baseline.

## What to learn, not clone blindly

- fixed-point simplify/inline/DCE cycles: add phase-order candidates and regression
  cases; do not assume more passes always shrink Brotli;
- namespace/property collapsing: prefer stronger typed layout proofs before adding a
  JS-shaped collapse pass;
- alias restrictions: partial/dynamic aliases deserve explicit escape facts and
  diagnostics, not optimistic renaming;
- output ordering and renaming: propose candidates, then score exact served bytes;
- debug/rename reports: improve explainability so a size drift can be attributed.

## Local evidence boundary

`comparison/apps/` currently has seven closed-world paired programs with a pinned
Closure JAR, expected-output checks, and separate raw/gzip/Brotli LilScript builds;
each build is gated only on its matching metric. Those results establish only that
corpus. The jQuery Closure lane remains open because it needs an equivalent
public API/extern boundary. Do not extrapolate the seven-app total to arbitrary JS.

## Source-reading route

When studying a Closure change, start from the current release’s option/pass wiring,
then follow the concrete pass and tests. Record release/commit because Closure changes
regularly. Useful official entry points are the
[repository](https://github.com/google/closure-compiler),
[flags/options](https://github.com/google/closure-compiler/wiki/Flags-and-Options),
[FAQ](https://github.com/google/closure-compiler/wiki/FAQ), and
[annotation guide](https://github.com/google/closure-compiler/wiki/Annotating-JavaScript-for-the-Closure-Compiler).
