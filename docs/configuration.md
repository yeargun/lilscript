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
scalar_replacement = true
dead_store_elimination = true
dead_code_elimination = true
call_site_specialization = true
capture_signature_cloning = true
profile_guided = true

[javascript]
priority = "size-first"
optimization_level = 15 # 0..15 compiler-effort budget
cost_model = "brotli" # raw | gzip | brotli
candidate_search = "production" # off | production | always
candidate_limit = 1536
# optimizations = ["parsed-peephole", "startup-cost-guard"]
compression = [
  "identifier-mangling",
  "entropy-aware-mangling",
  "quote-style-selection",
  "string-pooling",
  "size-aware-inlining",
  "safe-integer-coercion-elision",
  "compact-boolean-literals",
  "structured-closure-inlining",
  "string-array-packing",
  "scalar-phi-copies",
  "phi-affinity-coalescing",
  "ir-inlining-variants",
  "ir-closure-factory-variants",
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

`javascript.compression` is an optional exact allowlist of contested JavaScript
size tactics. If omitted, the selected profile supplies the list. If present,
only listed tactics are enabled; `compression = []` disables all of them:

- `identifier-mangling` assigns short names by whole-program use frequency.
- `entropy-aware-mangling` compares the canonical identifier alphabet with an
  alphabet ranked by emitted-character frequency, then lets the configured
  exact compressor choose the result.
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
  still compress worse.
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
parsed peepholes plus structural IR/loop/switch alternatives, and levels 13-15
use the largest configured beam. The effective cap is always the lower of the
level cap and `candidate_limit` when the level-derived feature set is active.

`javascript.optimizations` replaces the level-derived feature set with an exact
allowlist. This is separate from the older `compression` policy: `compression`
controls whether a representation is permitted, while `optimizations` controls
which alternative searches and post-emission analyses are run. Available names
are `ir-inlining-variants`, `ir-closure-factory-variants`,
`ir-specialization-variants`, `structural-control-flow-variants`,
`ssa-destruction-variants`, `conditional-expression-variants`,
`comma-expression-variants`, `structural-loop-variants`, `do-loop-variants`,
`update-loop-variants`, `switch-lowering-variants`,
`compound-mutation-variants`, `entropy-cross-scope-reuse`,
`entropy-property-assignment`, `parsed-peephole`, and `startup-cost-guard`.
The remaining names are `performance-shape-model`,
`profile-guided-optimization`, `call-site-specialization`, and
`capture-signature-cloning`.
An empty list disables all of these features. Duplicate names and levels above
15 are configuration errors. With an exact allowlist, `candidate_limit` is the
direct cap because `optimization_level` no longer selects either features or
effort.

The parsed peephole validates the complete generated artifact and Pratt-parses
eligible expressions before rewriting. It currently contracts AST-proven
simple-local `x=x op y` statements to compound assignments; it does not use
text substitutions. The startup guard compares deterministic syntax-derived
parse, engine-compile, and memory estimates against the configured baseline.
The three overhead limits are hard rejection thresholds, while the three
weights break equal-transfer-size ties. `--explain human` and `--explain json`
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
proven-safe integer coercion elision, boolean literals, structured closures,
identifier alphabets, quote styles, and equivalent top-level declaration,
phi-affinity, SSA parallel-copy, conditional/comma, structured/state-machine,
`while`/`for`/`do`, update-clause, switch/conditional-dispatch, and assignment/
prefix/postfix/compound-mutation layouts, bounded by the effective candidate
limit. Structural beams retain finalists from each prior family so one early
layout cannot hide a better cross-dimension combination. The configured
baseline is always retained as a startup-safe fallback.

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

`mangle.properties` renames LilScript-owned fields that cross an untyped
JavaScript boundary. It is off by default because external JavaScript must
otherwise use the renamed ABI. Members declared by `extern class` are host ABI
names and are never renamed, regardless of this setting. Internal struct and
class fields already lower to scalar values or numeric slots. `mangle.exports`
removes stable public ESM names and is intended for LilScript-only applications
whose static imports are linked before codegen.

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
