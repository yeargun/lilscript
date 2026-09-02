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
| Statement fusion via comma operator | `OXC/peephole/minimize_statements.rs` doc citing Closure's `StatementFusion.java`; `CompressOptions.sequences` (`options.rs:31-36`) | `TERSER/compress/tighten-body.js` `sequencesize`/`sequencesize_2`, `compressor.sequences_limit` (default `800` when `sequences==1`, `TERSER/compress/index.js:330`) | raw | peephole | `a();b();` → `a(),b();` |
| Dead-code-after-jump elimination | `OXC/peephole/minimize_statements.rs:53-90` (`is_control_flow_dead` tracking) | `TERSER/compress/tighten-body.js` `eliminate_dead_code` | raw | peephole | code after unconditional `return`/`throw` dropped |
| Switch-case minimization/fallthrough merge | `OXC/peephole/minimize_statements.rs:572` (`can_switch_case_be_inlined`) | `TERSER/compress/index.js:1244-1633` (`def_optimize(AST_Switch,...)`) | raw | peephole | adjacent identical `case` bodies merged |
| try/catch simplification | (part of the combined traversal, `remove_dead_code.rs:384-427`, `try_fold_try`) | `TERSER/compress/index.js:1634-1649` | raw | peephole | empty `catch{}` after non-throwing `try` block simplified |
| Sequence-expression folding / `remove_sequence_expression` (drops dead-value commas, hoists side-effect-only eval) | `OXC/peephole/remove_dead_code.rs:471-521` | `TERSER/compress/index.js:2053-2103` (`def_optimize(AST_Sequence,...)`) | raw | peephole | `(a(), b())` with unused result → `a(),b()` |
| Statement-level dead-code / unreachable-expression removal (side-effect-free expr statements dropped) | `OXC/peephole/remove_unused_expression.rs:20-1192` (`remove_unused_expression`, `esbuild`'s `SimplifyUnusedExpr`) | `TERSER/compress/drop-side-effect-free.js` (394 lines) | raw | peephole | `1+1;` (statement position) → removed |

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
