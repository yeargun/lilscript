# Compiler Configuration

Why knobs exist, precedence, and how they change compilation: [knowledge/config](knowledge/config/README.md). This page is the schema dump.

The CLI discovers `lilscript.toml` by walking from the input module toward the
filesystem root. Pass `--config path/to/config.toml` to select one explicitly.
Unknown keys and invalid numeric limits are errors.

```toml
[compiler.resources]
# threads = 12 # omit to use RAYON_NUM_THREADS or the host/Rayon default
codec_workers = 4 # terminal Brotli finalizer workers; must be nonzero

[optimization]
preset = "maximum" # maximum | none
constant_folding = true
algebraic_simplification = true
common_subexpression_elimination = true
finite_value_propagation = true
global_optimization = true
inlining = true
inline_closure_factories = true
constant_parameter_specialization = true # false keeps generic constant-argument call sites
specialize_tagged_constants = true # include boxed/union constants when specializing
scalar_replacement = true
dead_store_elimination = true
dead_code_elimination = true
call_site_specialization = true
capture_signature_cloning = true
identical_function_folding = true
# function_subsumption = true # explicit all-backend enable; false is a hard disable
# pipeline_fusion = true
# partial_escape_sinking = true
# region_outlining = true
# expression_superopt = true
# path_sensitive_propagation = true
# parameterized_function_merging = true
profile_guided = true
# for_of_specialize_family = 8 # 0 off; residual for-of over the first array parameter emits clones plus a picker

[javascript]
priority = "size-first"
# ecmascript = "es2022" # es2015 | es2016 | ... | es2022 | esnext; omitted = es2022
# browsers = ["chrome80", "firefox78"] # optional; intersected with ecmascript (conservative floor wins)
optimization_level = 15 # 0..15 compiler-effort budget
cost_model = "brotli" # raw | gzip | brotli
pool_numeric_literals = true # alias repeated profitable numeric literals
# integer_coercions = true # keep generated `|0`; source-written `value | 0` is always retained while live
candidate_search = "production" # off | production | always
candidate_limit = 1536
candidate_byte_budget = 1048576 # aggregate whole-artifact search-work budget
candidate_beam_width = 12
# candidate_proposal_limit = 384 # structural plans admitted before emission; 0 disables
# terminal_codec_probe_limit = 384 # terminal whole-artifact work; 0 disables
max_candidate_raw_growth_percent = 0 # raw-side admission allowance; maximum 1000
function_layout_exact_limit = 13 # 0 = heuristic only; maximum 18
local_name_reserve = 48 # consistent short identifiers reserved for lexical locals
stable_local_names = true # preserve source-local affinity across generated kernels
local_name_coalescing = true # reuse bindings for SSA values with disjoint live ranges
# function_spelling = "arrow" # arrow | function; see public-ABI note below
# strip_console = true # drop print()/debugLog; default on. Tests/oracles set false.
# assume_pristine_builtins = false # required before regex literals may bypass ambient RegExp
# public_aggregate_abi = "named" # named | positional; positional requires opaque array handles
# optimizations = ["parsed-peephole", "startup-cost-guard"]
compression = [
  "identifier-mangling",
  "entropy-aware-mangling",
  "quote-style-selection",
  "string-pooling",
  "size-aware-inlining",
  "compact-boolean-literals",
  "standard-grammar-elision",
  "structured-closure-inlining",
  "pure-helper-inlining",
  "dense-string-return-tables",
  "host-alias-spelling",
  "string-array-packing",
  "regex-literals",
  "unused-catch-binding-elision",
  "compact-generator-star",
  "callee-default-arguments",
  "scalar-phi-copies",
  "phi-affinity-coalescing",
  "ir-inlining-variants",
  "ir-closure-factory-variants",
  "ir-phase-ordering-variants",
  "loop-spelling-selection",
  "mutation-spelling-selection",
  "array-pipeline-fusion",
  "partial-escape-sinking",
  "region-outlining",
  "expression-superoptimization",
  "path-sensitive-propagation",
  "joint-representation-search",
  "joint-chunk-symbol-search",
  "parameterized-function-merging",
]
# inline_instruction_limit = 18
# inline_control_flow_limit = 45
# max_inline_growth = 16

[javascript.startup]
parse_weight = 1
compile_weight = 1
memory_weight = 1
# max_nesting = 128 # optional absolute generated-syntax ceiling
parse_overhead_limit_percent = 30
compile_overhead_limit_percent = 30
memory_overhead_limit_percent = 35

[javascript.performance]
deoptimization_weight = 32
allocation_weight = 12
indirect_call_weight = 24
hot_code_weight = 1
max_regression_percent = 25

[profile]
# path = "lilscript.profile.json"
specialization_min_count = 100
max_specializations_per_function = 8
max_clone_instructions = 64

[native]
partial_escape_analysis = true
stack_allocation = true
region_allocation = true
stack_array_element_limit = 64

[mangle]
# identifiers = true
# properties = true # size-first default; set false to keep owned property names
# exports = false
# pool_strings = true # optional explicit override

[bundle]
mode = "single" # single | split | preserve-modules
min_chunk_bytes = 16384
max_chunks = 32
shared_min_imports = 2
preload = "none" # none | entry | all

[bundle.cost]
raw_weight = 0
gzip_weight = 1
brotli_weight = 2
request_overhead_bytes = 1000
dependency_depth_penalty_bytes = 160
preload_request_discount_percent = 70
cache_reuse_discount_percent = 20

[lint]
enabled = true
preset = "recommended" # minimal | recommended | strict
deny_warnings = false
providers = ["correctness", "effects", "performance", "size", "web"]
exclude = ["**/generated/**"]
pure_extern_allowlist = ["auditedHostFunction"]

[lint.rules]
"performance/allocation-in-loop" = "warn" # off | hint | warn | error

[format]
enabled = true
line_width = 100
newline = "lf" # lf | crlf
organize_imports = true
```

