# 044 — a ternary arm loses its parentheses

**Status: CONFIRMED AND FIXED, landed with this folder. `fold_common_conditional_arms` re-emitted a
stripped arm bare; it now re-parenthesizes an arm that holds a top-level comma (`conditional_arm_text`)
and admission refuses `?a,b:c` (`validate_conditional_arm_sequences`). Seven shapes node-identical
through the fold and its late pass; the 041 narrowed artifact is refused at byte 40789 and the one
rebuilt behind the fix is admitted, `--check` clean, 6/6, 28641 Brotli (−663 against 29304);
suite 1663/1663. Closed 2026-09-02.**
Lane: compiler. Objective: brotli (correctness first). Ports: jquerylil, where 041's narrowed rename
exposed it; every port re-measured. Opened: 2026-09-02.

## Prior art

- **Terser** never emits it because parenthesisation is the printer's decision, not the transform's:
  `lib/output.js` `PARENS(AST_Sequence, …)` wraps a sequence whenever its parent is a conditional,
  call argument, unary, binary, property access or arrow body; `AST_Conditional` arms print through
  `print_expression` with that rule. Its `AST_If` → conditional folds (`lib/compress/index.js:1156-1208`)
  build `AST_Sequence` nodes freely and rely on it.
- **Oxc** prints from an AST (`oxc_codegen`, outside `finer/refs/`): `print_expr` takes a precedence
  and wraps a `SequenceExpression` below `Comma` precedence in parentheses.
- **Closure**'s `CodeGenerator` adds parentheses by precedence (`addExpr` with the parent's
  precedence) — the same rule.
- **LilScript** folds text, so precedence is each fold's responsibility. 040 put the printer's
  *adjacency* rule at the one splice point; there is no equivalent for *precedence*. The validator
  (`analyze_generated_javascript`) parses `a?b,c:d` as a valid expression region and does not refuse
  it; node refuses it (`SyntaxError`).

## Claim

The fold at `src/js_peephole/folds/boolean.rs:2352-2360` (`fold_common_conditional_arms`) takes the
text of an arm that is a parenthesised sequence and re-emits it bare. Confirms: a unit shape
`x?(a,b):c` through the fold keeps or restores the parentheses, node runs it, and the artifact 041
produced (`finer/out/041/jquery.esm.narrowed.js`) rebuilt with the fix passes `node --check` with
the same 28571 Brotli as the hand-repaired one. Falsifies: the parentheses are dropped elsewhere
(then the trace names the fold, and it gets the same fix). Second claim: the validator refuses
`?a,b:c` so a future fold cannot ship it — confirms: the 041 narrowed artifact is rejected at
admission before the fix and admitted after.

## Read

- `finer/objective.md`, `finer/status.md`, this folder; [041](../041-the-local-rename-starves/README.md) Result (the failing site) and [040](../040-splices-have-no-adjacency-rule/README.md) (the sibling rule)
- `src/js_peephole/folds/boolean.rs:2300-2400`; the validator's expression-region parser (`parse_expression_regions` in `src/js_peephole/mod.rs`) for where a ternary arm is checked
- Terser `lib/output.js` `PARENS` rules for `AST_Sequence` as the reference

## May touch

- `src/js_peephole/folds/boolean.rs`, `src/js_peephole/mod.rs` (validator), `src/js_peephole/tests.rs`; this folder; `finer/out/044/`

## Method

1. Unit shapes through `fold_common_conditional_arms` and through the whole peephole, node as the
   oracle: `x?(a,b):c`, `x?c:(a,b)`, `x?(a,b):(c,d)`, nested ternaries, a sequence as an arrow body,
   a sequence as a call argument.
2. The validator: `?a,b:c` refused with a named error; the suite still green.
3. Rebuild the 041 narrowed jquerylil artifact through the fixed pipeline (`git apply
   finer/out/041/narrow-the-bail.patch`, port build on four cores); `node --check`, the 039 harness
   6/6, codec sizes.
4. Then 041's Next: suite, fleet A/B with the narrowing, both folders' Result and Status.

## Result

