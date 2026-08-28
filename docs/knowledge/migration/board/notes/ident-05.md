# ident-05 — candidate search can rank an artifact whose names do not resolve

Parent: [ledger](../LEDGER.md). Status: landed. This was **07.1**. Unblocks
[search-02](search-01.md). [arch-02](arch-02.md)–[arch-04](arch-04.md) wait on
the rest of 07.1 (`ident-02`–`ident-04`).

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
broken, and the broken artifacts are the *smaller* ones.

The last remaining hole (2026-08-28) was not an unbound name. Beta-reduction of
`(r=>[...r].length)(a)` treated `[...r]` as a property access because the lexer
emits three `.` tokens, skipped substituting `r`, and left the helper parameter
spelling. In marked that spelling was the still-live `exec` match, so
`points(rDelim)` became `[...endMatch].length`. Emphasis at reserve 0 and
strikethrough at 8/12/48 were the same hole.

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
| 2026-08-28 | Spread operand is not a property | `cargo test --lib beta_reduce_substitutes_a_spread_operand beta_reduce_does_not_leave_code_point_length_bound_to_the_match size_first_search_spreads_a_delimiter_not_the_live_match` | 3 passed | gate |
| 2026-08-28 | marked reserve 0/8/12/48 always vs official | `node /tmp/verify-marked-official.mjs /tmp/marked-reserve-{0,8,12,48}.mjs` | 660/660, 0 throws at every reserve | gate |
| 2026-08-28 | react-markdown `candidate_search = always` | compile with `/tmp/react-markdown-always.toml`; `npm test` in `react-markdownlil` | 93/93 | gate |

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
  false-positives. — **LANDED** as the unbound guard, not as the whole task
- 2026-08-20 — Hole: `score_plan` analyzed the declaration spelling and then ranked
  the peephole sibling even when the sibling leaked (`var y` in the list tokenizer,
  `y.exec` in the table path). Fixed by only adding the optimized variant when
  `analyze_generated_javascript(&optimized.code).is_ok()`, plus
  `retain_resolved_javascript` after remaps/cleanup. marked now builds with
  `candidate_search = "production"` and passes 21/21 including setOptions. The
  reserve matrix has not been re-run. — **LANDED** as the sibling-score hole
- 2026-08-27 — Concrete reproduction on `react-markdownlil` with
  `candidate_search = "always"`: the artifact parses, has no unbound store, and
  throws `r is not a function`. Inside an inlined IIFE `r` is the `Info` factory;
  in the enclosing scope `r` holds a property value. The hole is that
  `validate_resolved_generated_bindings` only rejects a read that resolves to
  *nothing* — a read that resolves to the wrong, nearer binding passes. The fix
  belongs in the renamer: a local may not take a name that shadows an outer
  binding referenced by the function or any closure nested inside it. Three other
  search miscompiles on the markdown stack were fixed and are recorded in
  [md-01](md-01.md); this is the one that remains. — **OPEN**
- 2026-08-28 — Enclosing-binding reservation, mixed-`for` module writes, `JS.string`
  of `JsValue`, and callee-name reservation landed in the working tree. react-markdown
  `always` went 93/93. marked reserve 0 `always` still had 18 emphasis mismatches. —
  **OPEN**
- 2026-08-28 — `is_property_identifier` treated `[...r]` as `obj.r` because the
  lexer emits three `.` tokens. `substitute_idents` therefore skipped the spread
  operand, and `(r=>[...r].length)(delim)` kept `r` as the live match. Fix:
  rest/spread is not a member; beta-reduce substitutes the operand. marked
  `local_name_reserve` 0/8/12/48 with `candidate_search = always` is 660/660 vs
  official (0 throws), including the strikethrough that used to fail at 8/12/48.
  react-markdown `always` remains 93/93. — **LANDED**

## Next step

Done. 07.1 continues as [ident-02](ident-02.md). Do not flip committed port
`lilscript.toml` files here; that is [search-02](search-01.md).