`compiler.resources.threads` creates a local Rayon pool for one configured
JavaScript compilation. Omitting it preserves the process-global Rayon policy,
including `RAYON_NUM_THREADS`. `compiler.resources.codec_workers` defaults to
`4` and is capped by the active pool size. It bounds terminal Brotli plan
finalization, where every worker finalizes its assigned plans serially, and the
same ceiling applies to terminal binding-remap codec probes.
Selected-model candidate scoring uses the active Rayon pool, while the short
entropy-alphabet source searches stay serial to avoid multiplying Brotli-11
workspaces. These controls change scheduling and peak concurrent working state,
not the candidate frontier or the selected artifact. The CLI overrides TOML
with `-j N` / `--jobs N` and `--codec-jobs N`.

`javascript.candidate_limit`, `candidate_byte_budget`,
`candidate_beam_width`, `candidate_proposal_limit`, and
`terminal_codec_probe_limit` are different: they bound search effort and retained
whole-artifact source bytes, so changing them can change the selected output.
`candidate_limit` is the retained-frontier count, not an attempted-work count.
Terminal scope-naming and string-pooling challengers debit the remaining shared
plan and source-byte ledger before codec scoring; the already-retained incumbent
remains eligible when that tail is exhausted.
`candidate_byte_budget` is not a promise about total process RSS; optimizer IR
clones and codec workspaces are outside that accounting.
`candidate_proposal_limit` is charged when a new structural plan identity is
admitted, before IR-to-JavaScript emission. Failed emissions and candidates
later rejected by syntax, size, or codec ranking therefore still consume a
slot. Already-scored IR context seeds are outside this optional budget, and a
separate terminal tail remains available for factored naming/declaration
challengers. Omitted defaults additionally honor `candidate_limit` and scale to
one quarter for 16–64 KiB artifacts and one twelfth above 64 KiB. An explicit
value can exceed the survivor count and bypass artifact scaling, but it cannot
raise the optimization-level or `candidate_search` tier.
`terminal_codec_probe_limit` is the shared terminal-search work ceiling after
structural plans have been emitted. Parsed-peephole, cleanup, and binding-remap
families share the same counter. The current post-selection canonical peephole
may perform one additional codec comparison outside it. A proposal is charged before whole-artifact
repair/validation, and each exact-codec call also requires an admitted unit.
Exhaustion skips remaining leaves and retains the best already-scored artifact. A rare
missing score for the mandatory configured incumbent is measured outside this
optional budget so the fallback cannot disappear. Omitted defaults use artifact
scaling; an explicit ceiling bypasses that scaling while remaining bounded by
the optimization-level and search tier.

Every optional optimization key overrides its preset independently. The
`none` preset disables optional transforms but retains mandatory IR
normalization and correctness analyses. This makes it useful for debugging and
for isolating pass regressions without changing language semantics.

`finite_value_propagation` controls the bounded interprocedural lattice for
booleans, strings, nullable `null`, and owned nominal fields. The default is
enabled. Facts widen after four alternatives and become unknown at exported,
extern, indirect-call, closure, or untyped aggregate boundaries.

`identical_function_folding` runs after specialization and inlining decisions.
It redirects directly called private functions with identical normalized CFGs
and compatible escape states to one implementation. Exported, address-taken,
method, constructor, closure, and host-visible identities are excluded. For
JavaScript, level-derived or exact `identical-function-folding` selection can
additionally bound this late
whole-program work; native optimization uses the semantic pass switch directly.

`function_subsumption` controls proof-driven implementation sharing. A private,
direct-call-only function may be redirected to an existing function with extra
parameters only when binding those parameters to typed scalar literals or known
direct functions makes the normalized SSA/CFG bodies exactly equal. Calls receive
explicit arguments without permuting source argument evaluation; LilScript
never relies on JavaScript omitted-argument behavior. Exports, address-taken
functions, methods, constructors, closures, type mismatches, and non-equal
bodies are rejected. The default leaves native
output unchanged and lets size-first JavaScript level 14+ compare transformed
and untouched IR under the selected codec. `function_subsumption = false`
suppresses that candidate; `true` enables the pass for native output and permits
it for every JavaScript priority when the level-derived or exact JavaScript
feature is enabled.

`javascript.priority` is a JavaScript-target policy. It never weakens semantic
checks, mandatory IR normalization, DCE correctness, or host-boundary rules:

- `performance-first` uses straight-line/control-flow limits of `24`/`60`, has
  no inline-growth cap, disables automatic string pooling, and keeps signed-i32
  `|0` normalization on ordinary `int` math.
- `realistic-performance-first` uses limits of `18`/`45`,
  allows up to `16` estimated additional IR instructions from repeated-call
  inlining, and enables profitable string pooling. It keeps `|0`.
- `balanced` uses limits of `12`/`30`, permits up to `4` estimated additional
  instructions, and enables profitable string pooling. Proven-redundant `|0` is
  dropped; `|0` does not help gzip/Brotli.
- `size-first` is the default. It uses limits of `12`/`30`, permits up to `16`
  temporary IR instructions of inline growth so the following fold/DCE fixed
  point can expose a net byte win, enables profitable string pooling, enables
  owned-property mangling while leaving public export names stable, and
  considers delimiter-packed string literal tables. Packing adds startup work,
  so the performance-oriented profiles leave it disabled. Proven-redundant `|0`
  is dropped. Set `integer_coercions = true` to keep it.

