# ident-05 — candidate search can rank an artifact whose names do not resolve

Parent: [ledger](../LEDGER.md). Status: active. Blocks [search-01](search-01.md).

## Question

Candidate search emits and scores whole artifacts. Some winners reference an
identifier that resolves nowhere at run time (`Se is not defined`). Which stage
loses the binding, and how does the search learn to refuse such a candidate?

## What is known

The bug is **pre-existing** — it reproduces on the compiler as it stood before the
[ident-01](ident-01.md) fix, with a different name (`y` instead of `Se`). It is
reachable only with `candidate_search` on; the same source with search off is
correct at every setting tried.

It is also **selection-sensitive rather than shape-sensitive**: the same program and
the same passes, with only `local_name_reserve` changed, moves between correct and
broken, and the broken artifacts are the *smaller* ones. So the search is currently
winning by ranking a program that throws.

Full marked spec corpus, `candidate_search = "production"`, everything else fixed:

| `local_name_reserve` | spec | Brotli-11 |
|---|---|---|
| 2 | 660/660 | 9,253 |
| 4 | 660/660 | 9,253 |
| 6 | 108/660, 552 threw | 9,175 |
| 8 | 108/660, 552 threw | 9,175 |
| 12 | 0/660, 660 threw | 9,115 |

In the reserve-8 artifact the failing read is `C(e.t,Ue,e.r,_e,Se)` inside an arrow
emitted at its call site. `Ue` and `_e` resolve to top-level regex globals. `Se` and
`C` do not: `Se` exists only as a local of a *different* function (the table
tokenizer), and the regex it should name is declared at top level as `Ke`. So two of
the five operands in one call carry names from a different table than the one that
was emitted — the drift is per-region, not per-name.

## Constraints specific to this task

- The fix belongs in the compiler, not in a per-project config. `local_name_reserve`
  is a size knob; correctness may not depend on its value.
- A candidate that cannot be proven valid must lose, however small it is.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-19 | Reserve matrix above | `node verify-config.tmp.mjs /tmp/r-<n>.toml /tmp/r-<n>.mjs` in `markedlil` | 2/4 green, 6/8/12 throw | gate |
| 2026-08-19 | Pre-existing, not from ident-01 | build original `codegen_ir_js.rs`, compile with `/tmp/plain-always.toml` | throws `y is not defined` | diag |
| 2026-08-19 | Needs search | same source, `candidate_search = "off"` | correct at every reserve tried | diag |

## Log

- 2026-08-19 — Found while taking marked to a Brotli win: the winning artifact threw
  on GFM table + blockquote (`gfm.0.29.json#201`). — **OPEN**
- 2026-08-19 — Tried a scope guard in selection: reject any candidate where a name is
  bound at one use and unbound at another, then take the next best. It caught the
  marked case, but the peephole scope helpers are not a scope resolver — the rule
  produced false positives on ordinary programs and rejected *every* candidate in ten
  compiler tests ("startup limits rejected every JavaScript candidate"). Reverted. A
  guard is still the right shape; it needs a real binding resolver, or the compiler's
  own extern set passed in so only non-external free names are flagged. — **REJECTED**
  as implemented, not as an idea.
- 2026-08-19 — Shipped marked at `local_name_reserve = 4`, which the 660-case gate
  proves correct, and recorded the matrix above so the setting is not mistaken for a
  size preference. — **OPEN**
- 2026-08-20 — Guard landed as `validate_resolved_generated_bindings`: a value-use
  that is not visible in any enclosing function, but is bound as a local of some
  *other* function, is `unresolved generated identifier`. Visibility walks enclosing
  functions with `function_directly_binds_name` (nested `var`/`let`/`const` bodies
  skipped). `function_scope_declares` must not be used here — it attributes a nested
  `var y` to the ancestor IIFE and misses the leak. Expression-arrow params need
  `name_is_declared_in_enclosing_expression_arrow` or `e=>e+1` next to `function f(e)`
  false-positives. — **PARTIAL**
- 2026-08-20 — Hole: `score_plan` analyzed the declaration spelling and then ranked
  the peephole sibling even when the sibling leaked (`var y` in the list tokenizer,
  `y.exec` in the table path). Fixed by only adding the optimized variant when
  `analyze_generated_javascript(&optimized.code).is_ok()`, plus
  `retain_resolved_javascript` after remaps/cleanup. marked now builds with
  `candidate_search = "production"` and passes 21/21 including setOptions. The
  reserve matrix has not been re-run. — **PARTIAL**

## Next step

Find the stage that loses the binding, working from the reserve-8 artifact: the arrow
at the failing call site is emitted at its only call site, so compare the names its
body uses against the plan that produced it. If the region is rendered before the
final name table is fixed, that ordering is the bug. Only then re-attempt the guard,
with the compiler's extern set rather than a "bound somewhere" heuristic.

- 2026-08-27 — Concrete reproduction found on `react-markdownlil` with
  `candidate_search = "always"`: the artifact parses, has no unbound store, and
  throws `r is not a function`. Inside an inlined IIFE `r` is the `Info` factory;
  in the enclosing scope `r` holds a property value. The hole is that
  `validate_resolved_generated_bindings` only rejects a read that resolves to
  *nothing* — a read that resolves to the wrong, nearer binding passes. The fix
  belongs in the renamer: a local may not take a name that shadows an outer
  binding referenced by the function or any closure nested inside it. Three other
  search miscompiles on the markdown stack were fixed and are recorded in
  [md-01](md-01.md); this is the one that remains. — **OPEN**

