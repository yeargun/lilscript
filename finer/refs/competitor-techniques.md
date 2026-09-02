# Competitor technique inventory: oxc_minifier vs terser vs LilScript

Standing homework (objective.md §7, harvest), not tied to a hypothesis folder. Read directly from the
vendored sources below; nothing was downloaded.

**Path shorthands used in citations**

- `OXC/<path>:<line>` = `finer/refs/oxc_minifier-0.147.0/src/<path>`
- `ECMA/<path>:<line>` = `finer/refs/oxc_ecmascript-0.147.0/src/<path>`
- `TERSER/<path>:<line>` = `benchmarks/popular/node_modules/terser/lib/<path>` (all under
  `/home/azureuser/lilscript/`)
- `LS/<path>:<line>` = `/home/azureuser/lilscript/src/<path>`

**Scale note.** `OXC/peephole/*.rs` is ~14.8k lines, `TERSER/compress/*.js` + `output.js` +
`propmangle.js` + `size.js` is ~27.1k lines, and `LS/js_peephole/folds/*.rs` alone is 26.6k lines
(plus `LS/optimizer.rs` at 740KB and `LS/codegen_ir_js.rs` at 1.4MB). The tables below are a
**representative, not exhaustive**, catalog: one row per distinct *category* of technique, citing
the clearest implementation site. Where a category has dozens of near-duplicate sibling functions
(e.g. oxc's `substitute_*` family, LilScript's `fold_*` family), the row cites one or two
representative sites and says so.

Architectural framing that recurs in every section below and matters for reading the tables:

- **oxc**: all ~80 peephole techniques are dispatched from **one** `Traverse` visitor
  (`PeepholeOptimizations`, `OXC/peephole/mod.rs:304-905`) that runs bottom-up over the whole AST
  in a single walk. The walk itself is repeated by a fixed-point driver (Section G).
- **terser**: all techniques are `AST_Node.prototype.optimize()` methods, one per AST node class
  (`def_optimize(AST_X, fn)`, `TERSER/compress/index.js`), invoked by a single recursive
  `.transform()` over the tree. That whole transform is repeated by the `passes` option loop
  (Section G).
- **LilScript**: two distinct layers. (1) An SSA/CFG-level optimizer (`LS/optimizer.rs`,
  `LS/compress_passes.rs`) that is a classical **named-pass pipeline** (~30 explicitly ordered
  passes, several individually fixed-pointed) operating before JS text exists at all. (2) A
  **post-codegen textual peephole layer** (`LS/js_peephole/folds/*.rs`, 26.6k lines across 17
  files) that works like oxc/terser but on tokenized rendered source rather than an AST. Many
  things oxc/terser do as an AST rewrite, LilScript instead does once at IR→JS codegen emission
  time (`LS/codegen_ir_js.rs`), so a "missing peephole" is often not missing — it never had to
  fire because the emitter never produced the worse spelling to begin with.

---

## A. Alternate-syntax substitutions