`javascript.strip_console` defaults to `true` so production JavaScript does not
ship `print()` as `console.log`. Language tests and the root `lilscript.toml`
oracle set `false`. `debugLog` is also dropped; argument side effects stay.
`console.warn` is not stripped. Policy: [javascript.priority](knowledge/config/javascript-priority.md).

`javascript.function_spelling` is an explicit JavaScript ABI and spelling
override. When omitted, exported functions retain ordinary-function
constructibility while the compressor search may choose arrows or function
declarations for private bindings. `"function"` forces ordinary functions.
`"arrow"` also permits public arrows, which removes `prototype` and rejects
construction with `new`; use it only when the selected public API is itself
nonconstructible (for example Nano ID's published browser arrows). The
benchmark verifier checks arity and constructibility before a result is
eligible, so this setting cannot silently buy bytes by changing that API.

`javascript.public_aggregate_abi` defaults to `"named"`: structs and classes
that cross a reusable JavaScript boundary use stable named fields, including
aggregate types reachable through their public fields. `"positional"` emits
compact array-backed handles instead. It is an explicit ABI choice for modules
whose JavaScript consumers treat every exported aggregate as opaque and only
pass handles back to compiled functions; JavaScript must not inspect fields or
construct those handles as objects in that mode.

`javascript.ecmascript` is the JavaScript *syntax* floor (`es2015`…`es2022`,
or `esnext`). It is independent of CLI `--target js`. Omitted values match the
historical backend (`es2022`), so existing goldens stay byte-stable. Optional
`javascript.browsers` tokens (`chrome80`, `firefox78`, `safari14`, `edge80`)
intersect with that edition; the most conservative floor wins. Unknown tokens
are config errors. The floor is ES2015: there is no ES5 mode and no polyfill
emission. If a required construct has no exact older spelling, compilation
fails rather than emitting illegal JavaScript. Comparison and benchmark
baselines stay `es2022` unless a case is explicitly about a lower target.

`javascript.compression` overlays named size tactics on the selected
`javascript.priority` defaults. If omitted, the profile supplies the list.
Listing a name turns that tactic on even when the profile would leave it off.
`compression = []` disables all of them. Canonical options still follow the
listed names when the table is present. Size-first **search-only** spellings
such as `indexed-char-at` still compete unless the list is empty. Proven `|0`
is not in that bargain: size-first and balanced still drop it unless
`integer_coercions = true`.

- `identifier-mangling` assigns short names by whole-program use frequency.
- `entropy-aware-mangling` compares the canonical identifier alphabet with an
  alphabet ranked by emitted-character frequency, then lets the configured
  exact compressor choose the result. Size-first builds also run a bounded,
  deterministic permutation search over one-character emitted identifiers.
  The trial budget scales down with artifact size so quality-11 codec probes
  do not make large modules impractical; every proposed alphabet is re-emitted
  through the normal scope-aware mangler before it can be selected.
- `quote-style-selection` compares semantically equivalent single- and
  double-quoted string literals.
- `property-mangling` renames LilScript-owned properties. Escape-owned aggregates
  that cross an `extern` boundary may rename without `export-mangling`; ESM
  export-surface aggregate field names stay stable unless `export-mangling` is
  listed or `mangle.exports` is set. Size-first enables property mangling by
  default. Other priorities leave it off unless an exact allowlist or
  `mangle.properties` opts in.
- `export-mangling` permits public ESM export names to be shortened.
- `[mangle] extern_fields` (default on) keeps `extern class` member names exact so a JS library ABI stays readable. Set `false` for a closed LilScript program; host members such as `string.length` still do not mangle.
- `array-pipeline-fusion` fuses eligible same-block `map`→`map` chains.
- `partial-escape-sinking` sinks LocalOnly allocations into the single Branch
  arm that uses them.
- `region-outlining` extracts repeated pure instruction regions into helpers.
- `expression-superoptimization` applies bounded pure Int/Bool rewrites.
- `path-sensitive-propagation` runs sparse conditional constant propagation.
- `joint-representation-search` competes named vs positional aggregate spelling.
- `joint-chunk-symbol-search` scores chunk plans against layout and name-reserve
  emission variants under deploy cost.
- `parameterized-function-merging` merges permuted-parameter and single-operand-
  divergent private functions.
- `string-pooling` aliases repeated strings only when the emitter's raw-size
  model predicts a reduction.
- `size-aware-inlining` applies the profile's positive-growth limit to repeated
  straight-line calls.
- `safe-integer-coercion-elision` is not a transfer tactic. `|0` never helps
  gzip/Brotli of served code, so size-first and balanced drop proven-redundant
  coercions even when this name is omitted from an exact allowlist.
  `performance-first` and `realistic-performance-first` keep `|0`. Set
  `javascript.integer_coercions = true` to keep it on size-first or balanced.
  Overflow-capable operations still wrap.
- `compact-boolean-literals` compares `!0`/`!1` with `true`/`false` for
  surviving boolean constants and typed default fields.
- `standard-grammar-elision` permits three independent, standards-valid
  emission choices: block-terminal semicolons supplied by ECMAScript ASI,
  empty parentheses on zero-argument `new`, and redundant grouping around a
  call before member/index access. Candidate search keeps punctuation-retaining
  variants because fewer raw bytes can still be worse under gzip or Brotli.
  This does not enable malformed JavaScript, Annex B, sloppy-mode globals,
  `with`, `eval`, or browser-only syntax recovery.
- `structured-closure-inlining` compares compact nested structured closures
  with reusable outlined helpers under the selected compressor.
- `pure-helper-inlining` lets candidate search substitute a proof-gated private
  pure return-helper DAG at its static calls. It compares the named baseline,
  single-static-use substitution, and all-eligible substitution; cached effectful
  arguments, public identity, recursion, captures, exception regions, and host or
  allocating operations are refused.
- `dense-string-return-tables` lets candidate search replace a same-selector
  integer equality ladder returning constant strings with a complete literal
  table. Integer analysis must prove the entire zero-based domain (maximum 256
  entries); default-filled gaps make every lookup an own array element, so the
  rewrite does not depend on `Array.prototype`.
- `host-alias-spelling` compares a shared top-level binding with the native dotted
  spelling at each call site for direct-only static host callees such as
  `Object.hasOwn`. A detached function value, export, method/bound convention,
  constructor use, or lazy-module boundary keeps its binding; the configured
  baseline is shared and exact whole-artifact codec scoring may select direct.
- `string-array-packing` considers immutable literal tables such as
  `["a","b"]` as a delimiter-joined string plus `.split()`. It is a size/startup
  tradeoff and remains a compressor-scored candidate rather than a mandatory
  lowering.
- `regex-literals` replaces `new Regex(pattern, flags)` only for a statically
  valid, use-complete, shorter subset when
  `javascript.assume_pristine_builtins = true`. Open-world output keeps the
  constructor because a literal bypasses the ambient `RegExp` binding. Complex
  or potentially invalid ECMAScript patterns retain construction and exception
  timing.
- `unused-catch-binding-elision` emits `catch { ... }` instead of
  `catch (name) { ... }` only when semantic use counts prove the catch binding
  is unused. Source clauses without bindings already use the shorter grammar.
  Whole-artifact candidate search also retains the explicit-binding variant,
  because raw deletion can still lose under a selected transfer codec.
- `compact-generator-star` compares the standards-equivalent generator
  declaration spellings `function*name` and `function* name`. Candidate search
  keeps the spaced form: removing whitespace is not accepted merely for a raw
  byte win when gzip or Brotli scores it worse.
- `callee-default-arguments` compares private JavaScript parameter defaults
  with materializing the typed default at each direct call. Exported functions
  keep full-arity signatures so `Function.length` and omitted-call behavior do
  not change.
- `scalar-phi-copies` lets cyclic SSA parallel copies compete as scalar
  assignments against tuple destructuring. The scalar scheduler reuses a
  liveness-proven dead local for cycle breaking when one exists. Size-first
  enables the comparison; omitting this decision keeps tuple copies.
- `phi-affinity-coalescing` lets direct phi inputs share their destination name
  when normal liveness proves the pair does not interfere. Candidate search
  compares conservative deferred-expression interference, direct affinity,
  and contracted non-interfering phi groups because fewer raw assignments can
  still compress worse. A loop-carried update cannot overwrite its old value
  in place when a sibling phi still consumes that value on the same edge; the
  parallel-copy dependency remains a hard correctness constraint in every
  effort profile.
- `ir-inlining-variants` lets the configured inlining pipeline compete with a
  fully outlined IR under the exact selected codec. It is enabled by
  size-first, applies to single-file and reusable ESM output, and is omitted by
  performance-oriented profiles because it runs a second optimizer pipeline.
- `ir-closure-factory-variants` adds a partial-inlining IR that preserves
  straight-line factories returning closures while retaining ordinary and CFG
  inlining everywhere else. Exact codec scoring chooses between reusable
  factory environments and capture-specialized closure sites. The independent
  `[optimization] inline_closure_factories` switch disables factory inlining
  for every backend when an explicit policy is required.
- `ir-phase-ordering-variants` lets size-first builds compare the configured IR
  against bounded aggressive-inlining candidates, both with and without early
  common-subexpression elimination. This avoids materializing reusable
  temporaries before later inlining duplicates or exposes their expressions.
  These probes start only from the configured and unspecialized pipelines;
  they are not multiplied across unrelated outlining, capture-cloning, call-
  specialization, or function-subsumption toggles. Modules above the bounded
  function/IR-size threshold retain one combined unspecialized + no-early-CSE
  + aggressive-inlining proposal instead of six additional complete emission
  searches.
  The selected raw/gzip/Brotli codec scores complete artifacts; startup and
  performance-shape guards still apply. Omit this decision, lower
  `optimization_level` below 14, or disable candidate search for faster builds.
- `loop-spelling-selection` lets equivalent condition-only loops compete as
  `while(condition)` and `for(;condition;)`. They have equal raw spelling
  length, but different token context under gzip and Brotli. Size-first scores
  both forms for the best eight final-emission candidates; other profiles keep
  the frequency heuristic and avoid the extra emissions.
- `mutation-spelling-selection` compares assignment, prefix, and postfix forms
  for loop-carried increments. The shorthand is eligible only when SSA proves
  the add feeds one phi edge, its result is otherwise unused, and integer range
  analysis proves signed-i32 normalization unnecessary. Overflow-capable or
  observed increments retain explicit assignment and coercion.
- `indexed-char-at` lets size-first search compete proven in-range
  `string.charAt(i)` against `s[i]`. Canonical emission stays `.charAt`. Out of
  range the two spellings differ (`""` vs `undefined`), so the alternative is
  admitted only with constant-string length facts or a length-bounded loop
  (`i < s.length`). A snippet
  atlas win is not a ship gate; complete-artifact scoring decides.
- `effect-ternary` lets search compare discarded `if(x)a();else b()`
  against `x?a():b()`. Canonical emission already recovers that ternary when
  `conditional_expressions` is on; listing the decision only admits the
  statement-shaped alternative. No priority preset enables this search: the
  statement form lost raw/gzip/Brotli on the measured artifact.
- `array-pipeline-fusion` fuses eligible typed array `map`/`filter`/`reduce`
  pipelines when the exact selected codec prefers the fused form. Size-first
  enables it; other priorities keep the unfused IR unless listed.
- `partial-escape-sinking` sinks or scalarizes allocations that escape only on
  some paths. Size-first enables the comparison.
- `region-outlining` outlines repeated pure or effect-equivalent statement
  regions when helper calls win the codec objective. Size-first enables it.
- `expression-superoptimization` searches bounded rewrites of pure scalar and
  string expressions. Size-first and balanced enable it.
- `path-sensitive-propagation` adds relational/path-sensitive constants before
  ordinary fold and DCE. Size-first and balanced enable it.
- `joint-representation-search` compares aggregate, closure-environment, and
  related layout forms under escape facts and codec cost. Size-first enables it.
- `joint-chunk-symbol-search` scores chunk boundaries together with symbol
  assignment and declaration layout. Size-first enables it.
- `parameterized-function-merging` extends private-function sharing across
  compatible parameterizations. Size-first enables it when
  `[optimization] parameterized_function_merging` remains on.

The numeric `inline_instruction_limit`, `inline_control_flow_limit`, and
`max_inline_growth` keys override the selected profile. Setting
`max_inline_growth` explicitly also enables the growth guard, even when
`size-aware-inlining` is absent from the allowlist. These are IR instruction
budgets, not output-byte limits.

`javascript.optimization_level` controls JavaScript search effort from `0` to
`15`; it does not weaken type checking or the selected `[optimization]` IR
passes. Levels progressively raise the candidate cap and enable additional
dimensions. Level 0 emits one configured layout, levels 4-8 add inexpensive
conditional, update, mutation, SSA, comma, and entropy choices, levels 9-12 add
parsed peepholes plus structural IR/loop/switch alternatives, level 13 adds
late identical-body folding, declaration layout, and joint representation
search, and levels 14-15 add proof-driven function-subsumption IR candidates,
compress-pass variants, and joint chunk/symbol search. Effective count, byte,
and beam caps are always the lower of their level tier and configured ceiling.
Omitted proposal and terminal-work caps additionally scale by artifact size;
explicit ceilings bypass only that artifact scaling and stay within the
level/search tier. This remains true with an exact `optimizations` allowlist:
the list chooses behavior, while `optimization_level` chooses effort.
`candidate_beam_width` controls how many distinct leading
emission layouts advance to each subsequent structural decision. Raising it
can recover interactions whose first step is not locally best; lowering it
reduces complete-artifact emissions and compressor work. It must be greater
than zero and is always bounded by the effective candidate limit.

Typical effort settings are:

```toml
# Fast edit/build loop: one configured emission, with optional terminal
# exact-codec work disabled.
[javascript]
optimization_level = 0
candidate_search = "off"
candidate_limit = 1
candidate_byte_budget = 1
candidate_beam_width = 1
candidate_proposal_limit = 0
terminal_codec_probe_limit = 0
```

```toml
# Maximum release search: all level-derived dimensions, the full configured
# cap even outside normal production mode, and a wider interaction beam.
[javascript]
optimization_level = 15
candidate_search = "always"
candidate_limit = 1536
candidate_byte_budget = 67108864
candidate_beam_width = 48
candidate_proposal_limit = 1536
terminal_codec_probe_limit = 1536
cost_model = "brotli"
```

At level 15, `candidate_search = "always"` defaults the terminal tier to 1536
even when the explicit key is omitted. Production remains capped at 384.

The checked-in default sits between these at level 15, `production` search,
an effective 384-candidate cap shared across all IR optimizer variants, a 1 MiB
aggregate candidate byte budget, a beam width of 12, and at most 384 optional
structural proposals plus 384 optional terminal work units for artifacts up to
16 KiB. Both work defaults scale to one quarter through 64 KiB and one twelfth
above that. The byte budget is
divided across optimizer variants and converted to a candidate count from each
variant's configured baseline size. Thus tiny outputs can exhaust the count
cap, while broad outputs automatically run fewer whole-artifact emissions and
structural scores. At least the configured output from each retained IR
variant is always measured. Initial representation cross-products are bounded
before full emission and codec probing. A small terminal plan/byte slice is
reserved before structural retention so the selected incumbent can still expose
its factored naming/declaration challenger. Raise `candidate_byte_budget`,
`candidate_proposal_limit`, and `terminal_codec_probe_limit` for slower
maximum-compression releases. These controls change
compiler work and representation search only; they do not disable type checks
or mandatory correctness normalization.

`javascript.optimizations` replaces the level-derived feature set with an exact
allowlist. This is separate from the older `compression` policy: `compression`
controls whether a representation is permitted, while `optimizations` controls
which alternative searches and post-emission analyses are run. Available names
are `ir-inlining-variants`, `ir-closure-factory-variants`,
`ir-phase-ordering-variants`,
`ir-function-subsumption-variants`, `ir-specialization-variants`,
`structural-control-flow-variants`,
`ssa-destruction-variants`, `conditional-expression-variants`,
`expression-phi-region-variants`, `local-phi-expression-region-variants`,
`phi-edge-value-forwarding-variants`, `constructor-initializer-fusion-variants`,
`fresh-literal-factory-inlining-variants`, `default-argument-variants`,
`comma-expression-variants`, `structural-loop-variants`, `do-loop-variants`,
`update-loop-variants`, `switch-lowering-variants`,
`compound-mutation-variants`, `entropy-cross-scope-reuse`,
`entropy-property-assignment`, `function-layout-variants`, `parsed-peephole`,
`startup-cost-guard`, `ir-compress-pass-variants`, `joint-chunk-symbol-search`,
and `joint-representation-search`.
The remaining names are `performance-shape-model`,
`profile-guided-optimization`, `call-site-specialization`, and
`capture-signature-cloning`, plus `identical-function-folding`.
An empty list disables all of these features. Duplicate names and levels above
15 are configuration errors. An exact allowlist does not imply exhaustive work:
the level-derived count, byte, beam, and terminal-codec effort tiers still
apply. Set level 15 plus `candidate_search = "always"` and explicit larger
ceilings for a deeper laboratory run. It is exhaustive only if the report names
a finite candidate domain and records that the domain was fully enumerated.

`fresh-literal-factory-inlining-variants` (minimum level 5) is a late,
emitter-local candidate for an unexported ordinary zero-argument function whose
complete body is `return []` and whose only uses are zero-argument direct calls.
It replaces every call with a distinct `[]` and suppresses the now-unobservable
declaration; captures, async/generator functions, address taking, method or
constructor use, and any other body shape are refused. At most the best two
complete structural/name layouts are re-emitted with this option, then the exact
configured raw/gzip/Brotli objective selects the compiler's one output. This is
part of the same compiler invocation, not a downstream Terser/Oxc stage. Explicit
chunk plans conservatively retain the factory until chunk ownership/import
planning carries the same declaration-suppression proof.

`ir-function-subsumption-variants` is automatically searched only by
`size-first`; `balanced`, `realistic-performance-first`, and
`performance-first` require this exact feature name or
`optimization.function_subsumption = true`. This is the explicit control for a
semantics-preserving size transform that may add scalar or function arguments
at surviving call sites. The unmodified IR always remains a complete-artifact
candidate.

`function-layout-variants` proposes two declaration orders from repeated
emitted eight-byte runs. The adjacency order uses exact Held-Karp dynamic
programming through `function_layout_exact_limit` declarations and bounded
deterministic insertion for larger groups. The default is 13; `0` always uses
the bounded heuristic, while release builds can raise the cutoff to at most 18
when the exponential compile-time and memory cost is acceptable.
The window order additionally discounts or rejects similarities beyond the
selected codec's history: 32 KiB for gzip and 4 MiB for the configured Brotli
encoder. These remain proposal mechanisms: unchanged source order stays in the
beam, and the exact configured raw/gzip/Brotli model scores every complete
artifact before selection.

`local_name_reserve` keeps the first N mangled spellings out of module-scope
function/global assignment, then releases them inside each lexical function
scope. This makes structurally similar functions use a consistent short local
alphabet, improving raw size and cross-function gzip/Brotli matches. Module
bindings remain collision-free, referenced globals are still reserved in every
function that uses them, `0` disables the reservation, and the maximum is 256.
With production candidate search active, reservations `0`, `8`, `16`, and `32`
also compete with the configured value. Exact raw/gzip/Brotli scoring can
therefore choose a compact-module alphabet without discarding a larger
configured reservation that benefits broad reusable surfaces.
`stable_local_names = true` assigns the available spellings to interference
colors using non-semantic source-local affinity, with deterministic definition
order as the fallback. It does not alter liveness or the number of slots; it
makes duplicated numerical and generated kernels retain similar local
spellings for transport compression.
`local_name_coalescing = true` (the default) lets identifier-mangled JavaScript
reuse one local binding for SSA values whose live ranges provably do not
interfere. Setting it to `false` retains distinct bindings; liveness and
interference remain hard correctness constraints in both regimes. Unmangled
output keeps source-oriented names and does not use this switch.
Maximum-effort SSA-destruction search may retain both mangled regimes through
finalization. The exact whole-artifact raw/gzip/Brotli scorer then chooses
between them because the coalesced form's reassignments and the uncoalesced
form's declarations can have codec-dependent costs.

`max_candidate_raw_growth_percent` participates in candidate admission both
within one emitted-IR search and across optimizer variants. Under the `raw`
cost model it is a hard raw-size boundary relative to the configured baseline.
Under `gzip` or `brotli`, a candidate is admitted when its transfer bytes do
not exceed baseline **or** its raw bytes are within this allowance. Therefore
the default `0` can still admit raw growth when gzip/Brotli does not regress.
Raising the percentage (up to 1000) widens the raw-side fallback; the unchanged
baseline remains a candidate.

The parsed peephole validates the complete generated artifact and Pratt-parses
eligible expressions before rewriting. It contracts AST-proven simple-local
`x=x op y` statements to compound assignments, removes only unreferenced
function-scoped bindings, fuses adjacent same-kind declarations, folds
two-return arrow guards to conditional expressions, and rotates a generated
`flag=true; while(flag) { ...; flag=condition }` only when token/use analysis
proves the flag is synthetic and the loop has no `continue`. It does not use
unparsed text substitutions. The startup guard compares deterministic syntax-derived
parse, engine-compile, and memory estimates against the configured baseline.
The three overhead limits are hard rejection thresholds, while the three
weights break equal-transfer-size ties. Optional `max_nesting` is an absolute
candidate ceiling and remains active even when `startup-cost-guard` is not in
an exact optimization allowlist; `0` is invalid. `--explain human` and
`--explain json`
report the selected syntax metrics, candidate count, rewrite count, selected
codec bytes, typed-IR performance metrics, and measured LilScript compiler
time.

The performance shape model is deterministic static analysis, not a browser
measurement. It weights state-machine control flow, dynamically shaped values,
host operations, allocations, unresolved indirect calls, and known direct or
closure calls. `size-first` keeps exact transfer bytes as its primary key;
`balanced` combines normalized transfer and shape scores;
`realistic-performance-first` adds an over-limit bucket penalty to normalized
transfer before using the performance ratio as its next key; it does not hard-reject
an over-limit candidate. `performance-first` ranks the shape score first. The four
weights allow a project to tune that proxy without changing language semantics.

`[profile]` accepts an optional version-1 JSON file plus inline `functions` and
`loops` tables. Inline counters override file counters. Generate all stable
keys without annotating source:

```sh
lilscript src/main.lil --profile-template lilscript.profile.json
```

Function keys are `$entry`, a function name, `Class.method`,
`Class.constructor`, or a source-span-keyed closure. Loop keys append the
structured shape index, such as `$entry#0`. Counters must be positive. A hot
or statically byte-profitable direct call can clone a bounded callee for
constant and known-function arguments; constant closure captures can clone the
closure body and remove its environment slots. Each clone re-enters ordinary
folding and DCE and is bounded by the profile limits. The clones are transforms
within an optimizer pipeline, not independently codec-accepted functions. When
candidate search is active, disabled-specialization optimizer variants can let
the complete specialized and unspecialized artifacts compete. The corresponding
`[optimization]` switches are authoritative global gates; a JavaScript effort
level or exact feature allowlist cannot re-enable a pass explicitly set to
`false` there.

`javascript.cost_model` selects the exact objective used by optimizer-IR and
bounded final-emission candidate search. `raw` compares emitted bytes, `gzip` uses level 9, and
`brotli` uses the statically bundled official Google Brotli C 1.1.0 encoder in
generic mode, quality 11, with `lgwin = 22`. `gzip` uses statically bundled
upstream stock zlib C 1.3.1 at level 9 with deterministic `mtime = 0` framing.
Compiler selection and `lilscript-codec` share the same library measurement
functions; hard-gate verification invokes that batch scorer. Node's built-in codec
sizes are diagnostics.
The checked-in Cargo environment forces the bundled zlib path and disables
libz-sys's earlier vcpkg probe on Windows. This canonical-provenance guarantee
currently covers the Linux, macOS, and Windows release targets. Android, Haiku,
and OpenHarmony use libz-sys's platform-zlib path and must not publish canonical
LilScript codec measurements without an additional vendoring or rejection gate.
Under `size-first`, exact transfer bytes are the
primary rank key and performance breaks only exact transfer ties. Final ties use
the configured startup score, raw bytes, and then lexical output order. The search only disables already enabled
contested tactics for comparison; it never turns on a tactic omitted from the
exact `compression` allowlist. `candidate_search = "production"` is the
default. CLI `--mode development` forces multi-IR/emission candidate expansion off
for every configured search value, including `always`. `off` still runs the configured
optimizer/emission and mandatory validation, but it grants zero optional terminal
codec probes: parsed-peephole and binding-remap leaves cannot enter exact-codec
search. Configured profile/startup/performance features remain active. The current
search space compares profitable string pooling, literal-table packing,
numeric-literal pooling, boolean literals,
conservatively proven regular-expression literals,
structured closures, identifier alphabets, adaptive local-name reservations,
quote styles, and equivalent top-level declaration,
phi-affinity, SSA parallel-copy, conditional/comma, structured/state-machine,
`while`/`for`/`do`, update-clause, switch/conditional-dispatch, and assignment/
prefix/postfix/compound-mutation layouts, bounded by the effective candidate
limit. `candidate_beam_width` sets the cross-dimension search window, and
`terminal_codec_probe_limit` bounds the shared post-emission exact-codec tail; the
configured baseline is always retained as a startup-safe fallback. Transfer
scores already measured during search are reused when the parsed peephole
leaves a finalist unchanged, avoiding a second quality-11 compression pass.

When an intermediate emission pool must be truncated, the compiler visits rankings
for the selected objective, raw, gzip, and Brotli round-robin, with the selected
objective first and duplicate artifacts skipped. Structural finalist selection,
entropy sources, and one-character identifier mappings use the same bounded
objective stratification. Final selection still uses only the configured cost model
and priority. Optimizer-IR probes have their own selected-objective beam. Therefore
production output is the best artifact found within configured proposal/count/byte/
beam budgets, not a mathematical global minimum; only small test-only exhaustive
candidate spaces can prove an exact optimum.

The priority is applied after `[optimization]`: setting `inlining = false`
disables inlining in every profile. Explicit `[mangle]` values have the highest
precedence, followed by the exact compression allowlist, then profile defaults.
The aliases `realisticperf-first` and `realistic-perf-first` are accepted for
`realistic-performance-first`. Raw, gzip, and Brotli sizes can disagree, so
size policy is a compiler cost-model preference rather than a universal
guarantee for every compressor and workload. Measure release artifacts with
the intended transport compression.

The policy affects only JavaScript. A configured `--target all` build shares
parsing and semantic analysis, then optimizes separate JavaScript and native IR
copies. Changing `javascript.priority` therefore does not change generated C or
the native executable's optimizer policy.

`[native]` controls conservative partial escape analysis in the C/native
backend. Fixed local arrays up to `stack_array_element_limit`, non-escaping
class values, and eligible captured closures use function-frame storage.
Larger bounded local arrays use a function region when enabled. Values that are
returned, stored globally, captured across an unsafe boundary, passed to an
unknown call, merged through a phi, or resized remain heap allocated. Every
region is released on every generated return path. These switches alter
storage placement, not source-visible ownership semantics.

`mangle.properties` renames eligible LilScript-owned fields. Named aggregate
fields that cross an untyped JavaScript boundary remain stable unless
`mangle.exports` is also explicitly enabled; this keeps the default reusable
JavaScript ABI constructible and inspectable. Members declared by `extern
class` are host ABI names and are never renamed. Internal struct and class
fields already lower to scalar values or numeric slots. `mangle.exports`
removes stable public ESM names (and permits public aggregate-field mangling)
and is intended for LilScript-only applications whose static imports are linked
before codegen.

The bundle policy is separate from optimizer policy. Every mode first links and
optimizes the complete static module graph, so cross-file inlining, scalar
replacement, and DCE happen before a chunk boundary is selected.

- `single` emits one whole-program artifact and has no chunk overhead.
- `preserve-modules` emits surviving, movable dependency functions in one
  static ESM chunk per source module. Root functions and functions that assign
  module globals remain in the entry chunk. Size/import/count limits do not
  override source-module preservation.
- `split` considers modules imported by at least `shared_min_imports` distinct
  modules, rejects optional chunks smaller than `min_chunk_bytes`, then scores
  complete emitted plans. The score combines weighted raw/gzip/Brotli bytes,
  request overhead, dependency depth, preload behavior, shared reachability,
  and cache reuse. Every optional eager/shared chunk, including the first, must
  strictly lower complete deploy cost. Mandatory lazy chunks count toward
  `max_chunks`; compilation fails if the required lazy graph already exceeds
  that cap. `preserve-modules` remains exempt from the split-mode cap.

For `--target js` and `--target js-module`, `split` and `preserve-modules`
require `--output`. `--target all` instead keeps its implicit output behavior:
when `--output` is omitted, it derives `<input-stem>.js` as the bundle entry.
These modes write the entry module, sibling chunks, and
`<entry-stem>.manifest.json`. Static imports load eagerly.
`import("./feature")` creates a typed asynchronous module task and a mandatory
lazy chunk for a lazy-only module. `preload = "entry"` preloads chunks directly
requested by the entry artifact; `all` preloads every lazy root. Manifest v2
contains deterministic build/cache hashes, exact transport sizes, dependency
edges, and deploy-cost values. Use an `.mjs` output when running directly in
Node without a `"type": "module"` package boundary.

`[bundle.cost]` values are integer deployment-policy weights, not runtime
measurements. At least one byte weight must be nonzero. Request/depth values are
byte-equivalent penalties; preload and cache values are percentages from 0 to
100. Candidate code is always measured with gzip level 9 and Brotli quality 11.

Package metadata and locked path dependencies are configured at the top level:

```toml
[package]
name = "example"
version = "1.0.0"
abi = 1
entry = "src/lib.lil"

[dependencies]
mathkit = { path = "../mathkit", version = "^1.2", abi = 1 }
```

Run `lilscript src/main.lil --write-lock -o build/app.js` to rewrite
`lilscript.lock`. Normal builds verify the complete transitive graph, semver,
ABI, package-root confinement, and SHA-256 source checksum without mutating the
lockfile. See `docs/modules-and-delivery.md` for the full contract.

## Lint and format policy

`lilscript-lint` performs module-aware semantic checks and inspects optimized
IR for allocation and materialization findings. `minimal` enables only
correctness errors. `recommended` adds effect/performance warnings and size
hints. `strict` promotes effect findings to errors and size findings to
warnings. Configure any rule in `[lint.rules]` with `off`, `hint`, `warn`, or
`error`. Set
`deny_warnings = true` or pass `--deny-warnings` for a warning-free CI gate.
Trusted `pure extern` functions must appear in `pure_extern_allowlist` because
their effects cannot be verified from LilScript source.

Loop-cost analysis reports surviving arrays, aggregates, maps, sets, buffers,
typed-array views, materializing array operations, closures, and unresolved
indirect calls. Because it runs after optimization, values removed by DCE or
scalar replacement do not produce allocation findings.

`lint.providers` is an exact namespace allowlist. Built-in namespaces are
`correctness`, `effects`, `performance`, `size`, and `web`; the web provider
adds `web/eager-host-access` for top-level host work that can run before a
progressive-enhancement boundary. Embedders can call
`lint_path_with_providers` with Rust `LintRuleProvider` implementations. A
provider receives the checked module, optimized IR, source, path, and project
config, and emits stable namespaced diagnostics with optional evidence, help,
and fixes. Duplicate namespaces and undeclared rule IDs are rejected before
rules run. This is an in-process Rust API rather than an unstable dynamic
library ABI.

```sh
lilscript-lint src
lilscript-lint src --format json
lilscript-lint src --format sarif --deny-warnings
lilscript-lint src --fix
```

Use `// lilscript-lint-disable RULE` to suppress a rule from that line onward,
or `// lilscript-lint-disable-next-line RULE` for one following line. The
current machine-applicable fix removes unreachable expression statements;
findings that require intent remain diagnostic-only.

`lilscript-fmt` is a deterministic, comment-preserving formatter and import
organizer. It writes by default, supports `--check` for CI and `--stdout` for a
single file, and is idempotence-tested. Set `format.enabled = false` to disable
CLI/LSP formatting by policy; `--force` explicitly overrides it in the CLI.

```sh
lilscript-fmt src
lilscript-fmt src --check
```
