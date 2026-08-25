# emit-06 — a use-to-binding resolver, and converged naming scored on it

Parent: [ledger](../LEDGER.md). Status: landed (primitive), open (the win).
Predecessor: [emit-05](emit-05.md).

## Question

[emit-05](emit-05.md) established that LilScript emits **fewer** raw bytes than
`jquery.min.js` and **more** compressed ones, and that the deficit is header
spelling diversity. Two cures were rejected there for lack of a primitive: the
peephole could not ask what a name resolves to. Build that primitive, then see
whether converged naming pays.

## Current hypothesis

The primitive works and is worth keeping on its own. Converged naming built on
it is **correct but not a large win**: scored under the codec it is taken on
three artifacts and declined on four, for −76 Brotli overall and no regression.

## Constraints specific to this task

The pass may not be unconditional. Six assignment strategies were measured and
five made Brotli *worse*; the objective has to decide per artifact.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-25 | resolver coverage on five artifacts | scratch test over real `dist` output | jQuery 10,965 bound / 713 unresolved, monaco 22 unresolved, marked 38, zod 31, error-tracking 0 — every scope sound | diag |
| 2026-08-25 | resolver unit tests | `cargo test --release --lib js_peephole::binding` | 12/12, including parameter attribution, shadowing, catch, defaults, destructuring poison | gate |
| 2026-08-25 | renamer unit tests | `cargo test --release --lib js_peephole::rename` | 11/11, six of them running both programs under Node and comparing output | gate |
| 2026-08-25 | scored end to end | `lilscript-codec --json`, nine artifacts | Brotli −76 total: zod −36, otlp −34, error-tracking −6, six unchanged, **none worse** | gate |
| 2026-08-25 | compat | five posthog suites against the new artifacts | 28/28 | gate |
| 2026-08-25 | corpus | `node comparison/cases/run.mjs` | 617/617, strict wins 617/612/613 unchanged | gate |

## Log

- 2026-08-25 — `src/js_peephole/binding.rs`: total use-to-binding resolution. Every identifier answers `Bound(declaration)`, `Free`, or `Unresolved`; nothing returns "maybe". Parameters are attributed to the function whose body follows their list, which is the bug that made the [emit-05](emit-05.md) renamer unsound. Fail-closed: an unaccounted construct poisons **the name**, not the scope — scope-level poisoning let one reused temporary silence a whole file, and switching to per-name cut jQuery's unresolved identifiers from 3,216 to 713. — **LANDED**
- 2026-08-25 — `src/js_peephole/rename.rs`: converged naming on top of it. Six assignment strategies measured on the nine-artifact corpus, Brotli totals: per-scope canonical with strict availability **+6,099**; the same with precise availability **+430**; global frequency order **+691**; positional-first with an `etnrisou` alphabet **+388**; module bindings ranked last **worse still**; positional-first with `abc` **+78**. Only the last is near neutral, and only scoring makes it a win. — **LANDED**
- 2026-08-25 — Two findings from those negatives worth keeping. Availability must be "would this capture?", not "does this name appear?": the appearance test blocks `a` and `b` in nearly every large function and *manufactures* the shifted sequences it was meant to remove. And ordering by use count alone scrambles headers — jQuery emitted `(a,b)` beside `(b,a)` — so parameter position has to lead. — **LANDED**
- 2026-08-25 — Renaming a function or class name changes `Function.prototype.name`. The first version renamed `ErrorPropertiesBuilder` to `f` and failed the identity pin. Those bindings are now never renamed. — **LANDED**
- 2026-08-25 — Header convergence reached 45 distinct arrow spellings from 69, top-three coverage 42% to 51%. Terser reaches 20 and 74% on the same program. The residue is our own module bindings holding `a`, `b`, `c` where functions reference them; ranking module bindings last to free those was measured and lost, because it pushes them to two characters and costs more raw than the convergence returns. — **OPEN**
- 2026-08-25 — jQuery is where the gap is (+8.5% Brotli against `jquery.min.js`) and the probe **fires** there — 5,997 rewrites — and the codec **declines** it. That is a measured verdict, not an omission. Monaco never reaches the probe: its terminal budget is spent first, which is [search-04](search-04.md)'s ledger problem, not this one. — **OPEN**

## Next step

The primitive is the asset; spend it on correctness next. The folds behind
[ident-06](ident-06.md) and [ident-08](ident-08.md) matched text without
resolving it, and all three of this lane's miscompiles are that shape. Migrate
those folds to ask `resolve(token)` instead of inspecting neighbouring
punctuation, one at a time, each verified byte-identical or better.
