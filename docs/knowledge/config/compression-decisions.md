# `javascript.compression`

Parent: [Config](README.md). Emission: [JavaScript emission](../compilation/javascript-emission.md). Passes: [compress passes](../compilation/compress-passes.md).

Exact allowlist of **contested representations**. Omitted → `priority` defaults.
Present → only listed names for the canonical JS/IR options. `[]` → none, except
explicit `[mangle]` overrides for identifier/property/export mangling and string
pooling.

Duplicates are config errors. Names are kebab-case (`CompressionDecision::name`).

## Decisions

| Name | What | Typical on |
|---|---|---|
| `identifier-mangling` | Frequency-ranked short names | all priorities |
| `entropy-aware-mangling` | Codec-pick alphabet + bounded 1-char permutation | not performance-first |
| `quote-style-selection` | `'` vs `"` | not performance-first |
| `property-mangling` | Owned LilScript fields | size-first |
| `export-mangling` | Public ESM names (+ public fields) | never by priority |
| `string-pooling` | Alias repeated strings if raw model agrees | not performance-first |
| `size-aware-inlining` | Apply positive inline-growth cap | not performance-first |
| `safe-integer-coercion-elision` | Drop `|0` when range-proven | not performance-first |
| `compact-boolean-literals` | `!0`/`!1` vs keywords | not performance-first |
| `standard-grammar-elision` | ASI `;`, `new` parens, call-chain parens | all (still searched) |
| `structured-closure-inlining` | Nested structured closures vs helpers; with candidate search, proof-gated private single-use function expressions | not performance-first |
| `pure-helper-inlining` | Emission-only substitution of proven private pure return helpers; `none`, single-static-use, and all-eligible policies compete | size-first |
| `dense-string-return-tables` | Proven zero-based bounded integer guard ladders returning strings may become complete literal lookup tables | size-first |
| `host-alias-spelling` | Direct-only static host callees may use their dotted native spelling at each site instead of a shared binding | size-first |
| `string-array-packing` | `["a","b"]` vs split-string | size-first |
| `regex-literals` | `new Regex` → `/…/` narrow subset; also requires `assume_pristine_builtins` | not performance-first |
| `unused-catch-binding-elision` | `catch {` vs `catch (e)` | all |
| `compact-generator-star` | `function*n` vs `function* n` | all |
| `callee-default-arguments` | Emit JS defaults vs materialize | not performance-first |
| `scalar-phi-copies` | Scalar assigns vs tuple copies | size-first |
| `phi-affinity-coalescing` | Share names across proven non-interfering phis | all |
| `ir-inlining-variants` | Compete fully outlined IR | size-first |
| `ir-closure-factory-variants` | Keep factories, inline elsewhere | size-first |
| `ir-phase-ordering-variants` | No-early-CSE / aggressive inline probes | size-first |
| `loop-spelling-selection` | `while(c)` vs `for(;c;)` | size-first, balanced |
| `mutation-spelling-selection` | `x=x+1` vs `++x` vs `x++` when proven | size-first |
| `array-pipeline-fusion` | Fuse typed array pipelines | size-first |
| `partial-escape-sinking` | Sink allocs that escape on some paths | size-first |
| `region-outlining` | Repeated regions → helpers; canonical pass needs explicit `[optimization]` on, search probe needs no explicit hard-off | size-first |
| `expression-superoptimization` | Bounded pure rewrites | size-first, balanced |
| `path-sensitive-propagation` | SCCP-like facts before fold/DCE | size-first, balanced |
| `joint-representation-search` | Named vs positional and related layouts | size-first |
| `joint-chunk-symbol-search` | Chunk plan × symbol/layout | size-first |
| `parameterized-function-merging` | Merge compatible private functions | size-first + `[optimization]` |

Root `lilscript.toml` lists a **subset**. Because an explicit list replaces priority
defaults, property and export mangling are both off there unless `[mangle]` overrides
them; the root config currently has no such override. Adding a name that priority
would have left off enables that decision, subject to any separate
`javascript.optimizations` and `[optimization]` gates. Omitting a name that priority
would have enabled disables its canonical option. The root includes the narrow
`host-alias-spelling` candidate because `Shared` remains mandatory competition, but
continues to omit broader families such as region outlining and joint representation/
chunk search.

Region outlining follows the same contract. A search probe requires candidate search,
the `region-outlining` compression decision, and no explicit
`[optimization].region_outlining = false`. Thus an exact compression allowlist that
omits outlining cannot have it reintroduced by `ir-compress-pass-variants`.

## Overlap with `javascript.optimizations`

Several IR searches have **both** a compression decision and an optimization feature (`ir-inlining-variants`, closure-factory, phase-ordering, loop spelling, mutation spelling, default arguments, joint searches). `optimization_enabled` requires **level/allowlist AND compression** when the `legacy` decision is passed.

If you put `ir-inlining-variants` in `optimizations` but leave it out of an explicit `compression` list, the search stays off.

## Legality vs score

Enabling a decision means the representation may be emitted. Candidate search still
compares the opposite (disable) when that dimension is in the beam. Example: grammar
elision is on, but punctuated variants remain candidates because fewer raw bytes can
compress worse.

`structured-closure-inlining` also authorizes an emission-only proposal whose
configured baseline is off: a private, nonescaping function with one entry call may
be rendered as an anonymous expression after a whole-program proof. Candidate search
must be on, and the named form still competes. See
[inlining and sharing](../compilation/inlining-specialization-sharing.md#emission-only-single-use-function-expressions).

`pure-helper-inlining` and `dense-string-return-tables` are separate legality
decisions. When both are enabled, search expands the complete helper-policy × table
policy Cartesian family from the same frontier before pruning; it does not require
either isolated intermediate to win. Their configured emitter values remain
`none`/`false`, so the ordinary named guard-ladder artifact is mandatory competition.

`host-alias-spelling` is independently scored and keeps `Shared` as the configured
baseline. `Direct` is legal only for the static callee convention: it replaces the
private alias calls with native dotted calls such as `Object.hasOwn(...)`. Exports,
lazy exports, detached/address-taken values, method/bound conventions, and constructor
uses retain a binding, so enabling the decision cannot silently change function
identity or receiver behavior.
