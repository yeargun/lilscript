# 040 — splices have no adjacency rule

**Status: FIXED, landed with this folder. `fold_assigned_truthy_ternaries` turned
`return(n=Ir(…))?n:X` into `returnIr(…)||X`; the fused name hid one of the callee's two uses, the
callee was inlined at the other and deleted, and the repair fold refused. Shipped jquerylil threw
`ReferenceError` on `animate`. The fix is Terser's printer rule at the one splice point every fold
uses, so no fold can fuse again.**
Lane: compiler. Objective: brotli (correctness first). Ports: jquerylil; every port re-measured.
Opened: 2026-09-01.

## Prior art

- **Terser** decides spacing at print time, never in a transform: `benchmarks/popular/node_modules/
  terser/lib/output.js:595-605` (`print`) — after any `space()` in minified mode `might_need_space`
  is set, and the space is written only if the last character written and the next token's first
  character are both identifier characters (`is_identifier_char_broad`), or `//`, `++`, `--`. A
  transform that removes parentheses cannot fuse tokens because it never wrote the bytes.
- **Closure** does the same in `CodeConsumer.add` / `maybeInsertSpace` (Java, not vendored): the
  consumer tracks the last character and inserts a space when the next chunk would lex into it.
- **Oxc** prints from an AST (`oxc_codegen`, outside `finer/refs/`), where adjacency is the printer's
  `print_space_before_identifier`; its peephole never touches text.
- **LilScript** splices strings: every fold builds `(start, end, replacement)` and
  `js_peephole::rewrite::apply_token_rewrites` joins them with no rule. The repair fold
  `folds/syntax.rs:255` (`split_fused_keyword_identifiers`) undoes `returnX`/`throwX` only while
  `X` is still a visible binding. Inventory row 23 in
  [refs/competitor-techniques.md](../../refs/competitor-techniques.md).

## Claim

The fusion is not one fold's bug but the absence of the printer's rule at the splice. Confirming
number: with the rule in `apply_token_rewrites`, the traced jquerylil shape emits `return Ir(…)`,
the six splice cases and the whole-peephole shape pass, the suite passes, and every fleet artifact
is byte-identical except where a fusion existed, each such site costing exactly one byte.
Falsifying number: any port whose artifact changes by more than the spaces at former fusions.

## Read

- `src/js_peephole/rewrite.rs` (`apply_token_rewrites`), `src/js_peephole/folds/boolean.rs:695`
  (`fold_assigned_truthy_ternaries`), `src/js_peephole/folds/syntax.rs:209-320`
- [039](../039-terser-spells-names-by-frequency/README.md) Status line, where the fusion was found

## May touch

- `src/js_peephole/rewrite.rs`, `src/js_peephole/tests.rs`; this folder; `finer/out/040/`

## Method

1. Trace the full jquerylil compile (`LILSCRIPT_PEEPHOLE_TRACE=1`, `LILSCRIPT_DUMP_CANDIDATES`) through
   an `awk` filter that names the first fold whose output carries `return[A-Z]…(`: fold #471 of
   45067 traced, `boolean::fold_assigned_truthy_ternaries`, on
   `r=r||[];r=Ir(r,t,e,n);if(r)return r;return Ir(K["*"],t,e,n)` after an earlier fold had made it
   `return(r=Ir(…))?r:Ir(…)` with the space already elided before `(`. 64 of 4232 scored
   candidates carried the fusion; the dumped pre-fusion spelling is in `finer/out/040/`.
2. Put the rule at the splice: `separated_at_boundaries` in `rewrite.rs` inserts one space when the
   byte before the span and the replacement's first byte (or, for a deletion, the two neighbours)
   would lex as one token, and symmetrically at the end.
3. Tests: `splices_never_fuse_neighbouring_tokens` (six splice shapes plus the shipped shape through
   the whole peephole with node as the oracle) and
   `return_keyword_is_never_fused_into_its_operand`. Suite, then the fleet A/B against the 037
   binary.

## Result

| variant | evidence |
|---|---|
| shipped jquerylil, committed and tree | `returnHr(r,n,t,e)` / `returnRn(n,t,e,r)`; `$(el).animate()` throws `ReferenceError` |
| traced compile, 037 binary | first fusion after fold #471 `fold_assigned_truthy_ternaries`; 64/4232 candidates fused |
| guard, unit | 6/6 splice cases exact; whole-peephole shape node-identical |
| guard, suite | see status.md — recorded when the run completes |
| guard, fleet | see status.md — one byte per former fusion is the prediction |

## Verdict

Confirmed on the mechanism and fixed at the class level. The lesson is the owner's §10: Terser has no
such bug because its printer owns adjacency, and reading `output.js` first would have put the guard
at the splice instead of a repair fold behind it. `split_fused_keyword_identifiers` stays as a
second line. jquerylil must be rebuilt and its `animate` path tested before it ships again — its six
compat tests never call it (039), and a test that does belongs to the port.

## Next

Rebuild jquerylil with the guard, add an `animate` test to the port, ship. Then 039's second finding:
`converge_local_names` starving on the same port (status.md lead 2).
