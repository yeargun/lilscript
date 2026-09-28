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
[objective]
codecs = ["brotli"]           # raw | gzip | brotli: the codec whose bytes are minimized
[objective.brotli]
quality = 11                  # 0..11
window = 22                   # 10..24 (log2 bytes)
mode = "generic"              # generic | text | font
[objective.gzip]
level = 9                     # 1..9
window = 15                   # 9..15 (log2 bytes)

[effort]
level = 13                    # 0..16; 13 is the default

[javascript]
priority = "size-first"      # the only accepted value; see "Retired keys"
# candidate_search = "off"    # only `off` has an effect: --mode development sets it
# ecmascript = "es2022"       # es2015 … es2022 | esnext
# browsers = ["chrome80", "firefox78"] # intersected with ecmascript; the lower floor wins
strip_debug = false           # drop calls of `debug` declarations (print is never stripped)
strip_console_calls = false   # drop host console.* method calls
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

[target.javascript]
format = "esm"                # esm; cjs, iife, umd and bare arrive with plan M3.3b

[delivery]
mode = "single"               # single | split | preserve-modules
# entry_names = "[name].[ext]"   # placeholders: [name] [index] [hash:N] [path] [ext]
# chunk_names = "[index].[ext]"  # a library's default; an application's is [hash:8].[ext]
# module_names = "[path].[ext]"  # preserve-modules module files
preload = "none"              # none | entry | all
host_modules = "external"     # external | auto | embed
request_bytes = 0             # declared cost per file an entry loads beyond its first
depth_bytes = 0               # declared cost per static import level beyond the first

[delivery.entries]            # entry name = source, relative to this file
# index = "src/index.lil"
# animate = "src/entries/animate.lil"

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
- `print` is a program effect: the compiler never strips it, so a program
  with no configuration prints what it prints. Two keys strip host logging,
  both off by default because a library's logging is its behavior, and both
  keep their calls' argument evaluations and throws:
  `javascript.strip_debug` drops direct calls of a declaration marked
  `debug` (`debug void trace(string m) {…}`, `debug extern void
  invariant(bool ok, string m);`, language rule R15), and, until the ports
  declare it so, of an extern named `debugLog` (`migration/debug-class`
  writes the modifier); and
  `javascript.strip_console_calls` drops method calls of the host `console`
  (`console.warn(x)` through an extern `console`), a declared relaxation of
  console output. The retired `strip_console` stripped `print` and `debugLog`;
  its value now moves to `strip_debug`, with a warning.
- `javascript.keep_published_function_names` (default `true`) keeps each
  exported function's source `name` (D2). `keep_function_names` extends that to
  every function whose name some code could read.
- The `assume_*` keys are contract assumptions about foreign values, each off
  by default because a library cannot know its callers:
  `assume_pristine_builtins` (the host's builtins are the originals where a
  `JsValue` operation reaches them: a store that adds a key to a fresh object
  literal, a method called through `.call`, a standard global read as inert;
  typed operations mean the originals whatever it says, language rule R10),
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

`[objective] codecs` names the objective: the compiler minimizes the delivered
file's bytes under that codec. `raw` counts bytes, `gzip` is zlib 1.3.1 and
`brotli` is Google Brotli 1.1.0. `[objective.brotli]` and `[objective.gzip]`
set the codec's parameters (law B2: the judge is the configured codec). They
are fingerprinted and printed in the policy, and every exact judgement and
reported size of the build uses them. The defaults are the canonical settings
(Brotli quality 11, window 22, generic mode; gzip level 9, window 15), which
`lilscript-codec` and the benchmark contract always use. The walk's proxy is
Brotli at min(quality, 5) with the objective's window and mode. One codec per
build for now: several, one winner each, come with the multi-objective build
(plan M3.4). A single codec may be written as a string, `codecs = "gzip"`.

`[effort] level` (0 to 16, default 13) is a work budget with a versioned
schedule (`--print-policy` prints it) and grants no permission. Architecture
§13.4 and §9.6 state the schedule:
- Level 0 runs every rule and forms the level-0 artifact, and no codec runs:
  gzip and Brotli sizes are unmeasured.
- Levels 1 to 12 are the fast tiers. One pass over the walk's list from the
  level-0 artifact, with a prefix of 8 positions (levels 1–4), 24 (5–9) or all
  (10–12), and 2, 4, 6, 8 or 12 exact judgements.
