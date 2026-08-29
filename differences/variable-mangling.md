# Variable mangling

Parent: [comparison index](index.md). Related: [pipeline and safety](pipeline-and-safety.md)
and [advanced opportunities](advanced-opportunities.md).

## Closure's algorithm

Closure first normalizes local variables into positional keys. A local gets a temporary `L n`
identity where `n` includes the variables in enclosing local scopes. Independent functions can
therefore reuse the same slots, while simultaneously visible nested locals cannot. See
[`RenameVars.java` lines 270-291](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/RenameVars.java#L270-L291)
and [lines 525-566](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/RenameVars.java#L525-L566).

It then applies these heuristics:

- count every renamable AST name occurrence, including declarations, for a global name or local slot;
- sort by descending count and break ties by first source occurrence;
- give the shortest generated names to the most frequent assignments;
- for globals receiving names of equal length, reorder assignment by source occurrence so nearby
  declarations receive similar spellings, explicitly to improve gzip locality;
- reuse names from a previous `VariableMap` when they remain legal;
- reserve externs, exports, `arguments`, configured names, and hazardous host globals;
- disable positional local keys in scopes above 1,000 locals when stable-name preference is on,
  avoiding mass rename-map churn after an insertion.

The count/order logic is in
[`RenameVars.java` lines 294-344](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/RenameVars.java#L294-L344).
Map reuse and the equal-length gzip heuristic are in
[lines 396-499](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/RenameVars.java#L396-L499).

Closure has a separate pre-renaming coalescer. `CoalesceVariableNames` computes AST-level CFG
liveness, builds an interference graph, greedily colors it, and rewrites non-overlapping variables
to one declaration. This reduces declaration syntax and distinct names before `RenameVars` counts
them. See
[`CoalesceVariableNames.java` lines 50-70](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/CoalesceVariableNames.java#L50-L70)
and [lines 136-180](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/CoalesceVariableNames.java#L136-L180).
It only analyzes nonescaped locals in function scopes below the implementation's 100-variable
limit; globals and larger functions are skipped.

## LilScript's algorithm

LilScript avoids naming many values at all. Pure, safely deferable single-use SSA values remain
expressions; only values needing storage enter local allocation. The decision is made while
emitting typed control-flow IR, not reconstructed from source JavaScript. See
[`codegen_ir_js.rs` lines 19000-19050](../src/codegen_ir_js.rs#L19000-L19050)
and [lines 19240-19318](../src/codegen_ir_js.rs#L19240-L19318).

For stored locals LilScript:

- solves backward `live_in`/`live_out` dataflow;
- creates interference edges for simultaneously live values, parameters, member receivers,
  captures, loops, short-circuit paths, and phi hazards;
- removes selected edges for proven two-address phi pairs;
- forms noninterfering phi-affinity groups;
- in direct mode, colors parameters first, then by parameter position, static use count, graph
  degree, and stable `ValueId`;
- in grouped phi-affinity mode, orders groups by parameter status/position, aggregate uses, and first
  `ValueId`;
- can rank final colors by aggregate use count;
- can bias colors toward spellings associated with source-local names across functions.

The liveness and graph construction are in
[`codegen_ir_js.rs` lines 20222-20713](../src/codegen_ir_js.rs#L20222-L20713),
with coloring in [lines 21780-21917](../src/codegen_ir_js.rs#L21780-L21917).

Top-level functions, globals, adapter factories, and selected aliases share one `uses + 1`
ranking. Ties use binding kind and stable numeric ID. See
[`codegen_ir_js.rs` lines 5687-5814](../src/codegen_ir_js.rs#L5687-L5814).

Cross-scope reuse is explicit rather than an accidental effect of textual names. Depending on
configuration, the allocator releases unreferenced outer spellings using conservative,
transitive, or precise reference sets, then re-reserves bindings a nested function could capture.
`local_name_reserve` keeps a repeatable short-name prefix available in each function. See
[`codegen_ir_js.rs` lines 6550-6726](../src/codegen_ir_js.rs#L6550-L6726).

## Late convergence

[`converge_local_names`](../src/js_peephole/rename.rs#L34-L170) is a separate final-JavaScript
candidate. It:

- resolves identifier tokens to declarations;
- requires total whole-artifact resolution and rejects template-bearing artifacts;
- processes parent scopes before children;
- blocks observed free, unresolved, and referenced outer names in scopes the resolver accounts for;
- attempts parent/child reuse when the child does not read the parent binding;
- assigns parameters by position, then locals by descending resolved use count;
- restarts one artifact-derived alphabet in every function;
- generates only one- and two-character candidates;
- preserves directly declared function/class names because `.name` is observable.

It is attempted only for gzip/Brotli builds and retained only when the exact compressed artifact
gets smaller. See [`compiler.rs` lines 7574-7609](../src/compiler.rs#L7574-L7609).

The 2026-08-29 correctness migration now requires
[`BindingResolution::is_total`](../src/js_peephole/binding.rs#L179-L185), reserves fixed descendant
function/class names, and makes the bounded one-/two-character generator return exhaustion safely.
It conservatively declines template-bearing artifacts until template expressions carry binding
identity.

## Alphabet and objective

Closure's default generator enumerates `a-z A-Z $` in first position and adds `_0-9` later. It
filters keywords, reserved names, `let`, `yield`, `await`, and `eval`. See
[`DefaultNameGenerator.java` lines 82-103](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/DefaultNameGenerator.java#L82-L103)
and [lines 288-320](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/DefaultNameGenerator.java#L288-L320).
Its `favors()` character-frequency mechanism exists, but the normal production pass wiring in this
snapshot does not call it.

LilScript proposes artifact-frequency, binding-subtracted, keyword-frequency, and bounded
permutation alphabets. It re-emits typed IR for retained candidates and performs terminal global
and function-local name swaps. The winner is measured with the configured exact raw, gzip, or
Brotli scorer. Relevant search code is in
[`compiler.rs` lines 6846-6899](../src/compiler.rs#L6846-L6899),
[lines 8100-8678](../src/compiler.rs#L8100-L8678), and
[lines 8680-8875](../src/compiler.rs#L8680-L8875).

## Direct comparison

| Heuristic | Closure | LilScript |
|---|---|---|
| Avoid emitting a variable | Inlining and DCE passes | SSA expression fusion plus inlining/DCE |
| Non-overlapping live ranges | AST CFG coalescing | IR CFG interference coloring |
| Reuse across independent functions | Positional local slots | Per-function allocator plus explicit reservations/releases |
| Hot name priority | Renamable AST-name occurrence count | Use count, parameter position, degree or grouped affinity, then codec search |
| Compression locality | Source-order regrouping within equal lengths | Canonical local shape, function layout, exact codec search |
| Stable rebuilds | Persisted rename maps and large-scope heuristic | Deterministic IDs/order, but no persisted map |
| Debug names | Pseudo names and maps | Disable mangling or inspect traces |
| Search objective | Mostly raw length with gzip heuristic | Exact selected transfer metric |

## Assessment

LilScript's core local allocator starts from typed SSA information and evaluates the transfer cost
of bounded naming candidates. Closure starts from general JavaScript, with broader syntax coverage,
persisted maps, pseudo-name mode, and mature AST/source tracking. Source inspection alone does not
establish which allocator produces smaller output on equivalent programs.

Closure's equal-length source-locality rule is worth adding as a cheap proposal, not as the sole
allocator. LilScript can submit it to the existing exact scorer. Closure's positional `L n` scheme
should not replace SSA liveness coloring.
