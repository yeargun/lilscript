# Data, modules, and delivery compression

Parent: [comparison index](index.md). Selection model: [objective and search](objective-and-search.md).

## Repeated strings

Closure's generic `AliasStrings` is off by default. Its own documentation says it commonly hurts
gzip and remains useful mainly for unusually repetitive generated strings. Eligibility excludes
templates, protected messages, regexp children, and `"undefined"`; a raw-size formula assumes a
three-character renamed alias. See
[`AliasStrings.java` lines 35-65](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/AliasStrings.java#L35-L65)
and [lines 221-256](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/AliasStrings.java#L221-L256).

LilScript's generic pooling is a better architecture for compressed objectives: cheap raw models
generate candidates, then the complete artifact is scored. It also retries thresholds and includes
an unpooled competitor. See
[`codegen_ir_js.rs` lines 6006-6111](../src/codegen_ir_js.rs#L6006-L6111) and
[`compiler.rs` lines 4645-4690](../src/compiler.rs#L4645-L4690).

The more interesting Closure mechanism is `ReplaceStrings`. Configured function arguments such as
error/log messages become short codes while dynamic pieces survive through placeholders. The pass
builds a decoding map, which the CLI writes only when a string-map output path is configured. See
[`ReplaceStrings.java` lines 38-125](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ReplaceStrings.java#L38-L125)
and [lines 194-349](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ReplaceStrings.java#L194-L349).

This is semantic lossy compression, not pooling. A LilScript equivalent should require an explicit
domain and preserve a reverse map for diagnostics.

## Generated IDs and messages

Closure's `ReplaceIdGenerators` supports sequential inconsistent IDs, consistent IDs, stable
Base64 hashes, XID hashes, caller-supplied maps, previous-build reuse, and pseudo names. It replaces
literal calls/tagged templates and can rewrite object keys. See
[`ReplaceIdGenerators.java` lines 35-138](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ReplaceIdGenerators.java#L35-L138)
[lines 173-255](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ReplaceIdGenerators.java#L173-L255),
and [lines 318-482](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ReplaceIdGenerators.java#L318-L482).

Closure also replaces `goog.getMsg` calls with source or translated literals, chooses fallback
branches, joins adjacent static text, and emits concatenation trees for placeholders. The stock CLI
can do this even without translations to erase runtime message calls. See
[`ReplaceMessages.java` lines 477-571](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ReplaceMessages.java#L477-L571)
and [lines 758-953](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ReplaceMessages.java#L758-L953).

Its late-localization mode optimizes locale-independent code once, substitutes messages late, then
reruns inlining, call optimization, DCE, and constant folding. That two-stage design is useful for
multi-locale builds even when individual string substitutions are not novel.

LilScript has no equivalent typed generated-ID or replaceable-message domains today.

Closure also has a dedicated CSS-name domain. `ReplaceCssNames` accepts whole-name or hyphenated
component mappings, removes `goog.getCssName` calls, and can collect original CSS names. See
[`ReplaceCssNames.java` lines 35-91](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ReplaceCssNames.java#L35-L91)
and [lines 173-188](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ReplaceCssNames.java#L173-L188).

## Build constants and packed toggles

`ProcessDefines` replaces primitive build-time constants such as `goog.DEBUG` and target-assumption
flags, exposing whole branches to constant folding and DCE. See
[`ProcessDefines.java` lines 49-68](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ProcessDefines.java#L49-L68)
and [lines 266-389](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ProcessDefines.java#L266-L389).

`ReplaceToggles` either hard-codes configured booleans or packs runtime toggles into 30-bit words.
For each bit it chooses `word&mask` versus `word>>bit&1` using literal spelling thresholds. See
[`ReplaceToggles.java` lines 82-187](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ReplaceToggles.java#L82-L187).

These are explicit domain transforms, not general constant propagation. They matter because they
remove configuration abstractions before the main optimizer runs.

## Assertions and configured code stripping

ADVANCED options enable removal of Closure assertion calls and abstract-method scaffolding when
Closure primitives are processed. If an assertion result is used, its first argument replaces the
call; otherwise the call disappears. The source explicitly notes that removing assertions is not
provably safe. See
[`ClosureCodeRemoval.java` lines 25-40](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ClosureCodeRemoval.java#L25-L40).

The opt-in `StripCode` pass removes configured debug/logging domains by type and name prefix/suffix.
It can remove declarations, references, object keys, assignments, and calls, and understands
qualified names already flattened with `$`. See
[`StripCode.java` lines 35-50](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/StripCode.java#L35-L50)
and [lines 72-146](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/StripCode.java#L72-L146).

These are explicit semantic build modes rather than ordinary optimization. A LilScript analogue
should use declared feature/debug domains or defines, not naming conventions alone.

## Repeated structural prefixes

`ExtractPrototypeMemberDeclarations` replaces repeated `Class.prototype` prefixes with a global or
per-chunk temporary only after charging fixed declaration and per-cluster setup costs. It groups
contiguous declarations, but explicitly assumes an intervening RHS does not replace the prototype
object; the source documents this as a way to break the transform rather than checking it. See
[`ExtractPrototypeMemberDeclarations.java` lines 30-149](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ExtractPrototypeMemberDeclarations.java#L30-L149)
and [lines 277-426](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/ExtractPrototypeMemberDeclarations.java#L277-L426).

LilScript's generated class/prototype tables and host facades can expose similar repeated prefixes.
The Closure model is a useful candidate generator, but exact artifact scoring should replace its
fixed raw formula.

## Literal packing and table forms

Both compilers pack string arrays through `.split(delimiter)`. Closure uses a local quote-saving
heuristic and prefers common punctuation. LilScript tries six delimiters, chooses the shortest
packed spelling locally, then exact-scores the resulting packed artifact against unpacked and other
emission candidates. LilScript also has dense zero-based integer-to-string return tables and
numeric pooling, which have no direct general Closure counterpart in this audit.

LilScript sources:

- [`codegen_ir_js.rs` lines 12895-12913](../src/codegen_ir_js.rs#L12895-L12913)
- [`codegen_ir_js.rs` lines 25776-25799](../src/codegen_ir_js.rs#L25776-L25799)
- [`codegen_ir_js.rs` lines 8441-8618](../src/codegen_ir_js.rs#L8441-L8618)

## File and module pruning

Closure separates dependency pruning from ADVANCED optimization. ADVANCED alone uses sort-only
dependency management. Strict pruning requires entry points and `PRUNE`; compatibility-oriented
`PRUNE_LEGACY` also retains scripts without declared provides/modules. See
[`DependencyOptions.java` lines 63-139](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/DependencyOptions.java#L63-L139)
and [lines 182-280](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/DependencyOptions.java#L182-L280).

The chunk graph computes transitive dependencies, retains dynamic-import targets as roots, places
shared dependencies, and moves type-only/weak dependencies to a weak chunk. See
[`JSChunkGraph.java` lines 507-757](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/JSChunkGraph.java#L507-L757).

Module rewriting then removes imports/wrappers and globalizes internal names so alias inlining,
property collapse, and DCE can remove scaffolding. See
[`Es6RewriteModules.java` lines 432-683](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/Es6RewriteModules.java#L432-L683).

LilScript links a typed static graph before SSA, removes unreachable functions/globals/imports, and
can split by source/lazy modules. In `Split` mode, mandatory lazy chunks are inserted first and only
optional eager chunks are greedily admitted when complete deployment cost improves;
`PreserveModules` keeps all module candidates. See
[`compiler.rs` lines 879-1047](../src/compiler.rs#L879-L1047).

One Closure behavior worth testing explicitly is dynamic-import rooting: Closure collects dynamic
targets from original inputs, which may retain a target even if its importer later proves
unreachable. LilScript can use linked reachability to be more precise if semantics permit it.

## Cross-chunk motion

Closure moves global declaration SCCs to the deepest common dependent chunk and moves eligible
prototype methods to their deepest use chunk. Method motion normally leaves parent stubs to preserve
prototype enumeration/mixin semantics, which can increase total bytes while reducing startup bytes.

See
[`CrossChunkCodeMotion.java` lines 463-673](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/CrossChunkCodeMotion.java#L463-L673)
and
[`CrossChunkMethodMotion.java` lines 34-200](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/CrossChunkMethodMotion.java#L34-L200).

LilScript's deployment score is more explicit about aggregate transfer, requests, depth, preload,
and cache reuse. Closure's dependency-safe SCC movement and method-enumeration tests are valuable;
its transform should not be called a total-byte win without measurement.

## Target syntax and polyfills

Closure's ordinary transpilation and polyfill selection are feature/target/use-sensitive, although
some normalization always runs and explicit forced-library modes can inject helpers without an
observed use. When modern syntax is legal, a late raise step restores object shorthand and
expression-bodied arrows. See
[`TranspilationPasses.java` lines 68-185](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/TranspilationPasses.java#L68-L185)
and
[`SubstituteEs6Syntax.java` lines 26-77](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/SubstituteEs6Syntax.java#L26-L77).

Normal polyfill rewriting injects by observed unguarded usage and target version.
`RemoveUnusedCode` later removes injected polyfills that did not remain live; explicitly forced
polyfills are exempt from that removal. See
[`RewritePolyfills.java` lines 106-227](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/RewritePolyfills.java#L106-L227)
and
[`RemoveUnusedCode.java` lines 3018-3088](https://github.com/google/closure-compiler/blob/73eee2481cf1dd5dea0d8c9c0561b5a61498fec4/src/com/google/javascript/jscomp/RemoveUnusedCode.java#L3018-L3088).

The compression lesson is broader than polyfills: target the newest deployable syntax, inject
helpers by actual use, and rerun DCE after helper generation.

## Current-state cautions

- `AliasStrings` is implemented but off by default.
- Alias-keyword optimization and the old arguments-array pass are absent.
- `FunctionRewriter` is implemented but off in ADVANCED.
- CommonJS flattening is opt-in.
- Dependency pruning is not implied by ADVANCED.
- Cross-chunk motion optimizes placement, not guaranteed aggregate bytes.
- Polyfill isolation is intentionally late and may increase size.
- Assertion and configured-domain stripping intentionally alter failure/debug behavior.