- From level 13, size comes first (amendment AM2). The structural search runs,
  and each objective walks several starts (the search's winner, the level-0
  artifact, the level-0 artifact under each other naming seed) in passes to
  their fixed points, keeping the smallest. Above 13 the structural search
  widens with the level.

Each level passes through every lower level's result (the replay check,
[testing.md](testing.md#the-effort-schedules-monotonicity-m35)).
`candidate_search = "off"` (and `--mode development`) keeps only the level-0
artifact at any level.

Level 13 is the default. It is the first level where the whole search runs
and never grows a build because of a change (AM2). The measurements behind
the choice are in [007](../finer/hypotheses/007-level-13-sweet-spot/README.md).

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

- **Renamed (moved, with a warning).** `javascript.cost_model` is now
  `[objective] codecs`, and `javascript.optimization_level` is now
  `[effort] level` (schema v3, plan M3.1). When the new key is set too, it
  wins, and the old key is removed with a warning.
- **No effect since the counted budget (M3.5).** `javascript.candidate_limit`,
  `candidate_byte_budget`, `candidate_beam_width`, `candidate_proposal_limit`,
  `terminal_codec_probe_limit`, and `candidate_search` other than `"off"`.
  The effort level's schedule is the only budget.
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

## Delivery: `[delivery]`

The contract is [modules-and-delivery](modules-and-delivery.md#delivery): clauses DL1–DL10, the
modes, the files and manifest v3. Every mode checks and optimizes the whole
program, all entries together, before any file boundary is chosen.

- `mode`: `single` writes one file per entry; `split` writes each entry's file
  plus the files the entries that load them share; `preserve-modules` writes a
  file per source module. `split` and `preserve-modules` need module
  execution (`--target js-module`).
- `[delivery.entries]`: the program's entries, name to source path relative to
  this file. Names are letters, digits, `_`, `.` and `-`; they sort the entries,
  so the table's order never matters. The command line adds `--entry NAME=PATH`
  and `INPUT` (named by its file stem).
- `entry_names`, `chunk_names`, `module_names`: file name templates over
  `[name]`, `[index]`, `[hash:N]` (1–64 hex digits of a content hash that covers
  every file a file can load), `[path]` and `[ext]`. A template stays inside the
  output directory and has no empty or `.` path segment. Two files that get one
  name are refused.
- `preload = "entry"` preloads the lazily loaded files an entry can load, and
  `all` every lazily loaded file; the emitted guard is inert outside browsers.
- `host_modules` decides whether the relative JavaScript or TypeScript modules
  that `import extern` declarations name are imported from their specifiers
  (`external`), carried when every one can be delivered (`auto`), or carried
  with a refusal when one cannot (`embed`). A relative specifier is spelled from
  the output directory, which stands for the first entry's source directory.
- `request_bytes` and `depth_bytes` are declared deployment costs (L12, at most
  1 GiB each), added to each entry's row: per file it loads beyond its first,
  and per static import level beyond the first. The objective's codec prices
  every file; there is no other cost model.
- `[target.javascript] format` is `esm`; any other container is refused until
  plan M3.3b.

The old `[bundle]` table translates before the file is read: `mode`, `preload`
and `host_modules` move to `[delivery]` (with a warning to rename them);
`cost.request_overhead_bytes` and `cost.dependency_depth_penalty_bytes` become
`request_bytes` and `depth_bytes`; `min_chunk_bytes`, `max_chunks`,
`shared_min_imports` and the codec weights and discounts of `[bundle.cost]`
have no effect (placement follows reachability, and the objective's codec
judges files). A key set in both tables is refused.

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

`migration/js-builtin` warns at each `JS.*` call that has a syntax spelling
(`JsValue`'s ordinary JavaScript syntax, [language rule R12](language.md)) and
fixes it to that spelling, which means the same operation. A call is fixed
once its operands hold no other `JS.*` call, so `lilscript-lint --fix`, run
until nothing changes, rewrites nested calls from the inside out. A call whose
syntax would mean another operation (`+` of a typed `string`, a call of a typed
function, an array literal where no `JsValue` is expected) is reported without a
fix.

`migration/implicit-default` warns at each class field that has no initializer
and that `init` does not assign on every path: such a field takes its type's
implicit default, which [language rule R3](language.md) removes. The fix writes
that default as the field's initializer (`int count = 0;`, `string[] rows =
[];`, `Color tint = Color.None;`), which is what every construction evaluates
today. A field whose implicit default is not a value of its type (a class,
struct or function field left null until `init`) is reported without a fix.

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
