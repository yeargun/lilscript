# Compiler Configuration

Why knobs exist, precedence, and how they change compilation: [knowledge/config](knowledge/config/README.md). The generated key-by-key reference, with defaults, is [knowledge/config/schema.md](knowledge/config/schema.md). This page explains the file.

The CLI discovers `lilscript.toml` by walking from the input module toward the
filesystem root. Pass `--config path/to/config.toml` to select one explicitly.
`lilscript <input> --print-policy` prints the exact policy a build uses: the
contract, objective, effort, every tactic's permission, resources, and the
retired-key warnings, with a fingerprint.

A configuration is read in two steps:

1. **Retired keys.** Keys that belong to the deleted old compiler, or that this
   compiler reads nowhere, are handled first, by one table
   (`RETIRED_KEYS` in `src/config.rs`; the full list is in
   [schema.md](knowledge/config/schema.md#retired-keys)). A key with *no
   effect* is removed and the CLI prints
   `warning: <file>: `<key>` has no effect in this compiler: <reason>; remove it`.
   `--print-policy` reports the same lines under `"warnings"`. A key whose
   meaning the compiler cannot honour is *refused*: the build stops with the
   reason.
2. **Strict reading.** Everything else must be a known key with a valid value.
   A misspelled key or value is an error. Nothing is accepted silently.

## The schema

```toml
[javascript]
priority = "size-first"      # the only accepted value; see "Retired keys"
cost_model = "brotli"         # raw | gzip | brotli: the codec whose bytes are minimized
optimization_level = 13       # effort, 0..16; 13 is the default
candidate_search = "production" # off | production | always; --mode development sets off
candidate_limit = 1536        # retained whole-artifact candidates
candidate_byte_budget = 1048576 # retained candidate bytes
candidate_beam_width = 12
# candidate_proposal_limit = 384   # optional structural proposals; 0 disables
# terminal_codec_probe_limit = 384 # optional terminal codec probes; 0 disables
# ecmascript = "es2022"       # es2015 … es2022 | esnext
# browsers = ["chrome80", "firefox78"] # intersected with ecmascript; the lower floor wins
strip_console = true          # drop print()/debugLog; tests and oracles set false
assume_pristine_builtins = false
assume_pure_property_reads = false
assume_unconstructed_callbacks = false
keep_function_names = false
keep_published_function_names = true
operand_order_fusion = true   # false turns the target-compaction tactic off
# compression = ["identifier-mangling", "string-pooling"] # exact allowlist, see below
# optimizations = ["call-site-specialization"]            # exact allowlist, see below

[optimization]
preset = "maximum"            # maximum | none: the default of the preset-following tactics
# constant_folding = true     # each of these sets one tactic's permission
# inlining = true
# scalar_replacement = true
# dead_code_elimination = true
# call_site_specialization = true
# parameterized_function_merging = true

[mangle]
# identifiers = true
# properties = true
# pool_strings = true
# preserve_properties = ["onChange"] # contract: names the port's callers read

[policy]
version = 2

[policy.tactics]              # auto | on | off per tactic; `off` is a veto
# inlining = "off"

[policy.resources]            # hard ceilings for one compilation
# logical_work = 1000000000
# retained_bytes = 268435456
# wall_time_ms = 60000

[policy.search]
codec_schedule = "staged"     # staged | immediate
render_batch = 8
diversity_interval = 4

[bundle]
mode = "single"               # single | split | preserve-modules
min_chunk_bytes = 16384
max_chunks = 32
shared_min_imports = 2
preload = "none"              # none | entry | all
host_modules = "external"     # external | auto | embed

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
preset = "recommended"        # minimal | recommended | strict
deny_warnings = false
providers = ["correctness", "effects", "performance", "size", "web"]
exclude = ["**/generated/**"]
pure_extern_allowlist = ["auditedHostFunction"]

[lint.rules]
"performance/allocation-in-loop" = "warn" # off | hint | warn | error

[format]
enabled = true
line_width = 100
newline = "lf"                # lf | crlf
organize_imports = true
```

Per-library configuration is contract, objective, effort and permission
(architecture law L12). The sections below follow that split.

## Contract: what the output must preserve

- `--target js` builds a closed script; `--target js-module` builds a library
  whose exports are the API. The target decides the world and the execution
  mode; there is no key for it.
- `javascript.ecmascript` is the syntax floor (`es2015` … `es2022`, or
  `esnext`; default `es2022`). `javascript.browsers` tokens (`chrome80`,
  `firefox78`, `safari14`, `edge80`) intersect with it, and the most
  conservative floor wins. Unknown tokens are errors. There is no ES5 mode and
  no polyfill; a construct with no spelling at the floor fails the build.
- `javascript.strip_console` (default `true`) drops `print()` and `debugLog`
  from JavaScript; argument side effects stay, and `console.warn` is never
  stripped. Language tests and the repository's `lilscript.toml` set `false`,
  because `print` is their observation channel.
- `javascript.keep_published_function_names` (default `true`) keeps each
  exported function's source `name` (D2). `keep_function_names` extends that to
  every function whose name some code could read.
- The `assume_*` keys are contract assumptions about foreign values, each off
  by default because a library cannot know its callers:
  `assume_pristine_builtins` (ambient constructors are the originals),
  `assume_pure_property_reads` (a dynamic member read runs no getter, Terser's
  `pure_getters`), `assume_unconstructed_callbacks` (callers never construct a
  lambda the program hands them, Terser's `unsafe_arrows`). A port that sets one
  records why.
- `compression` entry `length-to-number-elision` (on under `size-first` when
  the list is omitted) is the assumption that a host value's `length` is an
  int32 Number.
- `mangle.preserve_properties` names properties the port's callers read in code
  the compiler never sees. Nothing renames properties yet, so every property is
  preserved; typed property renaming (plan M9.6) reads this list.

## Objective and effort

`javascript.cost_model` is the objective: the compiler minimizes the delivered
file's bytes under that codec. `raw` counts bytes, `gzip` is zlib 1.3.1 level
9, and `brotli` is Google Brotli 1.1.0 at quality 11, `lgwin = 22`, the same
encoders `lilscript-codec` measures with.

`javascript.optimization_level` (0 to 16, default 13) is the effort: a
versioned schedule of search breadth and tactic gates, printed in the policy.
It never weakens checking or a correctness normalization. The effective
retained-candidate count, candidate bytes and beam width are each the lower of
the level's tier and the configured ceiling (`candidate_limit`,
`candidate_byte_budget`, `candidate_beam_width`). `candidate_proposal_limit`
and `terminal_codec_probe_limit` default from the level; an explicit value may
exceed the level's default but not the `candidate_search` tier, and level 0
turns both off whatever they say. `candidate_search = "off"` (and
`--mode development`) keeps only the mandatory artifact.

Level 13 is the default because the measured curve is a plateau around it:
on the jQuery port, level 15 cost 20 times the CPU of level 13 for 1.4% of the
Brotli bytes, and levels 12 to 14 were within 0.15% of each other
([007](../finer/hypotheses/007-level-13-sweet-spot/README.md)). Those numbers were
measured on the old compiler; the default stands until the effort schedule is
re-measured on this one.

`[policy.search]` fixes the search's cadence: `codec_schedule` (`staged`
groups renders before codec measurement, `immediate` scores each at once),
`render_batch` and `diversity_interval` (every Nth expansion serves an old
pending cursor). Remaining budgets never change these values. `[policy.resources]` sets hard ceilings on logical work,
retained bytes and cooperative wall time. Exhausting one stops optional work
and keeps the best artifact found; a ceiling below what the mandatory artifact
needs fails the build.

## Permission: tactics

Every optional transformation belongs to a tactic in `[policy.tactics]`, each
`auto` (the default), `on` (permitted, never forced) or `off` (vetoed in direct
and searched use). `--print-policy` lists them with their resolved state. `auto`
follows the tactic's own default and its effort gate.

Several older keys set a tactic's permission. An explicit `true` is `on` and an
explicit `false` is `off`; a `[policy.tactics]` value that contradicts one is an
error.

| Key | Tactic |
|---|---|
| `optimization.dead_code_elimination` | `dead-code-elimination` |
| `optimization.constant_folding` | `constant-folding` |
| `optimization.inlining` | `inlining` |
| `optimization.scalar_replacement` | `scalar-replacement` |
| `optimization.call_site_specialization`, or `javascript.optimizations` naming `call-site-specialization` | `call-specialization` |
| `optimization.parameterized_function_merging`, or the `compression` entry `parameterized-function-merging` | `helper-sharing` |
| `javascript.operand_order_fusion = false` | `target-compaction` (off) |
| `mangle.identifiers`, or the `compression` entry `identifier-mangling` | `identifier-mangling` |
| `mangle.properties`, or the `compression` entry `property-mangling` | `property-mangling` |
| `mangle.pool_strings`, or the `compression` entry `string-pooling` | `string-pooling` |
| the `compression` entry `string-array-packing` | `string-array-packing` |
| `javascript.optimizations` naming `entropy-cross-scope-reuse` | `naming-search` |

`optimization.preset = "none"` turns off the default of the tactics that follow
the preset (dead-code elimination, constant folding, inlining, scalar
replacement, call specialization, helper sharing); explicit settings still
apply. `javascript.compression` and `javascript.optimizations` are exact
allowlists: when present, a listed entry is on and an entry the list omits is
off. `helper-sharing` and `property-mangling` have no producer in this compiler
yet, so their permission changes nothing today.

## Retired keys

The full table, generated from the source, is in
[schema.md](knowledge/config/schema.md#retired-keys). In summary:

- **No effect (warned and removed).** Every key that steered the old compiler:
  its optimizer passes (`optimization.algebraic_simplification`,
  `finite_value_propagation`, `identical_function_folding` and the rest),
  emitter spellings (`javascript.function_spelling`, `pool_numeric_literals`,
  `truthy_nullable_checks`, `struct_method_shorthand` …), naming
  (`local_name_reserve`, `stable_local_names`, `idiom_directed_naming` …),
  search bounds (`max_candidate_raw_growth_percent`,
  `function_layout_exact_limit`, `terminal_cleanup_finalists`), inliner bounds,
  `[javascript.startup]`, `[javascript.performance]`, `[profile]`, `[native]`,
  `[compiler.resources]`, `policy.search.interaction_interval` (the search has
  no pairwise interaction phase), `mangle.exports`, `mangle.extern_fields` and
  `mangle.internal_properties`; the `migration/target-tree` line's
  `name_ordering`, `terminal_cleanup_chain` and `wide_single_use_collapse`;
  `javascript.function_scope` (the module wrapper returns as the `format`
  contract axis, plan M3.1); and `public_aggregate_abi = "named"` and
  `optimization.for_of_specialize_family = 0`, which only restate what the
  compiler always does. Retired entries of the `javascript.compression` and
  `javascript.optimizations` lists are removed the same way, with one warning
  per list; the remaining entries keep their exact-allowlist meaning.
- **Refused.**

  | Key | Message |
  |---|---|
  | `[compiler] backend` | there is one compiler; remove [compiler] backend |
  | `javascript.priority` other than `"size-first"` | size is the objective; runtime priorities need runtime estimators that do not exist yet |
  | a `[policy.constraints]` table | size is the objective; runtime constraints need runtime estimators that do not exist yet |
  | `javascript.public_aggregate_abi = "positional"` | public aggregates are plain objects with named fields (D2); the positional shape is not produced |
  | nonzero `optimization.for_of_specialize_family` | the for-of family specialization was an old-compiler source rewrite and was removed |

## Delivery: `[bundle]`

Every mode first checks and optimizes the complete static module graph, so
whole-program work happens before any chunk boundary is chosen.

- `single` emits one artifact.
- `split` considers modules imported by at least `shared_min_imports` distinct
  modules, rejects optional chunks smaller than `min_chunk_bytes`, and keeps up
  to `max_chunks` chunks while each lowers the bundle's deploy cost. The cost
  combines the `[bundle.cost]` weights of raw, gzip and Brotli bytes, request
  overhead, dependency depth, preload and cache reuse. At least one byte weight
  must be nonzero; the percentages are 0 to 100.
- `preserve-modules` keeps one chunk per source module.

`split` and `preserve-modules` need `--output`; they write the entry, sibling
chunks and `<entry-stem>.manifest.json`. `preload = "entry"` preloads the
entry's direct lazy chunks and `all` every lazy root. `host_modules` decides
whether the relative JavaScript or TypeScript modules that `import extern`
declarations name are imported from their specifiers (`external`), carried
when every one can be delivered (`auto`), or carried with a refusal when one
cannot (`embed`). `preserve-modules` chunks and lazy `import()` chunks are
broken on this compiler today (plan M3.3).

## Packages

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
ABI, package-root confinement and SHA-256 source checksums without changing the
lockfile. See [modules-and-delivery](modules-and-delivery.md).

## Lint and format policy

`lilscript-lint` runs the same frontend a build runs, then checks each module's
syntax, the module-graph checker's results and the program they elaborate to.
`minimal` enables only correctness errors, `recommended` adds effect and
performance warnings and size hints, and `strict` promotes effect findings to
errors and size findings to warnings. Configure any rule in `[lint.rules]` with
`off`, `hint`, `warn` or `error`. Set `deny_warnings = true` or pass
`--deny-warnings` for a warning-free CI gate. Trusted `pure extern` functions
must appear in `pure_extern_allowlist`, because their effects cannot be
verified from LilScript source.

`lint.providers` is an exact namespace allowlist. The built-in namespaces are
`correctness`, `effects`, `performance`, `size` and `web`; the web provider adds
`web/eager-host-access` for top-level host work that can run before a
progressive-enhancement boundary. Embedders can call
`lint_path_with_providers` with Rust `LintRuleProvider` implementations; a
provider receives the checked program and the project configuration.

```sh
lilscript-lint src
lilscript-lint src --format json
lilscript-lint src --format sarif --deny-warnings
lilscript-lint src --fix
```

Use `// lilscript-lint-disable RULE` to suppress a rule from that line onward,
or `// lilscript-lint-disable-next-line RULE` for the following line.

`lilscript-fmt` is a deterministic, comment-preserving formatter and import
organizer. It writes by default and supports `--check` for CI and `--stdout`
for a single file. Set `format.enabled = false` to disable CLI and LSP
formatting; `--force` overrides it in the CLI.

```sh
lilscript-fmt src
lilscript-fmt src --check
```