| Technique | oxc | terser | Size axis | Class | Example |
|---|---|---|---|---|---|
| `true`/`false` → `!0`/`!1` | `OXC/peephole/substitute_alternate_syntax.rs:1645-1658` (`substitute_boolean`) | `TERSER/compress/index.js:3494-3520` (`def_optimize(AST_Boolean,...)`) | raw | peephole | `x=true` → `x=!0` |
| `undefined` → `void 0` (and vice versa when `void 0` is longer) | `OXC/peephole/substitute_alternate_syntax.rs:1047-1065` (`substitute_return_statement`, drops `return undefined`); inlining path also emits `void 0`, `OXC/peephole/inline.rs` | `TERSER/compress/index.js:2992-2998` (`def_optimize(AST_Undefined,...)` → `make_void_0`) | raw | peephole | `return undefined;` → `return;` |
| Computed→dotted property access | `OXC/peephole/convert_to_dotted_properties.rs:17-44` | `TERSER/compress/index.js:3583-3609` (`def_optimize(AST_Sub,...)`, gated by `property.length <= prop.size()+1`) | raw + gzip (fewer distinct tokens) | peephole (terser's is a mini cost-comparison) | `foo['bar']` → `foo.bar` |
| Numeric string key → number | `OXC/peephole/convert_to_dotted_properties.rs:35-43` (`string_to_equivalent_number_value`) | `TERSER/compress/index.js:3591-3599` | raw | peephole | `a['0']` → `a[0]` |
| Shortest-form numeric literal (hex / exponent, terser-adapted) | `ECMA/number_literal.rs:8-84` — comment: *"Adapted from Terser's `get_minified_number`"* | `TERSER/output.js:2494-2517` (`make_num`, `best_of`) | raw | peephole (candidate-compare over 3 spellings) | `1099511627776` → `1e12`/hex/`0x100...` whichever is shortest |
| Quote-character choice (fewer escapes) | n/a (oxc's printer is a separate crate, not in these refs) | `TERSER/output.js:422-461` (`make_string`, `dq > sq ? quote_single() : quote_double()`) | raw | peephole (per-string char count) | `"it's"` → `` 'it\'s' `` avoided by picking `"`  |
| `Boolean(a)`/`Number(0)`/`String()`/`BigInt(1)` → primitive op | `OXC/peephole/substitute_alternate_syntax.rs:1095-1170` (`substitute_simple_function_call`) | (terser's constructor folding lives in `evaluate.js`/native-objects list) | raw | peephole | `Boolean(a)` → `!!a` |
| `new Object()`/`Object()`/`window.Object()` → `{}`; `Array()` → `[]` | `OXC/peephole/substitute_alternate_syntax.rs:1171-1324` | (terser's `evaluate.js` folds some constructor calls via native-objects table) | raw | peephole | `new Object()` → `{}` |
| `new Function()`/`new RegExp()` → call without `new` | `OXC/peephole/substitute_alternate_syntax.rs:1325-1366` | — | raw | peephole | `new RegExp(x)` → `RegExp(x)` |
| Template literal → string literal (no interpolation / no side effects) | `OXC/peephole/substitute_alternate_syntax.rs:1406-1414` (`substitute_template_literal`) | (terser's `TemplateString` optimizer, `TERSER/compress/index.js:3917-3993`) | raw | peephole | `` `abc` `` → `"abc"` |
| Array of string literals → `"a,b".split(",")` | `OXC/peephole/substitute_alternate_syntax.rs:1729-1790` (`substitute_array_expression`, fixed threshold `THRESHOLD: usize = 40`, comma delimiter only) | — | raw (only pays off above the threshold) | peephole with a hand-tuned length gate | `["a","b","c",...40+ items]` → `"a,b,c,...".split(",")` |
| `if(a)/else` → `a?x:y` conditional expression | `OXC/peephole/minimize_if_statement.rs:51-61` | `TERSER/compress/index.js:1102-1243` (`def_optimize(AST_If,...)`) | raw | peephole | `if(a)b();else c();` → `a?b():c();` |
| `if(a) x();` → `a&&x();`, `if(!a) x();` → `a\|\|x();` | `OXC/peephole/minimize_if_statement.rs:74-85` | same `AST_If` optimizer | raw | peephole | `if(a)b();` → `a&&b();` |
| `typeof x=="undefined"` → `x===void 0` / `typeof x<"u"` (unresolved ref) | `OXC/peephole/substitute_alternate_syntax.rs:215-259` (`substitute_typeof_undefined`) | terser's `typeofs` option, `TERSER/compress/index.js` binary-expr optimizer | raw | peephole | `typeof foo=="undefined"` → `typeof foo>"u"` |
| `a==null?b:a` → `a??b`; `a==null?undefined:a.b` → `a?.b` (optional-chain injection) | `OXC/peephole/minimize_conditional_expression.rs:308-372` | terser's `??`/`?.` compression is coarser and mostly limited to literal `a==null\|\|...` chains | raw | peephole | `a==null?b:a` → `a??b` |
| `foo==null` / `foo!=null` canonicalization of `void 0`/`undefined` comparisons | `OXC/peephole/substitute_alternate_syntax.rs:637-649` (`substitute_loose_equals_undefined`) | — | raw + gzip (canonical spelling reused across file) | peephole | `foo==void 0` → `foo==null` |
| `a=a||b` → `a\|\|=b`, `a=a+b` → `a+=b`, `a-=1` → `--a` | `OXC/peephole/minimize_conditions.rs:192-280` | `TERSER/compress/index.js` `AST_Assign` optimizer, `ASSIGN_OPS` table (~3040) | raw | peephole | `a=a+b` → `a+=b` |
| `()=>{return x}` → `()=>x` (arrow body reduction) | `OXC/peephole/substitute_alternate_syntax.rs:200-213` | terser's `opt_AST_Lambda`, `TERSER/compress/index.js:745` | raw | peephole | `()=>{return x}` → `()=>x` |
| IIFE simplification (empty body → `void 0`; single-expr body inlined) | `OXC/peephole/substitute_alternate_syntax.rs:1907-2020` | inlining machinery, `TERSER/compress/inline.js` | raw | peephole | `(()=>{})()` → `void 0` |
| Rotate associative binary/logical chains to expose further folds | `OXC/peephole/minimize_conditions.rs:19-60`, `substitute_alternate_syntax.rs:356-457` | terser relies on evaluate.js's constant folding for a subset of this | raw (enabling pass) | peephole | `a\|\|(b\|\|c)` → `(a\|\|b)\|\|c` |

---

## B. Statement / control-flow minimization

| Technique | oxc | terser | Size axis | Class | Example |
|---|---|---|---|---|---|
| `if(a)return b;return c;` → `return a?b:c;` (return/throw tail merge) | `OXC/peephole/minimize_statements.rs:106-197` (reverse-order merge loop in `minimize_statements`) | `TERSER/compress/tighten-body.js` `handle_if_return` | raw | peephole (bounded: `conditional_expression_count_exceeded`, cap 500, `minimize_statements.rs:216-227`) | `if(a)return 1;return 2;` → `return a?1:2;` |
| Nested-if merge: `if(a)if(b)x;` → `if(a&&b)x;` | `OXC/peephole/minimize_if_statement.rs:86-102` | `AST_If` optimizer | raw | peephole | `if(a)if(b)x;` → `if(a&&b)x;` |
| Empty-block / empty-statement removal, `{block}`→`block` | `OXC/peephole/remove_dead_code.rs:20-58` (`try_optimize_block`) | `TERSER/compress/index.js:706-744` (`AST_Block`/`AST_BlockStatement`) | raw | peephole | `{ }` unwrapped/dropped |
| Constant-condition `if` folding, incl. numeric-canonical test (`1`/`0` not `true`/`false`) | `OXC/peephole/remove_dead_code.rs:59-166` (`try_fold_if`) | same `AST_If` path, `evaluate.js` | raw | peephole | `if(true)a();` → `a();` |
| `for`/`while` → `for` normalization, dead-loop folding | `OXC/peephole/minimize_for_statement.rs:10-163`; `remove_dead_code.rs:178-254` (`try_fold_for`) | `TERSER/compress/index.js:965-968` (`AST_While`→`AST_For`), `:1065-1101` (`AST_For`) | raw (uniform shorter loop head) | peephole | `while(x)y` → `for(;x;)y` |
| `arguments` copy-loop → spread rewrite | `OXC/peephole/substitute_alternate_syntax.rs:661-1045` (`try_rewrite_arguments_copy_loop`, several Babel/TS shapes) | — | raw | peephole (pattern-matches several concrete emitted shapes) | `for(var e=arguments.length,r=Array(e),a=0;a<e;a++)r[a]=arguments[a];` → `var r=[...arguments];` |
| `var`-declaration joining across statements | `OXC/peephole/minimize_statements.rs:33-45` doc, `mod.rs` (`join_vars` gated), `CompressOptions.join_vars` (`options.rs:26-29`, default `true`) | `TERSER/compress/tighten-body.js` `join_consecutive_vars`, gated by `option("join_vars")` | raw | peephole | `var a;var b=1;` → `var a,b=1;` |
| Statement fusion via comma operator (per-shape detail in B.1 below) | `OXC/peephole/minimize_statements.rs:37-38` doc citing Closure's `StatementFusion.java`; the shapes are handlers at `:448-455,593-600,668-676,822-832,847-854,919-937,986-1023`; `CompressOptions.sequences` (`options.rs:31-36`, default `true`) | `TERSER/compress/tighten-body.js:1253-1286` `sequencesize`, `:1305-1373` `sequencesize_2`, both under `tighten_body` (`:228-254`); `compressor.sequences_limit` (default `800` when `sequences==1`, `TERSER/compress/index.js:330`) | raw-neutral alone; Brotli through the folds it unblocks | peephole | `a();b();` → `a(),b();` |
| Dead-code-after-jump elimination | `OXC/peephole/minimize_statements.rs:53-90` (`is_control_flow_dead` tracking) | `TERSER/compress/tighten-body.js` `eliminate_dead_code` | raw | peephole | code after unconditional `return`/`throw` dropped |
| Switch-case minimization/fallthrough merge | `OXC/peephole/minimize_statements.rs:572` (`can_switch_case_be_inlined`) | `TERSER/compress/index.js:1244-1633` (`def_optimize(AST_Switch,...)`) | raw | peephole | adjacent identical `case` bodies merged |
| try/catch simplification | (part of the combined traversal, `remove_dead_code.rs:384-427`, `try_fold_try`) | `TERSER/compress/index.js:1634-1649` | raw | peephole | empty `catch{}` after non-throwing `try` block simplified |
| Sequence-expression folding / `remove_sequence_expression` (drops dead-value commas, hoists side-effect-only eval) | `OXC/peephole/remove_dead_code.rs:471-521` | `TERSER/compress/index.js:2053-2103` (`def_optimize(AST_Sequence,...)`) | raw | peephole | `(a(), b())` with unused result → `a(),b()` |
| Statement-level dead-code / unreachable-expression removal (side-effect-free expr statements dropped) | `OXC/peephole/remove_unused_expression.rs:20-1192` (`remove_unused_expression`, `esbuild`'s `SimplifyUnusedExpr`) | `TERSER/compress/drop-side-effect-free.js` (394 lines) | raw | peephole | `1+1;` (statement position) → removed |

### B.1 Statement fusion / boundary absorption (harvest 2026-09-02)

Folding a preceding expression statement into the next statement's expression slot. **terser**:
`tighten_body` (`TERSER/compress/tighten-body.js:228-254`, fixed point, at most 10 rounds) runs
`sequencesize` (`:1253-1286`) then `sequencesize_2` (`:1305-1373`) whenever `sequences_limit>0`
(`TERSER/compress/index.js:330`, default 800 members per run); the absorbed `prev` is only ever an
`AST_SimpleStatement` (`:1371`), so a `var`, a declaration, a label or a block breaks the chain.
**oxc**: there is no `statement_fusion.rs` in 0.147.0; the Closure `StatementFusion.java` reference
is a doc comment (`OXC/peephole/minimize_statements.rs:37-38`) and every shape is a handler of
`minimize_statements` (`:50-232`, one forward pass per statement list) that absorbs
`result.last_mut()` only when it is an `ExpressionStatement`, through `join_sequence` (`:318-339`),
under `CompressOptions.sequences` (`OXC/options.rs:31-36`, default `true`). **LilScript**: textual.
`fold_adjacent_expression_statements[_at]` (`LS/js_peephole/folds/calls.rs:132-229`) rewrites one
`;` to `,` when both neighbours are expression statements (`:221-225`, `is_expression_statement_span`
`:978-1006`) and refuses whenever the token after the `;` is a statement keyword (`:183-209`), so
every absorbing shape below is refused at the `;`. Neither competitor refuses `yield`/`await` (both
are legal comma members; no such check in either file); ASI and precedence (`for(k in(e,o))`,
`for((a in b);;)`) are left to their printers, which LilScript's textual layer must spell itself
(`spans_line_terminator` bails at `control.rs:1977,2360`). Each row is raw-neutral alone (035 Lead
1); the value is the last row.

| Shape | oxc | terser | LilScript | Verdict |
|---|---|---|---|---|
| `a;b` → `a,b` | `minimize_statements.rs:448-455` (`handle_expression_statement`) | `tighten-body.js:1253-1286`: a declarations-only `var` or a function declaration sits inside a run without breaking it and the joined sequence is emitted after it (`:1274-1276`, hoisting); non-first members pass `drop_side_effect_free` (`:1270-1271`); run cap `:1267` | in-block canonical (`calls.rs:132-135`; `LS/js_peephole/mod.rs:2476,2529,2532`); top-level once at the very end because a comma would hide statement-shaped patterns from the other folds (`calls.rs:137-146`, `mod.rs:2552`); refuses keyword-led neighbours `:183-209`, a directive prologue `:212-220`, a `;` inside a `for` header `:159-181`; no hop over `var`/`function`; no run cap | **PRESENT** |
| `e;return x` → `return e,x` | `:822-832` (`handle_return_statement`); the reverse move `:803-807` sinks a side-effectful `undefined`-valued argument into the previous statement and leaves `return` bare; a bare `return` never absorbs | `:1316-1317` (`AST_Exit`, so return and throw); a bare `return` gets `void 0` synthesised and `AST_Return` drops the undefined tail again (`index.js:3868-3873`, `is_undefined` `inference.js:315-323`) | `fold_expression_suffix_returns` `LS/js_peephole/folds/control.rs:1913-1927`: `E1;E2;return V` → `return E1,E2,V`, block-terminal returns only (`:1974`), line-terminator bail `:1977`, directive bail `:1989-1995`; **search-only** per-site variants (`:1929-1948`; `mod.rs:2625,2681` `objective_only`; `compiler.rs:8130-8139` `TERMINAL_LOCAL_PASSES`) and only when `terminal_local_rounds>0`: 6 on the finalist path `compiler.rs:6012`, 0 at `:5920,5998,15575`, each round charged to the terminal codec budget (`:8153`) | **PARTIAL** (exists, never canonical; 035's probe recorded "no") |
| `e;if(c)S` → `if(e,c)S` | `:668-676` (`handle_if_statement`, unconditional) | `:1342-1343` (unconditional) | `fold_if_prefix_guard_return` `control.rs:412-490`: `if(C){P;if(D)return R}` → `if(C&&(P,D))return R` (single-expression prefix `:471-476`, no statement keyword in it `:466-468,492-508`); codegen `parse_assignment_guard_return` `LS/codegen_ir_js.rs:17198-17226` at `:10578-10592`: `t=v;if(f(t))return t` → `if(C&&f(t=v))return t` (`t` read once in the condition `:17216`), which is esbuild/oxc single-use substitution (`minimize_statements.rs:1149`), not fusion; `LS/js_peephole/tests.rs:428` forbids sinking a sequence member into a condition | **PARTIAL** (guard-return shapes only; `calls.rs:185` refuses the general case) |
| `e;for(i;c;u)` → `for(e,i;c;u)`, `e;for(;c;)` → `for(e;c;)` | `:919-937` (expression or empty init; a `var` prev merges into a `var` init `:938-963`); no `in`-operator check in the minifier | `:1318-1337`: refuses a declaration init `:1319`; refuses when `prev` holds an `in` binary outside a nested scope `:1320-1329` (`for(a in b;;)` misparses); an empty init takes `prev` whole `:1332-1335`; `while` is already `for` (`index.js:965-968`) | `fold_prior_assign_into_for_init` `LS/js_peephole/folds/loops.rs:2104-2259`: only a run of bare `name=rhs;` statements (`LS/js_peephole/rewrite.rs:396-414`), into an empty init `:2122-2148` or a cheap-literal init sharing the rhs `:2150-2195`; declaration inits refused `:2123-2126`; no `in` check on the rhs; `while` is never normalised to `for` (codegen emits `while(` at `codegen_ir_js.rs:11025,11107`) | **PARTIAL** (bare assignments only; no `in` guard) |
| `e;switch(x)` → `switch(e,x)` | `:593-600` (`handle_switch_statement`) | `:1344-1345` | none; `calls.rs:196` refuses `switch` after `;` | **ABSENT** |
| `e;for(k in o)` → `for(k in(e,o))` | `:986-1023`: bare or `var` left only, and no side-effectful Annex-B initializer (`:990-1013`); `let`/`const` refused for shadowing (oxc#18650, `:998`); no for-of rule (`:1055-1095` only does `var a;for(a of b)`) | `:1338-1341` (`AST_ForOf` extends `AST_ForIn`, `TERSER/ast.js:526-540`, so `for(k of(e,o))` too); `let`/`const` heads refused `:1339` | none; `calls.rs:187` refuses `for` after `;` | **ABSENT** |
| `e;throw x` → `throw e,x` | `:847-854` (`handle_throw_statement`) | `:1316-1317` (same `AST_Exit` branch) | none; `calls.rs:194` refuses `throw` after `;` | **ABSENT** |
| Value: what fires once the boundary is gone | every handler first runs `substitute_single_use_symbol_in_statement` (`:1149`, esbuild's) on the joined expression; `minimize_if_statement.rs:13` (`try_minimize_if`) and the reverse if/return merge (`:114-197`) need single-expression bodies; `remove_dead_code.rs:471-521` prunes the sequence | `AST_If` `index.js:1156-1158` (both arms simple → `?:`), `:1166-1197` (`if(c)a(),b()` → `c&&(a(),b())` or the negated `\|\|`), `:1199-1208` (`return c?(a,b):d`), `handle_if_return`; `lift_sequences` `index.js:2091-2101,2217-2245,3078,3235-3240` pulls the prefix out of unary/binary/assign/conditional operands; `AST_Sequence` `:2053-2089` drops side-effect-free members and `void 0` tails; `collapse` `tighten-body.js:278` treats sequence members as candidates (`:713-714`) and `find_stop` walks through them (`:774-776`) | the in-block `a,b` feeds `fold_if_expression_to_and`, `fold_expression_branches` and `fold_conditional_return_tails` (`control.rs:1470`) in the `mod.rs:2476-2552` order; nothing feeds the five absorbed slots, so their downstream folds never see the shape | 035 band 95–359 Brotli per port; micromark `;` −1498, `,` +932, `if(` −299 (030, status) |

---

## C. Constant folding and known-method replacement

| Technique | oxc | terser | Size axis | Class | Example |
|---|---|---|---|---|---|
| Arithmetic/comparison constant folding | `OXC/peephole/fold_constants.rs` (1212 lines); model in `ECMA/constant_evaluation/mod.rs:33-68` | `TERSER/compress/evaluate.js` (530 lines) | raw | peephole (pure evaluation, no search) | `1+2` → `3` |
| String method folding: `charAt`/`charCodeAt`/`indexOf`/`lastIndexOf`/`slice`/`substring`/`trim*`/`toUpperCase`/`toLowerCase`/`fromCharCode` | `ECMA/constant_evaluation/call_expr.rs:77-98` dispatch table | `TERSER/compress/native-objects.js` pure-native table + `evaluate.js` | raw | peephole | `"abc".charAt(0)` → `"a"` |
| `Math.*` folding: `pow`/`sqrt`/`cbrt`/`abs`/`ceil`/`floor`/`round`/`min`/`max`/`imul`/`clz32`/… | `ECMA/constant_evaluation/call_expr.rs:88-96,448-528`; `Math.pow` specifically rewritten to `**`, `OXC/peephole/replace_known_methods.rs:59-91` | `TERSER/compress/native-objects.js`, `evaluate.js` | raw | peephole | `Math.pow(a,b)` → `+(a)**+b` |
| Known property access folding (`Number.MAX_VALUE`, etc.) | `OXC/peephole/replace_known_methods.rs:371-471` (`replace_known_property_access`) | `TERSER/compress/native-objects.js` static-property table | raw | peephole | — |
| Array/string index-into-literal folding | `OXC/peephole/replace_known_methods.rs:541` doc (`"abc"[0]` → `"a"`) | `evaluate.js` | raw | peephole | `[0,1,2][1]` → `1` |
| `[].concat(a).concat(b)` chain merge | `OXC/peephole/replace_known_methods.rs:116-199` (`replace_concat_chain`) | — (no direct equivalent found in `native-objects.js`/`evaluate.js`) | raw | peephole | `[].concat(a).concat(b)` → `[].concat(a,b)` |
| `Array.of(...)` → array literal | `OXC/peephole/replace_known_methods.rs:91-107` | — | raw | peephole | `Array.of(1,2)` → `[1,2]` |
| `[].concat(1,2)` → `[1,2]`; `"".concat(a,"b")` → `` `${a}b` `` | `OXC/peephole/replace_known_methods.rs:200-357` | — | raw | peephole | as shown |
| `String.fromCharCode`/`Number()`/`BigInt()` constant call folding | `ECMA/constant_evaluation/call_expr.rs:87-88` | `TERSER/compress/evaluate.js` | raw | peephole | `String.fromCharCode(97)` → `"a"` |

---

## D. Inlining, single-use substitution, and variable collapsing

| Technique | oxc | terser | Size axis | Class | Example |
|---|---|---|---|---|---|
| Single-use variable inlining into its one read site (esbuild's `substituteSingleUseSymbolInStmt`/`Expr`, explicitly cited) | `OXC/peephole/minimize_statements.rs:1137-1330` | esbuild-only pattern; terser's nearest analogue is `collapse_vars` below | raw | peephole, single-pass forward scan, not a search | `let x=fn();return x.y();` → `return fn().y();` |
| Constant-value propagation for hoisted `var`s during the "declarative prelude" | `OXC/peephole/inline.rs:104-153` (`inline_identifier_reference`, symbol-value cache) | `TERSER/compress/reduce-vars.js` (864 lines) | raw | peephole (O(1) cached lookup, not re-derived) | `var x=1;f(x)` → `f(1)` when provably safe |
| Right-to-left `collapse_vars`: move an assignment into its first subsequent use | — (`substitute_single_use_symbol_*` above is oxc's rough equivalent) | `TERSER/compress/tighten-body.js:230-300+` (`collapse`, named "collapse_vars", capped `max_iter=10` per statement list, `:234,252`) | raw | **search-ish**: scans right-to-left for candidates, then left-to-right for first use, honoring `sequences_limit` | `var a=x; return a+a;` (single first use) → `return x+x;` when safe |
| Function-call inlining (small function bodies substituted at the call site) | not in `oxc_minifier`'s peephole set at all (oxc leaves general inlining to a separate `oxc_minifier` roadmap item / bundler) | `TERSER/compress/inline.js` (683 lines): `inline_into_symbolref`, `inline_into_call`; gated 0-3 by the `inline` option (`inline>=2` injects args, `inline>=3` injects vars, `TERSER/compress/inline.js:582-585`) | raw | search-like (multiple shape-specific inlining strategies tried per call site) | small non-recursive function body substituted into its one call site |
| Whole-tree `best_of`: build both the rewritten and original candidate, keep the smaller by AST-size proxy | n/a (oxc rewrites unconditionally per rule; no candidate-compare step) | `TERSER/compress/common.js:170-190` (`best_of_expression`, `best_of`), backed by `AST_Node.prototype.size()` (`TERSER/size.js:93-108`) | raw (proxy, not real gzip/brotli bytes) | **search**: constructs and measures two full candidate ASTs | used pervasively, e.g. bracket-vs-dot property choice at `TERSER/compress/index.js:3600-3606` |
| Property-read purity gate: what must be proven before `o.k` is droppable, movable, or safe to duplicate (013/019's `unstable` mechanism) | `ECMA/side_effects/context.rs:7-13,44` (`PropertyReadSideEffects::{None,All}`), default `All` (`OXC/options.rs:176-180,221`). Under `All`, `property_access_may_have_side_effects` (`ECMA/side_effects/expressions.rs:460-503`) is pure only for a known global property (`ECMA/side_effects/known_globals.rs:454`), `.length` of an array literal or string-typed value, or an in-bounds integer index on a string/array literal (`:505-520`); a non-literal key is unconditionally impure (`:449-458`); private-field reads and destructuring patterns are impure (`:415-417`, `ECMA/side_effects/statements.rs:122-131`). No receiver-value tracking: `const o={a:1}; o.a` is impure by default. `None` waives everything, down to the Proxy-trap reasoning for `Object.keys`/`isFrozen` (`:735-770`) | Default `pure_getters: "strict"` (`TERSER/compress/index.js:261`): getter side effects are **waived by default**; the only hazard kept is a nullish receiver. `may_throw_on_access` (`TERSER/compress/inference.js:756-759`) → `_dot_throw` (`:765-830`) decides by receiver class — constant / array / getter-free object literal / class / function → safe; `null`, `undefined`, an `AST_ObjectGetter` → throws; `a.b` → throws unless `.prototype` of a function/class; `AST_SymbolRef` → recurse into `fixed_value()` from `reduce_vars`, unknown (`!fixed`) → throws. Feeds `has_side_effects(AST_Dot/AST_Sub)` (`:477-497`), `drop_side_effect_free` (`TERSER/compress/drop-side-effect-free.js:339-364`) and the `collapse_vars` scan stop (`TERSER/compress/tighten-body.js:398-399`). `pure_getters: true` additionally unlocks unused-destructure dropping (`index.js:4085`, `TERSER/compress/drop-unused.js:333`). `unsafe` is a separate axis: `_eval` reads through the receiver's evaluated value (`TERSER/compress/evaluate.js:397,422-470`) and native method calls become pure (`inference.js:993-1010`, `TERSER/compress/native-objects.js:92`) | raw (unlocks dropping and moving) | analysis precondition, not a rewrite | `var x=o.k;f(g(),x)` → `f(g(),o.k)` only when `o.k` may cross `g()` |
| Object literal → scalars (terser `hoist_props`; Closure `InlineObjectLiterals` / `CollapseProperties`, not vendored — verify) | **ABSENT as a pass** in `oxc_minifier` 0.147: `ObjectExpression` appears in the peephole only as spread-literal inlining (`OXC/peephole/fold_constants.rs:882-980`), unused-literal dropping (`OXC/peephole/remove_unused_expression.rs:435-480`) and single-use substitution into literal values (`OXC/peephole/minimize_statements.rs:1745-1790`). Degenerate case only: `remove_unused_member_assignment` (`OXC/peephole/remove_unused_expression.rs:814-850,990-1004`) deletes `o.x=v` when `o` is `FreshValueKind::Object` — a literal with no get/set/`__proto__` (`OXC/peephole/inline.rs:163-186,233-260`) — and every reference is a member-write target, so a never-read object dissolves to nothing | `hoist_properties` (`TERSER/compress/index.js:883-948`, on by default `:245`): `var o={a:..,b:..}` → `var o_a=..,o_b=..` iff `o` is declared in this scope, `def.escaped != 1` (never assigned, passed, returned or yielded whole: `mark_escaped`, `TERSER/compress/reduce-vars.js:228-270`), never reassigned, `!def.direct_access` (every reference is `o.key` for a `key` that is an `AST_ObjectKeyVal` of the literal — `read_property`, `TERSER/compress/common.js:207-227`, returns nothing for a getter/setter/method, which sets `direct_access` at `reduce-vars.js:269`), not single-use, not exposed, no spread or computed key. Also `flatten_object` (`index.js:3534-3581`): `{a:1,b:2}.b` → `[1,2][1]` when every prop is a plain non-computed key-val | raw + codec (reads vanish, names shrink) | whole-scope rewrite gated on an escape proof | `var o={a:1,b:2};f(o.a,o.b)` → `var a=1,b=2;f(a,b)` |

### D.1 Single-use assignment collapsing and unused-binding dropping (harvest 2026-09-02)

**terser.** `collapse_vars` is one routine, `collapse` (`TERSER/compress/tighten-body.js:278-970`),
run inside `tighten_body`'s fixed point (`:233`, at most 10 rounds). Candidates are harvested per
statement, right to left (`extract_candidates` `:653-731`): `x=E` with a side-effect-free target
and no optional chain in `E` (`:655-660`), `var`/`let` declarators with a value (the last 200 of a
list, `:675-680`, `:719-724`), `++x`/`x--` (`:713-717`); with `unused` on, an IIFE's arguments become
declarators (`extract_args` `:591-651`). Each candidate is scanned forward, in tree order, from its
own position through the *following* statements (`:494`) for the first read of `x` (`:347-390`);
that read receives the assignment `(x=E)`, or the value itself when the candidate is a declarator
with exactly one remaining reference that is not exposed (`:374-380`; more references → `var x;` and
`(x=E)` at the first read, `:381-386`); a declarator whose value is a declared identifier is instead
aliased away at every reference (`mangleable_var` `:787-797`, `multi_replacer` `:423-451`). The
target must not be `const`/`let`/`using`, `this`, a lambda name or a property of a constant
(`get_lhs` `:799-831`, `is_lhs_read_only` `:158-176`). A read reached inside a `&&`/`||`/`??` right
operand, a `?:` arm or an `if` body entered after the candidate aborts unless the target is local to
this function and this read is its only other reference (`stop_if_hit` `:340-346, 354-357`,
`is_lhs_local` `:893-904` — also refused inside a loop for `++`, compound and self-reading
candidates, `replace_all_symbols` `:912-924`). Hard aborts (`:300-338`): any other write to `x`
(`lvalues` `:473-476`, `:405-407`), `await`/`yield`/`using`, optional chains, `try`/`with`, a class,
an export, destructuring, `break`/`continue`, any loop other than a `for` init (`:318, 325`), an
undeclared global read unless `replace_all` (`:328-332`), `_NOINLINE`. Soft stops (`:392-413`: the
node's children are still scanned, then abort): every call; `return`/`throw` when `E` has side
effects or `x` is a property or modifiable from another scope (`may_modify` `:926-937`); a property
access when `E` has side effects or the receiver may throw; a read of a name `E` writes; a write to
a property or to a name `E` reads; and, when `E` may throw, any externally visible side effect
(`side_effects_external` `:939-958`, stricter inside `try`). Function bodies are never entered
(`:410, 518`). On success the candidate is removed (`remove_candidate` `:855-891`; `const` keeps
`void 0`). `unused` is `drop_unused` (`TERSER/compress/drop-unused.js:112-507`): in-use scan
(`:144-214`), initializer closure (`:216-224`), then the drop pass (`:227-497`).

**Census** (a scratch copy of Terser 5.50.0's `lib/` with a log at the replacement point
`tighten-body.js:367` and env-gated candidate classes in `extract_candidates`, `minify(src,
{module:true, compress:{…}, mangle:false})`, sizes by `lilscript-codec`; the instrumented outputs
were byte-identical to unpatched Terser). Sites on our artifacts, as
same-statement `x=E,…x` + `x=E;…x` + declarators + `x--`: micromarklil `dist/micromark.raw.js`
(the compiler's file) **244** = 184 + 11 + 42 (23 `let`, 19 `var`; 18 with a call in the
initializer) + 7; jquerylil `dist/jquery.esm.js` **445** = 381 + 30 + 27 + 6; mobxlil
`dist/mobx.esm.js` **231** = 178 + 9 + 40 + 4; markedlil `dist/marked.esm.js` **141** = 117 + 4 +
13 + 7. Nothing observable — no call, `new` or member read — completes between the candidate and
its read at 243/244, 444/445, 231/231 and 140/141 of those sites (Terser's own soft stops make the
read the leftmost observable evaluation); 71 / 212 / 109 / 93 sites cross nothing at all (the read
is the next leaf). Brotli cost of removing one option or one candidate class from Terser, compiled
file (micromark also through an esbuild `minifySyntax` re-print, its shipped step): all of
`collapse_vars` **+256 / +367** (043: +280 shipped), **+136**, **+56**, **+60**; assignment
candidates only +82 / +155, +20, +4, +6 (raw +267, +444, +198, +81: `a=E,`+`a` = `(a=E)` is
raw-neutral for a one-letter name and pays through the dead-store drop that follows); declarator
candidates only +119 / +151, +29, +62, +40 (raw +310, +200, +256, +85); `x--` only −29 (noise);
`unused` **+180 / +240**, **+94**, **+132**, **+29**; both +272 / +412. `unused` shapes on our
files (beautified diffs, regex counts): micromark — 12 dead bindings with an observable initializer
kept as bare reads (`var h=b.enter,…` → `b.enter,…`), 9 fresh-literal temporaries (`var i={…};
f(i)` → `f({…})`), 8 single-use function values moved to their one call, 10 trailing unused
declarator names, the string pool un-aliased (`yb=""` 21 uses, `zb='"'` 8 uses, 010/011's class),
and 23 `let a;` residues of Terser's own collapse; jquery — 32 dead-store hunks (`l=x=l.lastChild`
→ `l=l.lastChild`, `var i=null;f(i,i)` → `f(null,null)`), 4 never-referenced arrow declarators
(`li`,`oi`,`ui`,`si`), 3 single-use functions, 3 literal temporaries, 3 parameter trims; mobx — 35
dead-store hunks, 5 single-use functions, 3 never-referenced function declarators, 3 bare reads.

| Shape | oxc | terser | LilScript | Verdict |
|---|---|---|---|---|
| `x=E,…,x` → `…,(x=E)` (same statement: a sequence member into its first read) | **ABSENT**: `substitute_single_use_symbol_in_statement` (`OXC/peephole/minimize_statements.rs:1149-1190`) consumes only a *preceding `VariableDeclaration`* (`stmts.last_mut()`, `:1160`); a bare assignment is only ever dropped, never moved (`remove_unused_assignment_expr`, `OXC/peephole/remove_unused_expression.rs:750-800`, and only with no reads at all) | Assign candidate (`:655-660`) + the scan above; 184/244 micromark, 381/445 jquery, 178/231 mobx, 117/141 marked sites; the receiver-position shape `i=a[1],i.t` → `(i=a[1]).t` → (`unused`) `a[1].t` is the emitter's stored `unstable` read undone (013) | `fold_sequence_assignments_into_first_use` (`LS/js_peephole/folds/copies.rs:1040-1137`): only a parenthesised two-member `(x=E,t)` (`:1059-1069`), a prefix of literals and pure operators with no identifier at all (`:1071-1084`, `sequence_assignment_inert_prefix_token` `:1382-1406`), outer parentheses droppable only after `:`/`?`/`return`/`=>`/`&&` (`:1113-1124`); beam-only (`LS/js_peephole/mod.rs:2804-2806`, `LS/compiler.rs:8091-8092`), never canonical. `fold_statement_assignments_into_first_use` refuses any candidate after a `,` (`statement_start`, `copies.rs:1240-1249`) | **PARTIAL** (beam-only; ≈0 of the 184 shapes; 043 counted 0 sites) |
| `x=E;…x` → `…(x=E)` (across a statement boundary) | **ABSENT** (same reason) | Assign candidate scanned into `statements[i+1..]` (`:494`); 11 / 30 / 9 / 4 sites, 2 of them two statements away | `fold_statement_assignments_into_first_use` (`copies.rs:1185-1380`): statement-initial `x=E` only (`:1240-1249`), never inside a declaration (`:1251-1270`), read in the very next `,`/`;` unit, prefix admitted by `prefix_cannot_observe` (`:1155-1183`, applied `:1333`) — refuses a member read, a call, a foreign `y=F`, a template, but **accepts** `&&`/`\|\|`/`??`/`?`/`:` (`:1178-1179`) and `while` (`:1166`) where Terser aborts (`:318, 340-357`): `x=R;return a&&x` → `return a&&(x=R)`, `i=0;while(i<c…` → `while((i=0)<c…` (a live shape in `micromark.raw.js`); no `may_modify` check when an effectful `E` crosses an identifier read (`:1163`); beam-only (`mod.rs:2807-2809`, `compiler.rs:8081-8084`) | **PARTIAL** (beam-only, and unsafe as written) |
| `let x=E;S(x)` → `S(E)` (single-use declarator substituted: effectful `E`, mid-list declarators, a read inside an object literal or a call argument) | **PRESENT** (esbuild's algorithm): `:1149-1190` chains through consecutive preceding declarations; `substitute_single_use_symbol_within_declaration` (`:1192-1230`) within one list, right to left (`rposition` `:1245`); refuses script root / direct `eval` (`:1155`), `using` (`:1162`), catch variables (`:1263-1265`), kept names (`:1267`), observable, multiply-read or ever-written symbols (`:1270-1274`), non-literal values into block-scoped loop heads (`:1276-1279`); the walk (`:1305-1730`) stops at an identifier read that may reorder (`identifier_read_blocks_reorder`, `OXC/peephole/mod.rs:236`), an effectful assignment target (`:1427`), a compound assignment or member target when `E` has effects (`:1436-1470`), enters `&&`/`?:`/optional-call arguments only when `E` is pure (`:1506-1541, 1587, 1628`), never a callee that would change `this` (`:1613, 1666`), never past a spread (`:1645`); called from every statement handler (`:361, 422, 586, 665, 794, 840, 873-888, 978, 1061`) | VarDef candidate (`:719-724`), value substituted at `:374-380`; 42 / 27 / 40 / 13 sites (35 of micromark's into the next statement, 7 within the list); Terser's right-to-left order keeps a multi-declarator collapse (`let a=b.x,i=b.y;return{p:a,q:i}`) getter-order-preserving | `fold_single_use_temporaries` (`LS/js_peephole/folds/returns.rs:322-422`, canonical `mod.rs:2633`): whole-statement `var`/`let` only (`:339`), initializer a pure read (`is_pure_read` `:459-507`, applied `:359` — no call, `[`, `{`, `,`), exactly one read (`:375`) in the next statement and scope (`:391`), nothing evaluated before it (`nothing_runs_before` `:509-527`, applied `:394` — no `.`, `[`, `)`, `]`, update, `new`, assignment); `fold_returned_temporaries` (`:22-91`: `x=E;return x`, also the last declarator of a list); `fold_single_use_literal_bindings` (`copies.rs:795-1038`: literals and regexes) | **PARTIAL** (pure reads into the next statement; 36 of micromark's 42 refused — 18 calls, the rest lists and literal shapes) |
| `var x=E;…x…x` → `var x;…(x=E)…x` (multi-reference declarator: the first read takes the assignment) | **ABSENT** (`has_multiple_reads` refuses, `:1271`) | `:381-386`; 3 jquery sites | none | **ABSENT** |
| `var a=b;…a…` → `…b…` (alias declarator replaced at every reference) | `inline_identifier_reference` (`OXC/peephole/inline.rs:104-153`, constant values) | `mangleable_var` `:787-797`, `multi_replacer` `:423-451`, `replace_all_symbols` `:912-924`; 0 / 1 / 0 / 0 sites left on our files | `fold_identifier_copies` (`copies.rs:16-324`, guards `:326-515`, canonical ×8 `mod.rs:2500`) | **PRESENT** (013: the refusals left are the legal ones) |
| `x--;if(x<0)` → `if(--x<0)` (update into its comparison) | none | Unary candidate (`:713-717`), prefix spelling at `:368-370`; 7 / 6 / 4 / 7 sites | `fold_void_prefix_updates` (`LS/js_peephole/folds/loops.rs:1910`) and `fold_index_postfix_updates` (`:16`) re-spell an update in place; neither moves it into the following comparison | **ABSENT** (raw-neutral; −29 Brotli when removed from Terser on micromark, i.e. noise) |
| IIFE arguments treated as declarators and collapsed into the body | `substitute_iife_call` (`OXC/peephole/substitute_alternate_syntax.rs:1915`) for empty / single-expression bodies only | `extract_args` `:591-651` (needs `unused`; no `arguments`, no reassigned or shadowing parameter, the argument is zeroed by `remove_candidate` `:856-866`) | `fold_value_binding_iife` (`mod.rs:2481`), IR-level inlining | **PARTIAL** (0 sites in the four artifacts: the emitter does not write parameterised IIFEs) |
| Unused declarator with an observable initializer → bare expression (`var h=b.enter;` → `b.enter;`) | `remove_unused_variable_declaration` (`OXC/peephole/remove_unused_declaration.rs:157-178`) takes init-less declarators (`KeepVar`); an initializer with effects survives through `remove_unused_expression` (`OXC/peephole/remove_unused_expression.rs:21-50`; a member read is impure under `PropertyReadSideEffects::All`) | `drop-unused.js:383-387`: `drop_side_effect_free` keeps `b.enter` (`pure_getters:"strict"` — it may throw) and it is emitted as a statement (`:362-370, 394-398`); 12 micromark, 1 jquery, 3 mobx sites | `remove_unused_standalone_vars` (`LS/js_peephole/folds/declarations.rs:852-951`) needs an inert initializer (`declarator_initializer_is_inert` `:962-985`: none, one literal, `void 0`); `strip_unused_simple_declarators` (`:253-400`) likewise | **ABSENT** (legal under §7: the read stays, the binding goes) |
| Dead store with an effectful right side (`x=E`, `x` never read again → `E`) | `remove_unused_assignment_expr` (`:750-800`): only when the symbol has *no* reads anywhere | `drop-unused.js:232-247`: `!in_use`, or a `fixed_ids` assignment that is not the fixing one, keeps the right side alone (`maintain_this_binding` `:242`); 32 jquery / 35 mobx hunks | `fold_dead_pure_identifier_assigns` (`declarations.rs:1487-1615`), `fold_dead_identifier_copy_declarators` (`:1617-1659`), `fold_dead_increment_snapshots`; SSA `eliminate_dead_control_flow_instructions` (`LS/optimizer.rs:9934-9993`) before any name exists | **PARTIAL** (pure right sides only at the peephole; the emitter's `unstable` stores are exactly the effectful ones, 013) |
| Unused parameters trimmed | **ABSENT** (no parameter removal in `remove_unused_declaration.rs`) | `keep_fargs:true` by default (`TERSER/compress/index.js:252`): only an IIFE's own parameters (`drop-unused.js:262-289`) plus trailing call arguments to `UNUSED`-flagged parameters (`index.js:1717-1729`); 3 jquery / ≈11 mobx hunks | `optimize_unused_parameters` (`LS/optimizer.rs:4542-4611`) | **PRESENT** at the IR (the survivors are FFI-shaped, 021) |
| Single-use function value moved to its one call (`f=function(){…};o.k=f` → `o.k=function(){…}`); never-referenced function declarators dropped | the declarator substitution above covers `const f=…;f()`; `remove_unused_function_declaration` (`remove_unused_declaration.rs:180-196`) drops dead ones | `inline_into_symbolref` (`TERSER/compress/inline.js:191-260`; `single_use` from `reduce-vars.js:757-766`; requires no side effects / may-throw `:199-203`, same scope or a non-escaping constant expression `:209-241`), then `unused` drops the declaration (`drop-unused.js:306-312`, `:383-387`); 8 / 3 / 5 inlined, 4 jquery + 3 mobx dead declarators dropped | `fold_single_use_function_values` (`copies.rs:1547-1700`, `run_if(elide_functions)` `mod.rs:2503-2589`), `fold_single_use_function_expressions` (beam); SSA `eliminate_dead_functions` (`optimizer.rs:11650-11687`) | **PARTIAL** |
| Fresh literal temporary into its one use (`var o={…};f(o)` → `f({…})`) | the declarator substitution above (any value; `is_literal_value` gate only for loop heads, `:1276-1279`) | `inline_into_symbolref` single-use path (`inline.js:199-203`: a literal with pure members has no side effects) + `unused`; 9 micromark, 3 jquery, 0 mobx sites | `is_pure_read` refuses `{`/`[` (`returns.rs:459-507`); the emitter can defer `Array`/`Record`/`Struct` (`op_can_defer`, `LS/codegen_ir_js.rs:24703-24750`), so these stores come from the `cross_block` / unstable-dependency buckets (`unstable_values` `:20295-20330`, store decision `:19447-19476`, `record_store_reason` `:25133-25160`; 013's census) | **ABSENT** at the peephole (legal: an allocation is unobservable) |

---

## E. Name mangling and property mangling

| Technique | oxc | terser | Size axis | Class |
|---|---|---|---|---|
| Property-name mangling, 3-phase: collect → assign → rewrite | `LS/property_mangler.rs:1-9` doc, `collect`/`assign`/rewrite methods | `TERSER/propmangle.js:216-350` (`mangle_properties`, single walk-then-transform) | raw + gzip (short reused names) | search over name-length assignment |
| Assignment order: **descending occurrence frequency**, lexical tie-break | `OXC/property_mangler.rs:404-436` (`assign`: `names.sort_unstable_by` on `count_b.cmp(count_a)`) | **absent** — terser assigns in first-encounter tree-walk order (`cname` incremented per new name seen, `TERSER/propmangle.js:237,388-406`), not by frequency | raw (shorter code for hotter names) | oxc: one sort + greedy assign; terser: no sort |
| Output alphabet: fixed `base54` (`a-zA-Z$_` then `+digits`) | uses `oxc_mangler::base54` (not vendored in these refs; only the caller is visible, `OXC/property_mangler.rs:15,435`) | `TERSER/scope.js:1046-1062` (`base54`), alphabet order can be static or char-frequency-sorted | raw | — |
| Reserved/unmangleable name safety list (DOM props, hard-reserved keys) | `is_hard_reserved`, `PROPERTY_MANGLE_CACHE` validation, `OXC/property_mangler.rs:33-92` | `TERSER/propmangle.js:79-120` (`find_builtins`, pulls in `tools/domprops.js`) | correctness gate, not size | — |
| Variable (identifier) mangling | separate `oxc_mangler` crate, not vendored here | `TERSER/scope.js:808-895` (`mangle_names`), per-scope sequential `cname`, **not** reference-count sorted | raw | terser: no frequency sort at the variable level either |
| **Char-frequency-driven mangling alphabet** (gzip/brotli-aware) | not present in the vendored `oxc_minifier`/`oxc_ecmascript` sources | `TERSER/scope.js:976-1045` (`compute_char_frequency` + `base54.sort`): prints the whole program once, counts non-identifier character frequency, and orders the base54 alphabet so single-char mangled names reuse the program's already-common characters | **gzip/brotli-friendliness** (this is Section F material duplicated here because it's implemented as part of the mangler) | one full-program print + counting pass, then a sort — cheap relative to compression |

**Local-variable renaming across function scopes** (harvest for 041; 035/036/039 cover top-level
allocation and shadowing, not this). `oxc_mangler` is a crate dependency
(`finer/refs/oxc_minifier-0.147.0/Cargo.toml:103-104`) and is not vendored: the refs hold only its
caller (`OXC/lib.rs:66,74,186-188`, `examples/mangler.rs:48-57,74`), so the oxc column below records
what the caller shows and nothing about slots or per-slot frequency, which live in the missing crate.

| Technique | oxc | terser | LilScript | Size axis |
|---|---|---|---|---|
| **Per-scope counter restart**: every function scope hands names out from the first alphabet character again, so sibling scopes converge on the same short spellings | not vendored (see above) | PRESENT: `TERSER/scope.js:502` (`cname=-1` per scope), `:696-730` (`next_mangled` walks `++scope.cname`) | PRESENT twice: IR backend `LS/codegen_ir_js.rs:6708-6774` (`local_mangler` clones the top-level mangler and `rewind()`s at `:6712,6727,6772`); text pass `LS/js_peephole/rename.rs:144` (`CanonicalNames::new` per scope) | brotli (one spelling repeated beats several) |
| **Collision rule = "enclosed"**: a scope refuses only names that are *referenced* from it or its inner scopes and resolve outside; shadowing an unreferenced outer binding is allowed | not vendored | PRESENT: `TERSER/scope.js:501` (`enclosed`), `:654-667` (`mark_enclosed`/`reference`), `:706,723-727` (the only outer-name check in `next_mangled`); reserved words `:710`, `options.reserved` incl. `arguments` `:714,804`, short `keep_fnames` names `:718,885-893` | PRESENT in the text pass: `LS/js_peephole/rename.rs:57-116` (blocked = free/unresolved reads, outer bindings resolved from inside, fixed function/class names `:250-258`). PARTIAL in the IR backend: `reserve_enclosing_js_bindings` (`LS/codegen_ir_js.rs:6776-6803`) reserves every enclosing binding *name* unless `precise_cross_scope_shadowing` releases the unreferenced ones (`:6736-6745`) — 036's ground, not re-measured here | brotli + raw |
| **Within-scope order: parameters first, by position**, so same-arity headers spell alike (`(e,t)`, `(e,t,n)`) | not vendored | PRESENT by construction, not design: `to_mangle` is filled per scope from `variables` in declaration order (`TERSER/scope.js:850-856`; `def_variable` insertion order `:681-694`, funargs before body declarations) and mangled in that order (`:895`); there is **no** use-count sort at any level (039 priced this at +52 on jquery's module scope) | ABSENT in the IR backend: values are ordered by (emitted, use count desc, id) (`LS/codegen_ir_js.rs:19512-19524`), coloured by interference (`:19546-19557`), named per colour in colour or weight order (`:19619-19635`), and parameters take their colour's name (`:19671-19680`) — position is never a key, so `(e,t)` and `(t,e)` both occur (039's 90 header spellings). PRESENT in the text pass: `LS/js_peephole/rename.rs:118-142` sorts `(parameter position, −uses, declaration)` — but see the gate row | brotli |
| **Block-scope granularity**: `let`/`const`/catch in sibling blocks reuse names, and a function binding one name twice is still renamed | not vendored | PRESENT: block scopes own a `cname`/`enclosed` (`TERSER/scope.js:496-503,854-856`); catch parameters reuse the function-scope var's name (`:196-201,183-186`); `AST_Function.next_mangled` keeps a funarg off the function's own name (`:745-760`) | PARTIAL: `BindingResolution` is function-granular (`LS/js_peephole/binding.rs:23-26`, `ScopeKind` `:54-58` = Module/Function/Catch); a name declared twice in one function is `ambiguous` (`:451,485`, comment `:456-459`), a parameter list containing `[`, `{` or `...` is unsound (`:550-553,579-586`), and either makes `is_total()` false for the whole artifact (`:191-196`) — `rename.rs:46-48` then rewrites nothing | brotli + raw |
| **Unconditional whole-program local rename as the last pass**: runs once after compression, neither budgeted nor voted | caller shape only: `Mangler::default().with_options(options).build_with_semantic(&mut semantic, program)` runs once after the peephole fixed point (`OXC/lib.rs:186-188`); opt-outs are options (`top_level`, `keep_names`, `reserved`, `examples/mangler.rs:48-57`) | PRESENT: `AST_Toplevel.mangle_names` (`TERSER/scope.js:808-908`) walks the tree once (`:883`) and mangles every collected def (`:895`); the only opt-outs are options (`unmangleable` `:155-173`) | PARTIAL: IR names are final unless `converge_local_names` (`LS/js_peephole/rename.rs:33-180`) both runs and wins. Caller `LS/compiler.rs:7752-7790`: inside `apply_late_javascript_cleanup` only (early return `:7680-7684` when `remaining()==0`), only when `mangle_identifiers` and cost model ≠ Raw (`:7758-7759`), one work unit per beam candidate else `break` (`:7763-7764`) plus one codec probe (`:7778-7782`), kept only when `cost < candidate.cost` (`:7783`), then must survive `truncate(BEAM_WIDTH)` (`:7985`). The first call runs in a fair slice of `allowance.min(8)` units (`:5997-5998`) of which the canonical peephole spends 2 (`:7712,7743`); the level-13 ledger is 384 probes scaled by artifact size (`LS/config.rs:1856-1881,1627-1656`, ~42-84 on jQuery `:1868-1869`) and shared with every earlier terminal family (`LS/compiler.rs:5170-5172`). The pass itself returns the source untouched on any template literal (`rename.rs:35-37`) or a non-total resolution (`:46-48`). No timing counter names it (`binding.rs:89` counts `BINDINGS` only) | brotli |
| **Which text feeds the frequency alphabet** for local names | not vendored | PRESENT: `compute_char_frequency` counts the printed program *minus* mangleable symbol names and (with `properties`) property names (`TERSER/scope.js:985-1004`) — the bytes that survive mangling | PRESENT, different set: `rename.rs:194-214` counts identifier tokens only, *including* the current mangled spellings, deliberately (`:38-43`, measured −350 for converging on letters the artifact does not already use); the IR backend offers `for_code` and `for_code_excluding_binding_characters` (`LS/codegen_ir_js.rs:1573-1597`) to the codec vote (`LS/compiler.rs:3630-3652`). 039: spelling is worth ≤ 40 either way | brotli (≤ 40 measured) |

---

## F. Explicitly gzip/brotli-aware techniques

| Technique | oxc | terser | Notes |
|---|---|---|---|
| Character-frequency alphabet for mangled names | absent | `TERSER/scope.js:976-1045`, `base54.consider`/`sort` | See Section E; the only *explicitly* compression-aware technique found in either vendored codebase — everything else in both oxc and terser optimizes **raw byte count** (or terser's AST-`size()` proxy) and trusts that gzip/brotli will do no worse on shorter code. Neither tool ever invokes an actual gzip/brotli encoder during compression decisions. |
| Repeated-token/backreference-affinity ordering (e.g. co-locating similar emitted shapes to help LZ77 windows) | absent | absent | Not found in either vendored source tree. |
| Real-codec-scored candidate search (build N candidates, actually gzip/brotli them, keep the smallest) | absent | absent | Neither tool does this; see LilScript's `cost_model`/candidate search in Section H, which is the one place in this whole comparison that does. |

---

## G. Cost/effort controls — how each tool bounds its own work

This is the section the brief says matters most, so the constants and loop conditions are quoted
verbatim with exact citations.

### oxc

- **One fixed-point loop over the whole combined traversal.** `Compressor::run_in_loop`,
  `OXC/compressor.rs:106-146`:
  ```rust
  fn run_in_loop(max_iterations: Option<u8>, program: &mut Program<'a>, ctx: &mut ReusableTraverseCtx<'a>) -> u8 {
      let mut iteration = 0u8;
      compression_pass::finish_normalize_pass(program, ctx.get_mut());
      loop {
          let outcome = compression_pass::run_peephole_pass(program, ctx);
          if !outcome.needs_another_pass { break; }
          if let Some(max) = max_iterations {
              if iteration >= max { break; }
          } else if iteration > 10 {
              debug_assert!(false, "Ran loop more than 10 times.");
              break;
          }
          iteration += 1;
      }
      ...
  }
  ```
  `max_iterations: Option<u8>` defaults to `None` (`OXC/options.rs:55-56`) — i.e. unbounded except
  for the hard-coded `iteration > 10` escape hatch, which fires unconditionally (the `debug_assert!`
  itself is a release no-op, but the surrounding `break` is not, so release builds are still capped
  at 11 passes by construction).
- **Convergence signal is per-pass, not per-rule.** A pass is "done" when
  `PassOutcome::needs_another_pass` is false — the OR of "any AST mutation happened" and "any
  function newly became dead" (`OXC/compression_pass.rs:287-305`). There is no separate counter per
  technique; all ~80 rules share one convergence flag because they all run inside one traversal.
- **No search / candidate-compare cost model anywhere in the peephole layer.** Every
  `substitute_*`/`minimize_*` function is an unconditional rewrite once its guard conditions are
  met (see e.g. `substitute_array_expression`'s hard `THRESHOLD: usize = 40`,
  `OXC/peephole/substitute_alternate_syntax.rs:1732`, chosen by hand: *"this threshold is chosen by
  hand by checking the minsize output"*). No byte-size measurement happens at rewrite time.
- **Debug-only correctness guards cost nothing in release** — `debug_assert_no_over_prune` /
  `debug_assert_no_under_prune` / `debug_assert_no_stale_direct_eval`
  (`OXC/compression_pass.rs:71-200`) walk the whole program once per pass in debug builds only, so
  the effort/correctness tradeoff is itself gated by build profile, not by a runtime flag.

### terser

- **Outer `passes` option, default `1`.** `TERSER/compress/index.js:259` (`passes: 1` in
  `defaults(...)`), consumed at `TERSER/compress/index.js:449` (`var passes = +this.options.passes
  || 1;`) inside `Compressor.prototype.compress`:
  ```js
  for (var pass = 0; pass < passes; pass++) {
      this._toplevel.figure_out_scope(mangle);
      if (pass > 0 || this.option("reduce_vars")) this._toplevel.reset_opt_flags(this);
      this._toplevel = this._toplevel.transform(this);
      if (passes > 1) {
          let count = 0;
          walk(this._toplevel, () => { count++; });
          if (count < min_count) { min_count = count; stopping = false; }
          else if (stopping) { break; }
          else { stopping = true; }
      }
  }
  ```
  (`TERSER/compress/index.js:449-473`.) Convergence is judged by **total AST node count**: the loop
  stops after the *second* consecutive pass that fails to shrink the node count (one plateau pass is
  tolerated before breaking). Unlike oxc, this is a user-facing knob defaulting to a *single* pass —
  terser explicitly does not fixed-point by default; multiple passes are opt-in and paid for
  explicitly.
- **Inner `tighten_body` fixed point, capped at `max_iter = 10`.**
  `TERSER/compress/tighten-body.js:234,252`:
  ```js
  var CHANGED, max_iter = 10;
  do {
      CHANGED = false;
      ...
      if (compressor.option("collapse_vars")) collapse(statements, compressor);
  } while (CHANGED && max_iter-- > 0);
  ```
  This loop runs **once per statement list**, independently of the outer `passes` counter — so a
  single outer pass can already iterate up to 10 times locally where `dead_code`/`if_return`/
  `join_vars`/`collapse_vars` interact.
- **Candidate-compare cost model (`best_of`/`size()`).** `TERSER/compress/common.js:170-190` and
  `TERSER/size.js:93-108`: several rewrites (e.g. bracket-vs-dot property choice,
  `TERSER/compress/index.js:3600-3606`) build both spellings and keep the shorter by walking the
  full candidate subtree and summing per-node-type `_size()` — a byte-count *proxy*, including a
  `mangle_options`-aware guess at post-mangle identifier length: `AST_Symbol.prototype._size`
  (`TERSER/size.js:445-451`) returns `1` when the symbol is mangleable under the compressor's
  `_mangle_options`, else the real (pre-mangle) name length. This is real per-rewrite work (build
  two trees, walk both), not
  a global search — bounded implicitly by rewrite-site count, not by an explicit iteration cap.
- **`inline` is itself a graduated 0-3 effort ladder** (`inline===true` → `3`,
  `TERSER/compress/index.js:293`; thresholds `inline>=2`/`inline>=3` gate argument- vs
  variable-injection, `TERSER/compress/inline.js:582-585`) — the closest terser analogue to
  LilScript's `optimization_level`.

### LilScript

- **`optimization_level: u8`, validated to `0..=15`** (`LS/config.rs:764-765`), used throughout the
  codebase as a monotonic feature-unlock threshold (`feature.minimum_level()`,
  `LS/config.rs:1678-1682`) rather than a fixed-point iteration count — this is a materially
  different cost model than oxc/terser's "loop N times," and it is the one the objective's
  "13 should be the sweet spot" language is about.
- **Explicit candidate-frontier budget keyed off the level**, `effective_candidate_limit`
  (`LS/config.rs:1687-1699`):
  ```rust
  pub fn effective_candidate_limit(&self) -> usize {
      let level_limit = match self.optimization_level {
          0..=2 => 1, 3..=4 => 16, 5..=6 => 64, 7..=8 => 192,
          9..=10 => 384, 11..=12 => 768, 13..=14 => 1_024,
          _ => usize::MAX,
      };
      let search_limit = match self.candidate_search {
          CandidateSearch::Off => 1, CandidateSearch::Production => 384,
          CandidateSearch::Always => usize::MAX,
      };
      self.candidate_limit.min(level_limit).min(search_limit)
  }
  ```
  This is a genuinely different kind of cost control from either competitor: it bounds the width of
  a **whole-artifact candidate search** (quote style × identifier alphabet × declaration variant ×
  IR phase-ordering variant, etc. — see `LS/compiler.rs:3480-3499`), not just a rewrite-pass repeat
  count. Level 15 explicitly removes the cap (`usize::MAX`), matching the objective's "15 buys the
  last bytes with wall-clock" framing.
- **Two named uncapped fixed-point loops** at the SSA/CFG level —
  `optimize_scalar_fixed_point` (`LS/optimizer.rs:2363-2401`) and `optimize_inlining_fixed_point`
  (`LS/optimizer.rs:1032-1050`) — both `loop { ...; if !changed { break; } }` with **no** iteration
  ceiling analogous to oxc's `iteration > 10` or terser's `max_iter = 10`. See Section H for why
  this is flagged as the report's most actionable finding.

---

## H. Candidates LilScript may be missing

Verdicts are graded against the grep evidence actually found, not against what would be "nice to
have." `PRESENT` includes cases where LilScript's mechanism is structurally different from
oxc/terser's (e.g. baked into codegen emission rather than a post-hoc AST rewrite) but achieves the
same or a strictly better outcome — that distinction is called out per row.

| # | Technique (from A-G) | Verdict | Evidence |
|---|---|---|---|
| 1 | `true`/`false` → `!0`/`!1` | **PRESENT** | `LS/codegen_ir_js.rs:1447` (`return "!0".to_string();`), `:25640` (`ConstValue::Bool(true) if compact_boolean_literals => "!0"`), `:1159,26209` recognize `"!0"`/`"!1"` as canonical spellings. |
| 2 | `undefined` → `void 0` | **PRESENT** | `LS/js_peephole/folds/control.rs:600,677` emit `"void 0".to_string()`; `LS/js_peephole/folds/classes.rs:1163-1340` use `===void 0`/`!==void 0` guards pervasively for default-parameter lowering. |
| 3 | Bracket → dotted property access | **PRESENT, built into codegen rather than a post-hoc rewrite** | `LS/codegen_ir_js.rs:13793-13799`: `static_identifier_property(index, ...)` is checked before emission and dispatches to `JsExpression::member` (dot form); only a non-identifier-safe key falls through to `JsExpression::index` (bracket form, `:13802-13811`). Because the IR→JS emitter chooses the shorter form at the source, there is no `foo['bar']` ever emitted for LilScript-authored code to begin with — a stronger position than oxc/terser's after-the-fact rewrite. |
| 4 | Numeric-string key → number in bracket access | **PRESENT** | Same call site as #3 falls back through `context.string_constants.get(&index)` before falling further to a raw `.index()`, and `render_property_key_literal` (`LS/codegen_ir_js.rs:13803-13807`) renders numeric-looking keys unquoted. |
| 5 | Shortest-form numeric literal (hex / exponent) | **PRESENT, and extends the technique** | `LS/codegen_ir_js.rs` test `renders_shortest_exact_numeric_literals` (:34191-34201) exercises `shortest_integer`/`shortest_float`, including a spelling neither oxc nor terser produce: `shortest_integer(1_099_511_627_776)` → `"(2**40)"` (exponentiation-operator form), beating both the decimal and the hex spelling. |
| 6 | Quote-character choice | **PRESENT, different granularity than terser** | Not a per-string `dq`-vs-`sq` count like `TERSER/output.js:444-461`. Instead `CompressionDecision::QuoteStyleSelection` (`LS/config.rs:1022,1108`) drives a **whole-artifact candidate search**: `LS/compiler.rs:3495` (`quote_variants = ... * 2 + 1`) and `:3640-3657` build up to 3 full emissions (double/single/template) per identifier-alphabet variant and score them through the real `cost_model` (gzip/brotli), not a heuristic per-string count. Comment at `:3486-3492` explicitly names "two quote styles" as part of the bounded cross-product and mentions "Brotli-11 probes." Global-per-candidate rather than per-literal is the one real gap versus terser's finer granularity; whether that matters depends on how much per-string quote-mix variance a given `.lil` source produces. |
| 7 | Array-of-strings → `"a,b".split(delim)` | **PRESENT, more general than oxc** | `LS/codegen_ir_js.rs:25904-25926` (`packed_string_array`): unlike oxc's fixed `THRESHOLD=40` / comma-only rule, LilScript tries 6 candidate delimiters (`,` ` ` `\|` `;` `~` `:`), filters to ones that don't collide with any string's content, and picks the shortest via `.min_by`. No arbitrary length gate. |
| 8 | `Boolean(a)`/`Number(0)`/`String()` primitive-call folding | not directly checked | Not specifically greped; LilScript's constant-value system (`LS/optimizer.rs` `ConstValue`) and its `Intrinsic` folding table (see #12) make this likely subsumed by general constant folding rather than needing a syntactic special case, but this was not independently confirmed. Marking **PARTIAL** pending direct evidence. |
| 9 | `new Object()`/`new Array()` → literal | **PARTIAL / not directly evidenced** | LilScript compiles from its own typed language, not from arbitrary JS source, so a literal `new Object()` call is unlikely to ever appear in generated IR; no grep hit for an explicit constructor-to-literal fold. Structurally this is closer to N/A than ABSENT — LilScript's object literals are synthesized directly from struct/class layouts (see #17), so the JS-level rewrite this technique exists for in oxc has no analogous source shape to trigger it. |
| 10 | `if/else` → ternary | **PRESENT** | `LS/js_peephole/folds/boolean.rs:16-21` doc + `fold_expression_branches` (:21-120): *"Fold a braced `if`/`else` whose arms contain only expression statements into a conditional expression."* Notably gated as a **scored candidate**, not an unconditional rewrite — see #21 (the `cost_model`-scored candidate search this feeds into). |
| 11 | IIFE simplification | **PRESENT** | `LS/js_peephole/folds/calls.rs:256-271` (`fold_identity_arrow_iife`, `fold_zero_argument_return_iife`), `:470` (`fold_return_only_iife`), plus `LS/js_peephole/folds/classes.rs:7026` (`fold_value_binding_iife`). |
| 12 | Known-method constant folding (`charAt`, `indexOf`, `slice`, …) | **PRESENT, with correct UTF-16 semantics** | `LS/optimizer.rs:12796-12870` folds `Intrinsic::StringCharAt`/`StringCharCodeAt`/`StringIndexOf`/`StringLastIndexOf`/`StringIncludes`/`StringRepeat` against `ConstValue`, using `receiver.encode_utf16()` — i.e. it respects ECMA-262 UTF-16 code-unit indexing rather than naive Rust `char` indexing, which a straightforward port would get wrong. |
| 13 | `Math.pow`→`**` and other `Math.*` folding | not directly checked for the `**` rewrite specifically | LilScript's `Intrinsic` table doesn't obviously expose a bare `Math.pow` textual rewrite the way oxc does (`OXC/peephole/replace_known_methods.rs:59-91`); LilScript's approach is to fold `Math.*` calls to constants when arguments are constant (general constant folding) rather than rewriting `Math.pow(a,b)` to `+(a)**+b` when arguments are *not* constant. **PARTIAL** — the constant-argument case is covered by general folding; the non-constant operator-rewrite case was not found. |
| 14 | Single-use variable inlining into its one read site | **PRESENT** | `LS/js_peephole/folds/returns.rs:322` (`fold_single_use_temporaries`), `LS/js_peephole/folds/copies.rs:795-829` (`fold_single_use_literal_bindings`/`fold_single_use_regex_bindings`), `:1547` (`fold_single_use_function_values`). |
| 15 | `collapse_vars`-style assignment-into-next-use folding | **PRESENT, explicitly modeled on terser** | `LS/js_peephole/folds/copies.rs:1033-1039` doc, verbatim: *"This is the small, proven `collapse_vars` subset generated control-flow tends to expose after return-branch folding."* — direct acknowledgment of the terser technique it is porting a subset of. |
| 16 | Function inlining (small bodies substituted at call site) | **PRESENT, and it's a whole SSA-level pass, not a peephole** | `LS/optimizer.rs:6044` (`inline_small_functions`) + `LS/optimizer.rs:1032-1050` (`optimize_inlining_fixed_point`), bounded by `LS/config.rs:998-1000,1164-1166` (`inline_instruction_limit`, `inline_control_flow_limit`, `max_inline_growth`), themselves level/priority-scaled (e.g. `LS/config.rs:2254-2305`: performance-first `24`/`60`, balanced `12`/`30`). Also `JS_HOST_INLINE_USE_LIMIT: usize = 4` (`LS/optimizer.rs:1087`) bounds inlining JS-host adapter closures used at ≤4 call sites. |
| 17 | Closure-ADVANCED-style object→array/scalar flattening | **PRESENT** | `LS/optimizer.rs:7149` (`scalar_replace_linear_classes`) and `:9552` (`scalar_replace_control_flow_aggregates`) promote non-escaping struct/class fields to individual SSA scalars (full flattening, stronger than an array-of-indices rewrite when it applies). For instances that must still be heap objects, LilScript emits them **positionally** (array-slot access, not `{name:value}` with mangled string keys) for internal (non-ABI) classes — see the test names `uses_positional_internal_classes_and_named_public_class_abi` (`LS/codegen_ir_js.rs:30670`) and the `Array.prototype`-slot behavioral test at `:31071-31076`. This is functionally the "object → array with indices" trick the objective names Closure ADVANCED mode for. |
| 18 | Property mangling: frequency-sorted assignment | **PRESENT, and it's loop-weighted, not just count-sorted** | `LS/codegen_ir_js.rs:6307` (`fn assign_property_names`), `:6394` (`frequencies: AHashMap<String, usize>`), weights occurrences inside loop bodies/updates by `1 + loop_count*3` (`:6417-6419`), then `fields.sort_unstable_by(right.1.cmp(&left.1)...)` (:6468-6470) — strictly more information-aware than oxc's plain occurrence-count sort (`OXC/property_mangler.rs:404-409`) and than terser's non-frequency, encounter-order assignment (`TERSER/propmangle.js:388-406`). |
| 19 | Char-frequency-driven mangling alphabet (gzip/brotli-aware) | **PRESENT, and more refined than terser's; measured worth ≤ 38 Brotli (039)** | `LS/codegen_ir_js.rs:1537-1611` (`IdentifierAlphabet::for_code`, `for_code_excluding_binding_characters`): sorts the base54/base64 alphabets by observed ASCII byte frequency in the surrounding emitted code, exactly the same idea as `TERSER/scope.js:976-1016` (`compute_char_frequency`: prints the program, `consider(text, +1)`, every mangleable `AST_Symbol` `consider(name, -1)`) and `:1018-1062` (`base54`: `reset`/`consider`/`sort` stable-sort the 54 leading characters and the digits separately, `get` numbers names first-character-fastest), but with an extra refinement terser's version lacks — `for_code_excluding_binding_characters` explicitly subtracts out characters contributed by *already-mangled* binding spellings so "yesterday's arbitrary mangled names" don't bias "tomorrow's alphabet" (doc comment, `:1565-1568`). Wired into binding names as well as properties: `LS/compiler.rs:3630-3652` proposes the configured, `for_code`, excluding-binding and keyword alphabets as scored candidates, `LS/compiler.rs:8872` (`search_identifier_alphabets`) probes bijective one-character swaps, and `LS/js_peephole/rename.rs:197` (`dominant_identifier_alphabet`) orders the converged-local alphabet — all gated by `CompressionDecision::EntropyAwareMangling` (`LS/config.rs:1040`). Also ships a static ETAOIN-style fallback, `IdentifierAlphabet::javascript_keyword()` (`:1566-1571`). 039 priced the technique: inside terser's own mangle the frequency alphabet is worth 38 / −36 / 18 / 13 Brotli (jquery tree / jquery committed / mobx / micromark), and a same-length relabel of our artifacts into terser's order moves −27 / +32 / +19 / −36. |
| 20 | Reserved/unmangleable-name safety list | not directly checked in detail | `LS/codegen_ir_js.rs:6325-6327` builds `PROTOTYPE_SENSITIVE_PROPERTY_NAMES` plus extern-field names into `stable_property_names`; this covers the correctness role oxc's `is_hard_reserved` and terser's `domprops.js` play, though a byte-for-byte comparison of the reserved-name universe (DOM API surface specifically) was not performed. **PRESENT** for the mechanism; coverage breadth unverified. |
| 21 | Real-codec-scored candidate search (score by actual gzip/brotli bytes, not a heuristic proxy) | **PRESENT — this is the one technique neither oxc nor terser has at all** | `LS/config.rs:1167,1504` (`CompressionCostModel::{Raw,Gzip,Brotli}`), consumed at `LS/compiler.rs:590-593,618-621` where actual `gzip_bytes`/`brotli_bytes` (real encoder output, matching the pinned zlib-1.3.1/Brotli-1.1.0 per objective.md) select among candidates. `LS/config.rs:1512-1513` (`CandidateSearch::{Off,Production,Always}`) and `decision_registry.rs:40` (`DecisionClass::Scored`) formalize which decisions are searched this way. Both oxc's `best_of`-less unconditional rules and terser's `size()`-proxy `best_of` (Section D/G) only ever optimize a byte-count *estimate*; LilScript is the only one of the three that measures the real compressed artifact during search. |
| 22 | Fixed-point iteration cap (compile-time safety valve) | **ABSENT — the report's most actionable finding** | `LS/optimizer.rs:2363-2401` (`optimize_scalar_fixed_point`) and `:1032-1050` (`optimize_inlining_fixed_point`) are both bare `loop { ...; if !changed { break; } }` with no iteration ceiling, no `debug_assert!`-style escape hatch, and (confirmed via grep) no `MAX_ITER`/`max_iterations`/`iteration_limit`/`deadline`/`Instant`-based cutoff anywhere in `optimizer.rs`, `compiler.rs`, or `compress_passes.rs` (`Instant::now()` at `LS/compiler.rs:1497` is used only for elapsed-time *reporting*, not as a deadline gate). oxc caps its single combined-traversal fixed point at effectively 11 iterations (`OXC/compressor.rs:129-136`); terser caps its per-statement-list `collapse_vars`/`if_return`/etc. loop at `max_iter=10` (`TERSER/compress/tighten-body.js:234,252`) independent of its outer `passes` count. LilScript's two central SSA-level fixed points have no equivalent safety net — this is directly on-target for the objective's own complaint (objective.md:20, *"I'm seeing that compilation takes very long time right now"*; objective.md:39, *"Right now since compilation takes infinitely long idk.. it's annoying"*): an interaction between two or more of the ~30 passes inside these loops that keeps finding small, non-terminating "improvements" (a classic pass-ordering oscillation risk in a 30-pass pipeline) has no hard backstop before it manifests as unbounded compile time. Recommend either an oxc-style debug-assert-then-break at a generous cap (cheap, catches regressions in CI without affecting release timing) or a terser-style hard numeric cap scaled by `optimization_level`, consistent with how `effective_candidate_limit` already scales the *search* side of the same level knob (Section G). |
| 23 | Token-adjacency spacing decided at print time | **ABSENT at the splice; a repair fold covers one shape (039 → 040)** | Terser never fuses two tokens because spacing is not the transform's job: `TERSER/output.js:595-605` (`print`) sets `might_need_space` after any `space()` and emits the space only when the previous character and the next token's first character are both identifier characters (or `//`, `++`, `--`). Closure does the same in `CodeConsumer.add` (`maybeInsertSpace`, not vendored). LilScript's textual folds splice replacement strings through `LS/js_peephole/rewrite.rs` (`apply_token_rewrites`) with no adjacency rule, then rely on `LS/js_peephole/folds/syntax.rs:255` (`split_fused_keyword_identifiers`) to undo `returnX`/`throwX` — a repair that refuses when the fused name is no longer a visible binding. Shipped jquerylil carries `returnHr(r,n,t,e)` from exactly that refusal (039). The technique to adopt is the printer's: one adjacency guard at the splice point, so no fold can fuse. |
| 24 | Property-read purity gate (the getter/proxy assumption behind `unstable`) | **PRESENT — stricter than all three by default, and decided by the receiver's *type* rather than by its syntax or a flag; the mechanism is fusion-blocking, not purity** | `LS/config.rs:1258-1265,1320` (`assume_pure_property_reads`, off) is consulted only at `LS/codegen_ir_js.rs:25198-25201`: an `IndexGet` is observable iff the flag is off **and** the receiver's coercion category is `dynamic`, which is exactly `JsValue` = `Type::TypeParameter("$js")` (`:25338-25341`, `LS/semantic.rs:222`). `FieldGet`, `RecordFieldGet` and `HostFieldGet` fall to `_ => false` (`:25267`): a `struct`, `class`, `Record<T>` or `extern class` read is never an observable evaluation. But **no member read is deferrable** (`op_can_defer`, `:24703-24750`, lists none of them), so every read is in `unstable_values` (`:20295-20328`) with or without the flag, and survives only by *fusion* into its consumer (`can_defer_value_to_block_end`, `:24444-24468`): every instruction between definition and use must be non-observable, expression-only and not a `HostFieldGet` (`:24466`). So 013's −405 stores / −540 Brotli is not "reads became pure" — it is "dynamic reads stopped blocking each other's fusion". The typed version of the same effect needs no flag: a receiver retyped from `JsValue` to `Record<T>`/`struct`/`object` stops blocking, while an `extern class` receiver stays a blocker by design (`:24466`; `docs/knowledge/language/effects-purity.md:15`) — which the flag wrongly waives for `JsValue`-typed DOM reads (shipped jquerylil: `elem` 130, `event` 40, `xhr` 22, `win` 14 of 1344 string-key reads). Correction to 013: Terser's default is not "off" but `"strict"` (`TERSER/compress/index.js:261`), which already waives getters and keeps only the nullish-receiver hazard; LilScript's default (hooks kept) equals Terser's `pure_getters:false` and oxc's `PropertyReadSideEffects::All`. |
| 25 | Object literal → scalars / scalar replacement of aggregates | **PRESENT at the SSA level for `LocalOnly` aggregates; ABSENT for anything that escapes, where Closure `CollapseProperties` (not vendored — verify) still collapses** | `LS/optimizer.rs:7186-7219` (`scalar_replace_linear_classes`, requires `value_escapes[out] == EscapeState::LocalOnly`), `:9552` (`scalar_replace_control_flow_aggregates`), escape graph `:7408` (`analyze_escapes`; `EscapesToUntypedBoundary` for host, export, `JsValue`, print and indirect calls — `LS/semantic.rs:19-23`, `docs/knowledge/language/boundaries-escape.md:22`). Records: `immutable_closed_record_values` (`:8936-9010`) and `project_closed_record_observations` (`:9205-9218`) forward statically-own reads of a compiler-owned `Record`/`object{}` allocation whose identity never reaches a write, phi, terminator, alias or host use (`docs/language-v0.1.md:125-131`). That precondition is stronger than terser's `hoist_props` (which tolerates `o.k=v` writes and escapes deeper than one property) and than Closure's `InlineObjectLiterals` (local var, accessor-free literal, every reference `v.prop` read or plain write, no whole-value use, no `v.m()` this-call, no nested-function reference — not vendored, verify); the coverage is weaker than Closure's `CollapseProperties`, which flattens an *exported* namespace `a.b.c` → `a$b$c` on a whole-program proof (one global set, no local sets, no `delete`, no aliasing gets or `UNSAFE_NAMESPACE`, accessor-defined names never collapse, `@nocollapse`). Shipped jquerylil (`/home/azureuser/jquerylil/src`, 134 files) declares 0 `Record<`, 0 `object {`, 80 `struct`, 2645 `JsValue` mentions, so neither the record proof nor SROA has anything to fire on; its biggest receivers are `jQuery` 218 + `fn` 45 (an `export JsValue` function carrying data members, `src/core.lil:86`), `s` 104 (`ajaxSetup(JS.object(), options)`, `src/ajax.lil:435` — a compiler-owned allocation typed `JsValue`), `options`/`opts`/`opt` 65 and `hooks`/`specialForType`/`support`/`tween` 96. |
| 26 | Getter/proxy legality position — what each tool assumes by default and how it proves the exception | **PRESENT — LilScript is the only one of the four whose proof is per-value and typed; what it lacks is a way to carry that proof through a declared API boundary** | `docs/knowledge/mission.md:105` refuses a default-on getter/proxy/pristine assumption; `JsValue` carries no purity (`docs/language-v0.1.md:421-430`); `docs/knowledge/language/compressor-surface.md:157-158,169-176` refuses "treating an external object declaration as hook-free" and requires a compiler-owned non-proxy allocation, no accessors, no prior untyped escape, and a proven-own key or controlled prototype. Terser proves non-nullishness per receiver *syntax* and waives getters (`"strict"`, above); oxc proves nothing per value (one global flag plus a known-globals list); Closure proves per property *name* — `GatherGetterAndSetterProperties` → `AccessorSummary`, consumed by `AstAnalyzer.mayHaveSideEffects` under `CompilerOptions.assumeGettersArePure`, so a read is impure only if a getter of that name exists anywhere in the program or externs (not vendored — verify the default). None of the three can know that *this* allocation was created by the compiler and that every write to it was a data write; LilScript's escape graph already knows both, and a declared data-ABI type — the way `export constructor` declares constructor identity (`docs/language-v0.1.md:568-571`) — would let that proof survive the escape through the port's own public API instead of dying at it. |

### Summary of the valuable findings

- **#22 (no fixed-point iteration cap)** is the standout ABSENT and lines up exactly with the
  objective's stated pain point about long compile times — worth a dedicated hypothesis folder.
- **#6 (quote-style granularity)** and **#13 (`Math.pow`→`**` for non-constant operands)** are the
  two genuine PARTIAL gaps found; both are small, bounded peephole additions if pursued.
- **#8/#9** are honestly unverified rather than confirmed-absent; they'd need a few more targeted
  greps (or a small compiled-output experiment) before claiming a gap.
- Everything else checked (#1-5, #7, #10-12, #14-21) is not just present but in several cases
  (numeric-literal exponentiation spelling, `packed_string_array`'s multi-delimiter search,
  loop-weighted+entropy-aware property mangling, and above all the real-codec `cost_model` search)
  is **more general or more accurate** than the corresponding oxc/terser mechanism — LilScript is
  not just "keeping up" with these two competitors on the specific techniques surveyed here, it has
  already generalized several of them.

---

# Addendum — the two PARTIAL verdicts, measured

The survey above left three items short of PRESENT. Two were resolved by measurement rather than by
more reading, and both turn out **not** to be gaps.

## #13 `Math.pow(a,b)` → `+(a)**+b` for non-constant operands — **N/A, not ABSENT**

oxc rewrites this (`peephole/replace_known_methods.rs`). The survey marked LilScript PARTIAL because
only the constant-argument case is covered by general folding.

`grep -rn "MathPow\|\"pow\"\|Math\.pow" src/*.rs` returns **nothing**. LilScript has no `Math.pow`
intrinsic and never emits the call, so there is no input for the rewrite to fire on. oxc needs it
because it minifies arbitrary hand-written JavaScript; LilScript owns its own code generation and
emits `**` directly where the operation arises. Reclassified **N/A**.

## #6 Per-string quote selection — **PRESENT and better, not PARTIAL**

terser picks the quote character per string literal (`output.js`); LilScript picks one style for the
whole artifact and scores the alternatives through the real `cost_model`. The survey called the
coarser granularity "the one real gap".

Measured on the shipped jQueryLil artifact (1110 double-quoted literals):

- literals that would be **shorter** single-quoted: **5**
- total raw bytes they would save: **5**
- literals containing both quote kinds, i.e. template-literal candidates: **0**

So the finest possible per-string assignment is worth **five raw bytes on an 89 KB artifact**, and
under a compressing objective it is likely *negative*: mixing quote characters flattens the byte
distribution that Brotli's context modelling and the entropy-aware identifier alphabet both exploit.
LilScript's whole-artifact, codec-scored choice is the better design for this objective, not a
weaker version of terser's. Reclassified **PRESENT**.

## #22 Fixed-point iteration cap — **was ABSENT, now closed**

The survey's headline finding. Caps landed in `src/optimizer.rs`
(`MAX_SCALAR_FIXED_POINT_ROUNDS = 32`, `MAX_INLINING_FIXED_POINT_ROUNDS = 24`) with a
`debug_assert!` so an oscillation fails a test rather than hanging a build.

Worth recording alongside it: the survey inferred this was likely causing the long compile times.
It was not. Measured convergence on the jQuery port is **2** rounds for the scalar pipeline and
**12** for the inlining pipeline, together under 6% of wall clock — see
[004](../004-peephole-relex-tax/README.md). The cap is a correctness/robustness fix, and the real
compile-time cause was elsewhere. A structural gap and a performance cause are different claims.

---

# Addendum 2 — terser's statement sequencing, read against LilScript's gap

Prompted by [013](../013-statement-density/README.md): jQueryLil emits **2.06x** Terser's assignment
statements, and [016](../016-marked-size-regression/README.md) showed a regression whose whole
signature was **+479 `;` and −388 `,`**. So the obvious question is what terser does to merge
statements that LilScript does not.

## What terser actually does

`TERSER/compress/tighten-body.js` has two distinct passes.

**`sequencesize` (:1253)** merges runs of adjacent simple statements into one comma expression,
bounded by `compressor.sequences_limit` (default 200). Note the detail at :1272 — every statement
after the first goes through `body.drop_side_effect_free(compressor)`, so a pure statement in the
middle of the run is *deleted*, not merged.

**`sequencesize_2` (:1305)** is the one with no LilScript counterpart. It absorbs a preceding simple
statement into the *head* of the next control-flow construct:

| construct | rewrite |
|---|---|
| `AST_Exit` (:1317) | `x=1; return v` → `return (x=1, v)` |
| `AST_For` (:1319) | `x=1; for(i;;)` → `for((x=1,i);;)`, or `for(x=1;;)` when there is no init |
| `AST_ForIn` (:1338) | `x=1; for(k in o)` → `for(k in (x=1,o))` |
| `AST_If` (:1343) | `x=1; if(c)` → `if((x=1,c))` |
| `AST_Switch` (:1345) | `x=1; switch(e)` → `switch((x=1,e))` |
| `AST_With` (:1347) | same shape |

Plus `to_simple_statement` (:1287), which hoists `var` declarations out of an `if`'s branches so the
`if` itself becomes a simple statement and can then be sequenced.

LilScript has the for-init case (`fold_prior_assign_into_for_init`, `folds/loops.rs:2104`) and the
adjacent-statement merge (`fold_adjacent_expression_statements`, `folds/calls.rs:132`). It has **no**
fold that absorbs a prior statement into an `if` condition, a `return` value, or a `switch`
discriminant.

## ...and that gap is worth approximately nothing

Before proposing it, the arithmetic:

```
a=1;if(c){...}      ->  if(a=1,c){...}        9 chars -> 9 chars
a=1;b=2;return x    ->  return(a=1,b=2,x)    16 chars -> 17 chars
```

**`;` and `,` are both one byte, so sequencing is byte-neutral on raw and can be a byte worse** once
the wrapping parentheses are counted. terser wants it because it feeds `drop_side_effect_free` and
its other expression-level passes, not because the comma itself is smaller.

This also corrects the reading of 013's own evidence. The +479 `;` / −388 `,` in the regressed
artifact roughly cancel; the actual cost was the **+261 `var`/`let` keywords** (3–4 bytes each) and
**+441 identifier occurrences** that came with them. The expensive thing is starting a new
*declaration*, not starting a new *statement*.

**So the technique to chase is not sequence absorption — LilScript already has
`merge_adjacent_declarations` — it is not splitting a declarator list in the first place.** That is
an SSA-destruction decision, upstream of every fold discussed here. Recorded so the terser pass is
not ported on the strength of its name.
