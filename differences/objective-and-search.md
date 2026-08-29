# Objective and search

Parent: [comparison index](index.md). Applications: [compression opportunities](compression-opportunities.md).

## The central difference

Closure usually asks: is this rewrite legal, structurally simplifying, or locally smaller under a
raw/minified-size proxy? LilScript can ask: among complete legal artifacts, which one is smallest
under the configured raw, gzip, or Brotli objective?

That distinction should guide how Closure ideas are adopted. Closure is a rich source of candidate
generators and safety cases. Its local cost models should rarely become LilScript's final decision
rule.

## Closure's decision machinery

### Fixed-point scheduling

Closure groups loopable passes and selectively reruns them based on observed changes; any later
change can make an earlier stable pass eligible again. The hard limit is 100 iterations. A
code-removal loop may stop after two batches each changing AST node count by less than 0.05 percent.
See
[`PhaseOptimizer.java` lines 302-447](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PhaseOptimizer.java#L302-L447).

The peephole driver has another fixed point inside each invocation. It revisits changed
function/script roots rather than traversing the whole AST every round. See
[`PeepholeOptimizationsPass.java` lines 60-135](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PeepholeOptimizationsPass.java#L60-L135).

This is a scheduling heuristic, not a compressed-byte objective. AST node reduction can disagree
with printed bytes, and both can disagree with gzip/Brotli.

### Local cost models

Closure uses several unrelated local models:

- function inlining runs a real code generator over candidate AST but normalizes identifiers to
  two characters and constants to one character;
- condition minimization charges only negations and precedence parentheses common to competing
  forms;
- prototype-prefix extraction charges fixed helper and per-cluster setup syntax;
- array-join folding uses estimated generated code;
- string-array packing estimates saved quotes against `.split(...)` syntax;
- numeric folding compares decimal spelling lengths and avoids unsafe precision cases;
- most DCE, parameter, object-splitting, and peephole rewrites have no byte check.

Sources:

- [`InlineCostEstimator.java` lines 21-106](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/InlineCostEstimator.java#L21-L106)
- [`FunctionInjector.java` lines 905-1075](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/FunctionInjector.java#L905-L1075)
- [`MinimizedCondition.java` lines 196-314](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/MinimizedCondition.java#L196-L314)
- [`ExtractPrototypeMemberDeclarations.java` lines 95-149](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ExtractPrototypeMemberDeclarations.java#L95-L149)

### Compression-aware but not codec-scored heuristics

Closure does include heuristics specifically motivated by compressed output:

- equal-width global names are assigned by source occurrence so nearby declarations receive
  similar spellings;
- `const`/inner `let` may be homogenized toward `var`/`let` to improve keyword repetition;
- prototype-prefix extraction amortizes repeated `X.prototype` text;
- statement fusion assumes commas are likely cheaper/more frequent than semicolons;
- string-array delimiters prefer common punctuation;
- string aliasing is disabled by default because its own documentation says it usually hurts gzip.

These are useful proposal strategies. None measures the final gzip or Brotli stream.

## LilScript's decision machinery

### Exact transfer cost

LilScript measures:

- raw UTF-8 bytes;
- gzip level 9 through pinned zlib 1.3.1;
- Brotli quality 11, generic mode, `lgwin=22`, through pinned encoder 1.1.0.

See [`compiler.rs` lines 9311-9389](../src/compiler.rs#L9311-L9389).

`size-first` compares exact selected transfer bytes before performance tie-breaks. For gzip/Brotli,
candidate admission succeeds when compressed size does not regress regardless of raw growth, or as
a fallback when raw size remains within `max_candidate_raw_growth_percent` even if compressed size
regresses. See [`compiler.rs` lines 5926-5989](../src/compiler.rs#L5926-L5989).

### Portfolio search

The search spans more than mangling:

- optimizer-setting variants that disable CSE/specialization or alter inlining limits;
- inlining, specialization, closure-factory, and helper alternatives;
- scalar, positional, named, and retained aggregate representations;
- conditional, phi, operand-order, loop, mutation, and function-spelling variants;
- string/numeric pooling, packed arrays, dense tables, booleans, quotes, and regex forms;
- function ordering, declaration spelling, local-name reservations, and alphabets;
- parsed-JavaScript cleanup and terminal local/global name search.

The registry exposes 48 sequential emission families plus IR variants. See
[`decision_registry.rs` lines 741-1493](../src/decision_registry.rs#L741-L1493) and
[lines 1511-1684](../src/decision_registry.rs#L1511-L1684).

Candidate retention is bounded by count, source bytes, family beams, priority reserves, and a
terminal codec-probe ledger. The configured artifact remains a mandatory fallback. See
[`compiler.rs` lines 6453-6829](../src/compiler.rs#L6453-L6829) and
[lines 8885-9182](../src/compiler.rs#L8885-L9182).

### Multi-objective retention

Intermediate candidates are retained across selected, raw, gzip, and Brotli rankings even though
the final winner uses the configured objective. This reduces the chance that an intermediate form
needed for a later dictionary win is discarded too early. See
[`compiler.rs` lines 6113-6451](../src/compiler.rs#L6113-L6451).

### Function layout and naming

Function order can be source order, n-gram-similarity order, or codec-window-aware order. The
similarity objective uses exact Held-Karp below its cutoff and deterministic insertion plus 2-opt
above it. The separate window-aware heuristic grows from endpoints and compares source, adjacency,
and window paths; it does not exactly solve that objective or apply 2-opt. See
[`codegen_ir_js.rs` lines 562-937](../src/codegen_ir_js.rs#L562-L937).

Identifier search proposes artifact-frequency alphabets and bounded permutations, then re-emits
through the real mangler. See [variable mangling](variable-mangling.md#alphabet-and-objective).

### Chunk deployment objective

For split output, LilScript compresses chunks independently and combines raw/gzip/Brotli weights,
request overhead, dependency depth, preload discount, and cache-reuse discount. Optional joint
search currently varies function layout and local-name reservation. See
[`compiler.rs` lines 1049-1136](../src/compiler.rs#L1049-L1136) and
[lines 1311-1348](../src/compiler.rs#L1311-L1348).

## Limits of LilScript's advantage

Exact scoring only ranks generated candidates. It cannot select a Closure transformation that
LilScript never proposes. Search is also bounded:

- production mode clamps the effective retained candidate count;
- broad modules receive fewer optimizer-setting variants;
- many peepholes use fixed round counts;
- split builds do not run the complete single-artifact search;
- the checked-in explicit compression list disables many implemented families;
- final transfer optimality is not proven globally.

The checked-in [`lilscript.toml`](../lilscript.toml#L8-L60) uses size-first Brotli search but an
explicit decision allowlist. [`mode = "single"`](../lilscript.toml#L90-L93) disables compiler-managed
split planning in that build.

## Recommended synthesis

1. Port a Closure transform as a semantics-preserving candidate generator.
2. Prefer typed IR when ownership, effects, ranges, or calls are known there.
3. Use a hygienic target-JS tree for JavaScript-specific syntax alternatives.
4. Retain Closure's legality checks and adversarial tests.
5. Keep the existing output as incumbent.
6. Re-emit and validate the challenger.
7. Let the selected raw/gzip/Brotli or chunk deployment objective decide.
8. Record why the candidate won, lost, or was ineligible.

This converts Closure's mature heuristics into search coverage without inheriting its proxy cost
model as policy.