| variant | evidence |
|---|---|
| 041 narrowed build, before | `…?o=…,s=u in e,s&&(r=e[u]):s=o`: `node --check` fails. With the new validator, admission refuses it at byte 40789, "sequence in a conditional arm without parentheses"; the hand-repaired and the base artifacts are admitted (`finer/out/044/validator-over-041-artifacts.log`) |
| unit shapes | `a_sequence_moved_into_a_conditional_arm_keeps_its_parentheses`: `x?(a,b):c` → `x\|\|y?(n.push(1),y):0`; `x?c:(a,b)` → `x&&y?1:(n.push(2),0)`; both arms → `x&&y?(n.push(1),1):(n.push(2),2)`; three nested → `x\|\|y\|\|z?(n.push(1),1):2`; an arrow body `()=>(a,b)` and a call argument `q((a,b))` keep their own parentheses and gain none; the jquerylil site → `a&&b?(o=…,s=u in e,s&&(r=e[u])):s=o`. Each through `fold_common_conditional_arms` alone and through `late_generated_javascript_cleanup_pass(CommonConditionalArms)`, admitted, stdout identical to the source under node: 7/7 |
| validator | `a_bare_sequence_in_a_conditional_arm_is_refused`: six bare shapes refused with the named error (`x?a,b:c`, nested, in an else-arm, as a call argument, the jquery statement); fourteen with brackets or a comma after the `:` admitted (`x?(a,b):c`, `x?[a,b]:c`, `x?{a:1,b:2}:c`, `x?f(a,b):c`, `x?a:b,c`, `x?()=>(a,b):c`, `for(x?a:b,c;;);`, `x?.y`, `a??b?c:d`, …). The pre-existing `factors_repeated_conditional_arms_without_reordering_tests` still passes: no parentheses where no comma |
| 041 narrowed build, after | jquerylil built in its tree with the fixed binary plus `narrow-the-bail.patch` (four cores, `finer/out/044/build-port.sh`): `node --check` clean, 6/6, 40 header spellings, 82602 / 31882 / **28641** (−663 against the 29304 base, itself rebuilt byte-identical to 041's `jquery.esm.base.js`; not the hand-repaired 28571 — the search took another path through the fixed fold, 70 bytes short of 041's prediction, `animate` smoke ok) |
| suite | `cargo test --release` on the fixed tree with the patch: 1663 passed, 0 failed |

## Verdict

**Confirmed, and fixed at the fold.** The parentheses are lost exactly where the claim put them:
every unit shape reproduces the bug through `fold_common_conditional_arms` alone, and the validator's
first refusal on the 041 artifact is that fold's site; nothing else in the seven shapes drops them.
The fold strips an arm's parentheses to compare arms by text, then re-emitted the stripped text at
four sites (`then_value` / `else_value` in each branch). All four now go through
`conditional_arm_text`, which restores the parentheses when the stripped range holds a top-level
comma — the one expression below AssignmentExpression, the arm's grammar. `strip_parenthesized_range`
has no other caller, and `render_logical_operand` already carried the rule for the condition side.

**The second claim holds too.** `validate_conditional_arm_sequences` counts open `?` per bracket
frame and refuses a `,` while one is open; the 041 artifact that admission used to pass is now
rejected at byte 40789, the repaired one and every fleet artifact are admitted, and a fold that
strips a sequence into an arm again dies at admission instead of shipping.

The lesson is 040's, one level up: Terser's printer owns *precedence* (`PARENS(AST_Sequence)` under
an `AST_Conditional`, `AST_Arrow`, `AST_Call`, … parent) as it owns adjacency, so its `if`-to-
conditional folds build sequences freely. LilScript's folds are the printer, so each fold that
re-spells a sub-expression carries the rule itself; 040 put adjacency at the splice, this puts
precedence at the only arm-spelling site. Inventory row to add (`refs/competitor-techniques.md`,
outside this folder's May-touch): *sequence parenthesisation — Terser/Oxc/Closure at print time by
parent precedence; LilScript per fold, PRESENT in `render_logical_operand` and
`conditional_arm_text`, checked at admission.*

Fleet: the fix shipped in one A/B with 041's narrowing (two changes, one binary, since the bare
sequence only ever appeared with the rename on); the per-port table is in
[041/measurements.md](../041-the-local-rename-starves/measurements.md) §C — no port worse, jquerylil
−663, remarklil −2094, rehypelil −766, remark-gfmlil −277, markedlil −62.

## Next

None for this folder. jquerylil's tree now holds the fixed artifact, uncommitted; shipping it
still owes the port an `animate` test (040). The fold's sibling stripping helpers in other folds
were not audited beyond `strip_parenthesized_range`'s callers — a grep for folds that re-emit an
arm or an operand from a stripped range is the cheap follow-up if a second shape ever surfaces.
