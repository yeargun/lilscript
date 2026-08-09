# Compiler Configuration

The CLI discovers `lilscript.toml` by walking from the input module toward the
filesystem root. Pass `--config path/to/config.toml` to select one explicitly.
Unknown keys and invalid numeric limits are errors.

```toml
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
profile_guided = true

[javascript]
priority = "size-first"
optimization_level = 15 # 0..15 compiler-effort budget
cost_model = "brotli" # raw | gzip | brotli
pool_numeric_literals = true # alias repeated profitable numeric literals
candidate_search = "production" # off | production | always
candidate_limit = 1536
candidate_byte_budget = 1048576 # aggregate whole-artifact search-work budget
candidate_beam_width = 12
max_candidate_raw_growth_percent = 0 # hard raw-size boundary; maximum 1000
function_layout_exact_limit = 13 # 0 = heuristic only; maximum 18
local_name_reserve = 48 # consistent short identifiers reserved for lexical locals
stable_local_names = true # preserve source-local affinity across generated kernels
# function_spelling = "arrow" # arrow | function; see public-ABI note below
# public_aggregate_abi = "named" # named | positional; positional requires opaque array handles
# optimizations = ["parsed-peephole", "startup-cost-guard"]
compression = [
  "identifier-mangling",
  "entropy-aware-mangling",
  "quote-style-selection",
  "string-pooling",
  "size-aware-inlining",
  "safe-integer-coercion-elision",
  "compact-boolean-literals",
  "standard-grammar-elision",
  "structured-closure-inlining",
  "string-array-packing",
  "scalar-phi-copies",
  "phi-affinity-coalescing",
  "ir-inlining-variants",
  "ir-closure-factory-variants",
  "ir-phase-ordering-variants",
  "loop-spelling-selection",
  "mutation-spelling-selection",
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
# properties = false
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
  no inline-growth cap, disables automatic string pooling, and retains eager
  signed-i32 normalization for numeric hot paths.
- `realistic-performance-first` uses limits of `18`/`45`,
  allows up to `16` estimated additional IR instructions from repeated-call
  inlining, enables profitable string pooling, and removes coercions only when
  range analysis proves the result remains a signed i32.
- `balanced` uses limits of `12`/`30`, permits up to `4` estimated additional
  instructions, and enables profitable string pooling.
- `size-first` is the default. It uses limits of `12`/`30`, permits up to `16`
  temporary IR instructions of inline growth so the following fold/DCE fixed
  point can expose a net byte win, enables profitable string pooling, and
  considers delimiter-packed string literal tables. Packing adds startup work,
  so the performance-oriented profiles leave it disabled.

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

`javascript.compression` is an optional exact allowlist of contested JavaScript
size tactics. If omitted, the selected profile supplies the list. If present,
only listed tactics are enabled; `compression = []` disables all of them:

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
- `property-mangling` renames LilScript-owned boundary-visible properties.
- `export-mangling` permits public ESM export names to be shortened.
- `string-pooling` aliases repeated strings only when the emitter's raw-size
  model predicts a reduction.
- `size-aware-inlining` applies the profile's positive-growth limit to repeated
  straight-line calls.
- `safe-integer-coercion-elision` removes signed-i32 normalization from ordinary
  arithmetic only when inferred ranges prove that the result is already in
  range. Unknown or overflow-capable operations remain normalized.
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
- `string-array-packing` considers immutable literal tables such as
  `["a","b"]` as a delimiter-joined string plus `.split()`. It is a size/startup
  tradeoff and remains a compressor-scored candidate rather than a mandatory
  lowering.
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
late identical-body folding and declaration layout, and levels 14-15 add
proof-driven function-subsumption IR candidates. The effective cap is always
the lower of the level cap and `candidate_limit` when the level-derived feature
set is active. `candidate_beam_width` controls how many distinct leading
emission layouts advance to each subsequent structural decision. Raising it
can recover interactions whose first step is not locally best; lowering it
reduces complete-artifact emissions and compressor work. It must be greater
than zero and is always bounded by the effective candidate limit.

Typical effort settings are:

```toml
# Fast edit/build loop: one configured emission and no compressor search.
[javascript]
optimization_level = 0
candidate_search = "off"
candidate_limit = 1
candidate_byte_budget = 1
candidate_beam_width = 1
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
cost_model = "brotli"
```

The checked-in default sits between these at level 15, `production` search,
an effective 384-candidate cap shared across all IR optimizer variants, a 1 MiB
aggregate candidate byte budget, and a beam width of 12. The byte budget is
divided across optimizer variants and converted to a candidate count from each
variant's configured baseline size. Thus tiny outputs can exhaust the count
cap, while broad outputs automatically run fewer whole-artifact emissions and
quality-11 codec probes. At least the configured output from each retained IR
variant is always measured. Initial representation cross-products are bounded
before full emission and codec probing, so neither cap can be multiplied
silently by module breadth or optimizer variants. Raise
`candidate_byte_budget` for slower maximum-compression releases. These controls change
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
`comma-expression-variants`, `structural-loop-variants`, `do-loop-variants`,
`update-loop-variants`, `switch-lowering-variants`,
`compound-mutation-variants`, `entropy-cross-scope-reuse`,
`entropy-property-assignment`, `function-layout-variants`, `parsed-peephole`,
and `startup-cost-guard`.
The remaining names are `performance-shape-model`,
`profile-guided-optimization`, `call-site-specialization`, and
`capture-signature-cloning`, plus `identical-function-folding`.
An empty list disables all of these features. Duplicate names and levels above
15 are configuration errors. With an exact allowlist, `optimization_level` no
longer lowers the candidate cap; `production` search still bounds it at 384,
while `always` uses `candidate_limit` directly.

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

`max_candidate_raw_growth_percent` is a hard selection boundary applied both
within one emitted-IR search and across optimizer variants. The default `0`
allows codec-aware search to choose only artifacts no larger than its
configured baseline in raw bytes. Projects that deliberately accept raw-byte
growth for gzip/Brotli wins can raise the percentage (up to 1000); the unchanged
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
`realistic-performance-first` rejects candidates beyond
`max_regression_percent` before minimizing transfer; and `performance-first`
ranks the shape score first. The four weights allow a project to tune that
proxy without changing language semantics.

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
closure body and remove its environment slots. Every clone re-enters ordinary
folding and DCE, is bounded by the profile limits, and remains an independently
codec-scored optimizer candidate for JavaScript. The corresponding
`[optimization]` switches are authoritative global gates; a JavaScript effort
level or exact feature allowlist cannot re-enable a pass explicitly set to
`false` there.

`javascript.cost_model` selects the exact objective used by optimizer-IR and
bounded final-emission candidate search. `raw` compares emitted bytes, `gzip` uses level 9, and
`brotli` uses quality 11. Candidate selection is deterministic: ties use the
configured startup score, raw bytes, and then lexical output order. The search only disables already enabled
contested tactics for comparison; it never turns on a tactic omitted from the
exact `compression` allowlist. `candidate_search = "production"` is the
default and is skipped by CLI `--mode development`; `always` remains active in
that mode, while `off` disables compressor-in-the-loop emission. The current
search space compares profitable string pooling, literal-table packing,
proven-safe integer coercion elision, numeric-literal pooling, boolean literals,
structured closures, identifier alphabets, adaptive local-name reservations,
quote styles, and equivalent top-level declaration,
phi-affinity, SSA parallel-copy, conditional/comma, structured/state-machine,
`while`/`for`/`do`, update-clause, switch/conditional-dispatch, and assignment/
prefix/postfix/compound-mutation layouts, bounded by the effective candidate
limit. `candidate_beam_width` sets the cross-dimension search window; the
configured baseline is always retained as a startup-safe fallback. Transfer
scores already measured during search are reused when the parsed peephole
leaves a finalist unchanged, avoiding a second quality-11 compression pass.

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
  and cache reuse. The least-cost eligible shared chunk honors the explicit
  split request; every additional optional chunk must lower deploy cost.

`split` and `preserve-modules` require `--output`. They write the entry module,
sibling chunks, and `<entry-stem>.manifest.json`. Static imports load eagerly.
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
