# Source reference

Parent: [comparison index](index.md).

## Snapshot

Closure Compiler was cloned with:

```sh
git clone --depth 1 git@github.com:google/closure-compiler.git
```

The reviewed revision is
[`73eee2481cf1dd5dea0d8c9c0561b5a61498fec4`](https://github.com/google/closure-compiler/tree/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4),
dated 2026-08-28. The clone used for this audit lives outside the repository in the temporary
workspace, so this report pins web links rather than depending on that local directory.

## Closure implementation map

| Responsibility | Source |
|---|---|
| Production pass order | [`DefaultPassConfig.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/DefaultPassConfig.java) |
| Variable renaming | [`RenameVars.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/RenameVars.java) |
| Name generation | [`DefaultNameGenerator.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/DefaultNameGenerator.java) |
| Variable live-range coalescing | [`CoalesceVariableNames.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/CoalesceVariableNames.java) |
| Ordinary property renaming | [`RenameProperties.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/RenameProperties.java) |
| Type-based property splitting | [`DisambiguateProperties.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/disambiguate/DisambiguateProperties.java) |
| Property cluster state | [`PropertyClustering.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/disambiguate/PropertyClustering.java) |
| Type-based property merging | [`AmbiguateProperties.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/disambiguate/AmbiguateProperties.java) |
| Namespace collapse and aliases | [`InlineAndCollapseProperties.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/InlineAndCollapseProperties.java) |
| Rename map format | [`VariableMap.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/VariableMap.java) |
| Coding conventions | [`CodingConvention.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/CodingConvention.java) |
| Export generation | [`GenerateExports.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/GenerateExports.java) |
| Literal ID generators and maps | [`ReplaceIdGenerators.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ReplaceIdGenerators.java) |
| Optimization loop | [`PhaseOptimizer.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PhaseOptimizer.java) |
| Unused-code removal | [`RemoveUnusedCode.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/RemoveUnusedCode.java) |
| Function inlining | [`InlineFunctions.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/InlineFunctions.java) |
| Call optimization | [`OptimizeCalls.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/OptimizeCalls.java) |
| Parameter optimization | [`OptimizeParameters.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/OptimizeParameters.java) |
| Return optimization | [`OptimizeReturns.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/OptimizeReturns.java) |
| Effect summaries | [`PureFunctionIdentifier.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PureFunctionIdentifier.java) |
| Peephole driver | [`PeepholeOptimizationsPass.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PeepholeOptimizationsPass.java) |
| Condition minimization | [`PeepholeMinimizeConditions.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PeepholeMinimizeConditions.java) |
| Constant folding | [`PeepholeFoldConstants.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PeepholeFoldConstants.java) |
| Known-method folding | [`PeepholeReplaceKnownMethods.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/PeepholeReplaceKnownMethods.java) |
| String replacement | [`ReplaceStrings.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ReplaceStrings.java) |
| Message replacement | [`ReplaceMessages.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ReplaceMessages.java) |
| Dependency pruning | [`DependencyOptions.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/DependencyOptions.java) |
| Chunk graph | [`JSChunkGraph.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/JSChunkGraph.java) |
| Build-time defines | [`ProcessDefines.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ProcessDefines.java) |
| Toggle packing | [`ReplaceToggles.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ReplaceToggles.java) |
| CSS-name replacement | [`ReplaceCssNames.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ReplaceCssNames.java) |
| Dead local assignments | [`DeadAssignmentsElimination.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/DeadAssignmentsElimination.java) |
| Dead property assignments | [`DeadPropertyAssignmentElimination.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/DeadPropertyAssignmentElimination.java) |
| Immutable property inlining | [`InlineProperties.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/InlineProperties.java) |
| Trivial method inlining | [`InlineSimpleMethods.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/InlineSimpleMethods.java) |
| Bracket-to-dot conversion | [`ConvertToDottedProperties.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ConvertToDottedProperties.java) |
| Label shortening/removal | [`RenameLabels.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/RenameLabels.java) |
| Assertion/abstract removal | [`ClosureCodeRemoval.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ClosureCodeRemoval.java) |
| Configured domain stripping | [`StripCode.java`](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/StripCode.java) |

## LilScript implementation map

| Responsibility | Source |
|---|---|
| Typed control-flow IR and property categories | [`src/ir.rs`](../src/ir.rs) |
| Escape and optimization analysis | [`src/optimizer.rs`](../src/optimizer.rs) |
| Main JavaScript emitter/mangler | [`src/codegen_ir_js.rs`](../src/codegen_ir_js.rs) |
| Candidate search and exact codec scoring | [`src/compiler.rs`](../src/compiler.rs) |
| Late local-name convergence | [`src/js_peephole/rename.rs`](../src/js_peephole/rename.rs) |
| Generated-JS binding resolution | [`src/js_peephole/binding.rs`](../src/js_peephole/binding.rs) |
| Generated-JS scope index/remappers | [`src/js_peephole/scope.rs`](../src/js_peephole/scope.rs), [`src/js_peephole/mod.rs`](../src/js_peephole/mod.rs) |
| Configuration | [`src/config.rs`](../src/config.rs), [`lilscript.toml`](../lilscript.toml) |
| Search classification | [`src/decision_registry.rs`](../src/decision_registry.rs) |
| Compression-only IR passes | [`src/compress_passes.rs`](../src/compress_passes.rs) |
| Generated-JS folds | [`src/js_peephole/folds/`](../src/js_peephole/folds/) |
| Existing architectural note | [`docs/knowledge/research/closure-advanced.md`](../docs/knowledge/research/closure-advanced.md) |

## Current versus non-current Closure features

Current production code, when enabled by options:

- `RenameVars`
- `RenameProperties`
- `DisambiguateProperties`
- `AmbiguateProperties`
- `CoalesceVariableNames`
- `InlineVariables` and `FlowSensitiveInlineVariables`
- `InlineAndCollapseProperties`
- `ReplaceIdGenerators`
- `VariableMap`
- `DefaultNameGenerator`

Present but not standard production behavior in this snapshot:

- `DefaultNameGenerator.favors()` has tests but no normal pipeline caller;
- `RandomNameGenerator` exists but standard pass wiring uses `DefaultNameGenerator`;
- the custom `RenameProperties` eligibility predicate is an internal test hook; standard
  production wiring does not supply it;
- there is no active property-affinity heuristic;
- there is no current standalone `ShadowVariables` pass;
- `CollapseProperties` is now implemented inside `InlineAndCollapseProperties`.

Legacy behavior still present in production should not be mistaken for a recommended design:

- missing-property accesses invalidate disambiguation partly for compatibility with the pre-2021
  implementation;
- some constructor/interface/enum collapse exceptions are explicitly described as unsafe legacy
  behavior;
- `InlineVariables.Mode.CONSTANTS_ONLY` remains for compatibility.

## Interpretation rules

- An implemented LilScript option is not necessarily enabled in the checked-in root config.
- A Closure class in the source tree is not necessarily selected by `DefaultPassConfig`.
- Deterministic output is not the same as edit-stable output.
- Fewer property colors do not necessarily mean fewer compressed bytes.
- Raw-size and compressed-size claims require equivalent public/extern boundaries.
- Closure's JavaScript type `Color` and LilScript's nominal owner/index identities are analogous
  inputs to renaming, not equivalent type systems.
