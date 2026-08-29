# Peephole and syntax compression

Parent: [comparison index](index.md). Selection model: [objective and search](objective-and-search.md).

## Closure's peephole architecture

Closure runs ordered peephole sets early, inside main optimization loops, after normalization is
relaxed, and late after renaming. The normal set includes exit minimization, condition
minimization, alternate syntax, known-method replacement, dead-code removal, constant folding, and
property-assignment collection. The late set adds statement fusion and uses late-only spellings.

See
[`DefaultPassConfig.java` lines 1687-1803](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/DefaultPassConfig.java#L1687-L1803).

Within one invocation, `PeepholeOptimizationsPass` reaches a changed-scope fixed point. Between
invocations, the outer phase optimizer lets inlining, DCE, and other passes expose more peepholes.

LilScript's generated-JS peephole uses a fixed ordered schedule: most entries run once, selected
families use caps of four, six, or eight, some folds converge locally, and one second whole pass is
conditional on a remaining constructor/prototype-table shape. Terminal cleanup uses bounded beams
and exact codec probes. See
[`js_peephole/mod.rs` lines 1598-1861](../src/js_peephole/mod.rs#L1598-L1861) and
[`compiler.rs` lines 7495-7894](../src/compiler.rs#L7495-L7894).

## Condition minimization

Closure's `MinimizedCondition` is a compact dynamic program over positive and negated forms of
`!`, `&&`, `||`, ternary, and comma trees. It counts only changed punctuation: each `!` and any
parentheses forced by precedence. Ties preserve the input form. A caller can declare an outer
negation free when it can swap branches. See
[`MinimizedCondition.java` lines 27-193](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/MinimizedCondition.java#L27-L193)
and [lines 196-314](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/MinimizedCondition.java#L196-L314).

`PeepholeMinimizeConditions` then applies the selected form in contexts such as:

- `if (c) e()` to `c&&e()`;
- two expression branches to a ternary;
- same-LHS branch assignments to one conditional RHS;
- conditional returns to one return;
- identical trailing branch statements hoisted after the branch;
- terminal loop breaks moved into loop conditions;
- Boolean-valued ternaries reduced to `!`, `&&`, or `||`.

See
[`PeepholeMinimizeConditions.java` lines 108-238](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PeepholeMinimizeConditions.java#L108-L238)
and [lines 503-777](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PeepholeMinimizeConditions.java#L503-L777).

LilScript has many equivalent targeted folds in typed emission and
[`folds/boolean.rs`](../src/js_peephole/folds/boolean.rs) plus
[`folds/control.rs`](../src/js_peephole/folds/control.rs). The missing generality is a reusable
positive/negative condition optimizer driven by printer precedence, whose alternatives can then be
exact-codec scored.

## Exit and control-flow minimization

Closure's `MinimizeExitPoints` removes trailing `return`, `continue`, and selected `break` nodes.
Its less obvious transform nests the suffix of a block under the inverse condition:

```js
if (x) return;
work();
```

becomes a shape later reducible to `!x&&work()`. It preserves function hoisting, avoids finally
completion changes, and refuses lexical-declaration movement through loops when per-iteration
capture could change. See
[`MinimizeExitPoints.java` lines 27-182](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/MinimizeExitPoints.java#L27-L182)
and [lines 305-375](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/MinimizeExitPoints.java#L305-L375).

`PeepholeRemoveDeadCode` additionally performs constant switch pruning, fallthrough merging, try
shell cleanup, unreachable-tail deletion with hoist preservation, assignment-to-condition
propagation, false-loop reduction, and optional-chain nullish folding. See
[`PeepholeRemoveDeadCode.java` lines 485-988](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PeepholeRemoveDeadCode.java#L485-L988)
and [lines 998-1533](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PeepholeRemoveDeadCode.java#L998-L1533).

LilScript already covers early exits, continue-tail guards, conditional-return ladders, loop
spellings, branch recovery, try/return alternatives, and state-machine versus structured emission.
Closure's switch cleanup and generalized duplicate return/throw follow-node reasoning are the best
additional test sources.

## Statement and assignment fusion

Late `StatementFusion` collects preceding expression statements into the final return, throw,
switch discriminant, `if` condition, or `for` clause using a comma expression. It deliberately does
not run in function/script roots and protects `for-in` LHS behavior. See
[`StatementFusion.java` lines 23-164](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/StatementFusion.java#L23-L164).

`ExploitAssigns` folds a standalone assignment into an immediately following equivalent use, such
as chaining equal assignments or moving an assignment into a condition. It traverses only safe
left/control positions and rejects destructuring and receiver-sensitive property cases. See
[`ExploitAssigns.java` lines 22-233](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ExploitAssigns.java#L22-L233).

`CollapseVariableDeclarations` then merges adjacent declarations of the same kind. See
[`CollapseVariableDeclarations.java` lines 29-159](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/CollapseVariableDeclarations.java#L29-L159).

LilScript has comma, operand-order, assignment-chain, declaration, and initializer/reassignment
families. SSA value identity can support broader assignment exploitation than Closure's syntactic
qualified-name checks, but every spelling should remain an optional candidate.

## Constant and known-method folding

`PeepholeFoldConstants` handles unary/binary arithmetic, BigInt, equality/relational comparisons,
short-circuit operations, string concatenation, arrays/objects/spreads, literal property access,
templates, assignment operators, and `Object.defineProperties(obj,{})`. Its careful cases are more
valuable than the obvious folds:

- it preserves numeric/string coercion order while gathering string leaves;
- it requires type facts for identities invalidated by NaN or BigInt mixing;
- it scans discarded array elements for effects;
- it refuses array-spread flattening when holes would become explicit `undefined`;
- it models object spread overwrite barriers, accessors, `super`, call receiver semantics, and
  optional-chain repair;
- it maintains both raw and cooked template text and avoids introducing accidental escapes.

See
[`PeepholeFoldConstants.java` lines 488-975](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PeepholeFoldConstants.java#L488-L975)
and [lines 978-2078](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PeepholeFoldConstants.java#L978-L2078).

`PeepholeReplaceKnownMethods` evaluates selected `Array`, `Number`, `Math`, and literal-string
methods. It also partially folds array joins and merges chained array concatenations when later
arguments cannot mutate the receiver. See
[`PeepholeReplaceKnownMethods.java` lines 108-445](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PeepholeReplaceKnownMethods.java#L108-L445)
and [lines 683-1302](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PeepholeReplaceKnownMethods.java#L683-L1302).

LilScript's typed intrinsic folder already covers arithmetic, ranges, array lengths, `imul`, and
many string operations. `trim`, `trimStart`, `trimEnd`, `slice`, and `split` reach the constant-fold
dispatcher but lack fold implementations; `replace` does not yet reach that dispatcher. General
constant `ArrayJoin` is also absent. Closure implements literal `trim` and selected literal
replacement, but not `trimStart`/`trimEnd`. See
[`optimizer.rs` lines 5128-5155](../src/optimizer.rs#L5128-L5155) and
[lines 12778-12865](../src/optimizer.rs#L12778-L12865).

## Literal reconstruction

`PeepholeCollectPropertyAssignments` folds immediately following indexed/property assignments into
a fresh array or object literal. It preserves holes, quoted keys, duplicate-key ordering, and
conflicting accessors already present in the literal, and allows at most three inserted holes. It
does not prove that prototype-chain setters are absent. A function literal may refer to the object
because it runs later; an immediately invoked function may not. See
[`PeepholeCollectPropertyAssignments.java` lines 26-149](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PeepholeCollectPropertyAssignments.java#L26-L149)
and [lines 168-329](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PeepholeCollectPropertyAssignments.java#L168-L329).

LilScript already folds fresh empty-object property writes and consecutive array pushes only under
the explicit pristine-builtins contract. The 2026-08-29 correctness migration moved the formerly
unconditional object fold behind that gate because syntactic/self-reference checks cannot prove an
inherited `Object.prototype` setter is absent. It already handles static dot/string keys and
duplicate-key order. Direct indexed array writes, nonempty initial literals, and more closed-record
observations remain useful extensions.

## Alternate syntax and denormalization

Closure substitutes compact syntax late:

- `Boolean(x)` to `!!x` or `x` when already Boolean;
- immutable `String(x)` to `""+x` for further folding;
- normalized `undefined` to `void 0`;
- safe `Object()`/`Array()` to literals;
- `x-=1` to `x--`;
- booleans to `!0`/`!1` or loose-comparison `1`/`0`;
- string arrays to delimiter-packed `.split(...)` when a local quote-saving model wins;
- constant templates to strings.

See
[`PeepholeSubstituteAlternateSyntax.java` lines 164-360](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PeepholeSubstituteAlternateSyntax.java#L164-L360)
and [lines 363-627](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PeepholeSubstituteAlternateSyntax.java#L363-L627).

`Denormalize` moves expressions/declarations into loop heads, restores compound/logical assignment
when target syntax permits, and moves bare `var` declarations to first assignments. See
[`Denormalize.java` lines 29-262](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/Denormalize.java#L29-L262).

`OptimizeLetAndConstPeephole` changes hoist-scope `let`/`const` to `var` and inner `const` to `let`,
explicitly seeking keyword homogeneity for compression. See
[`OptimizeLetAndConstPeephole.java` lines 23-93](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/OptimizeLetAndConstPeephole.java#L23-L93).

LilScript already searches booleans, quote styles, packing, loop/mutation spelling, declaration
forms, terminal semicolons, function spelling, optional/nullish syntax, and other emitted forms.
Closure's keyword-homogeneity transform is best added as a whole-function candidate rather than an
unconditional rewrite.

Two final ADVANCED syntax passes are easy to overlook. `ConvertToDottedProperties` changes safe
`obj["name"]` and quoted/computed static keys to shorter dot/unquoted forms while preserving
`constructor` and object-literal `__proto__` semantics. `RenameLabels` removes unused labels and
reuses short names by nesting depth independently in each function. See
[`ConvertToDottedProperties.java` lines 25-119](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ConvertToDottedProperties.java#L25-L119)
and
[`RenameLabels.java` lines 32-95](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/RenameLabels.java#L32-L95).

## What not to copy

- Closure explicitly accepts a semantic caveat when deleting empty destructuring assignments that
  would have thrown for a noniterable RHS.
- Closure's literal property collection does not prove absence of inherited array/object setters.
- Computed property access is assumed getter-free because the conservative model cost too much.
- `StatementFusion` admits that its comma-frequency assumption is empirical rather than measured.
- RegExp syntax and method folding are intentionally narrow; Closure does not provide a hidden
  regexp optimizer to copy.
- Most local rewrites are not gzip/Brotli evaluated.
