# Compiler Configuration

Why knobs exist, precedence, and how they change compilation: [knowledge/config](knowledge/config/README.md). The generated key-by-key reference, with defaults, is [knowledge/config/schema.md](knowledge/config/schema.md). This page explains the file.

The [migration configuration contract](migration/plan.md#3-configuration-is-a-product-contract)
describes planned completion separately from this accepted schema. TOML, CLI and
the Rust build API support any nonempty set of independent raw/gzip/Brotli
results in one request. Remaining consumer integration and family-registry
completion are tracked under [D3](migration/plan.md#d3); C1 owns permission enforcement.

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

## Precedence and explanations

The nearest discovered file is the entire project configuration; parent files
are not merged. An explicit `--config` replaces discovery and a missing file
fails. Defaults fill omitted keys. Retired aliases translate before strict
reading; contradictory aliases fail even if an eventual CLI override would
hide the conflict. A singleton objective string and singleton list are equal.

CLI `--delivery`, `--format`, `--proxy-pruning` and development mode apply to
the loaded request before the shared resolver. The deprecated environment
adapters for proxy auditing and service work apply only to CLI requests and
lose to their explicit flags. Service resource ceilings intersect project
limits; no flag or environment value raises a TOML hard cap. The library API
reads no optimization environment variables. Timing/debug verification switches
only collect diagnostics or perform validation; native toolchain environment
selection is a separate target concern.

`--print-policy` reports `configuration_inputs` and per-tactic `resolution`:
origin (default, preset, legacy alias or policy), disabled/unavailable/automatic
state, effective producer prerequisites and permitted risks. Origin wording is
outside the fingerprint; semantically equal alias and canonical requests keep
the same identity. Build reports include the same resolution for each target
and requested objective.

Permission and observed use are separate. Structural reports retain discovered,
unreached, legal, unknown, truncated, conflicting, redundant and equivalent
opportunity counts by family. `inventory_reached = false` means discovery was
not reached, not that no site exists. Terminal trials separately record exact
rejection, proxy pruning, veto, budget exhaustion and selection. Selected
artifact provenance names permitted formation assignments; it does not claim
that every selected pass changed the source or that static risk proves speed.

## Field initialization during migration

`[language] field_initialization` accepts `"legacy"` (the current default) or
`"explicit"`. Select `explicit` to adopt R3: every class field needs a declared
initializer or assignment on every normal constructor path, before any read or
capture of `this`. The checker follows early returns, branches, loops and
`finally`; exception paths are conservative, so an explicit initializer can be
needed when assignment cannot be proved. The diagnostic keeps the field's source
span. Native, JavaScript, build and check APIs enforce the same contract.

Use `lilscript-lint --fix` with `migration/implicit-default` under `legacy` before
opting in. It uses the same checked field facts and writes the existing default
where a type-safe spelling exists. A non-nullable reference with no valid default
needs a source edit. `legacy` preserves existing source behavior during the port
transition; its eventual default change requires qualified port releases (V2).

This controls accepted source, not search effort or runtime risk. Level 0 and
level 13 apply identical checks, and disabling an optimization cannot bypass
one. The bounded flow analysis adds checking work; the setting does not launch
codec trials or promise smaller output. The resolved receipt and cache
fingerprint include it. Normal configuration discovery and explicit `--config`
precedence apply; unknown keys or values fail.

## Absence during migration

`[language] absence = "legacy" | "unified"` selects R2 independently of effort,
target and optimization permissions. `legacy` is the compatibility default until
the qualified port transition. `unified` permits either `null` or `undefined`
inside an optional value and applies a parameter default to either absence.
Narrow optional values before string conversion, spelling tests or collection
operations that distinguish them. The checker and `migration/absence` lint share
the observation rule; lint fixes preserve legacy text where that is possible.

Write `T | null` or `T | undefined` for a boundary with an exact spelling. The
type retains the pin through generic substitution and nested callable types.
An unpinned optional callable result uses `null` at a JavaScript crossing;
an unpinned optional public object field is an omitted key. A defaulted public
parameter accepts omission or `undefined`; development checks reject `null`.
The program's internal default still applies to both spellings.

Mutable arrays and maps retain their identity across aliases. Writes normalize
an optional element or value to its storage pin, including later indexed writes,
`push`, `fill` and `set`. Arrays with different element pins cannot alias through
assignment; a fresh spread copy can change the pin. This puts the conversion at
the write and avoids an eager copy at every public call. Native transport of an
absent argument to a defaulted parameter is diagnosed at the source until N2
implements that calling convention; omitted native arguments already work.

This can remove repeated internal normalization and private optional-field key
maintenance. Public adapters may add code and runtime work where an exact
spelling is required. It is a language choice, not a claim that every artifact
gets smaller. The receipt and policy fingerprint retain the choice. Unknown
keys and values fail configuration loading; no compression level overrides it.

## Character codes during migration

`[language] char_code_at = "legacy" | "number"` selects the `charCodeAt` source
contract on both targets. The migration default `legacy` returns an int32 and
zero outside the string. `number` returns the UTF-16 code unit or NaN, matching
ECMAScript; assignments to `int` then require an explicit conversion. Run the
`migration/char-code` fix on legacy source first to retain its results, or use
`codeUnitAt` where the index is known to be in range. The lint does not rewrite
programs already using the number contract.

The number contract can remove `| 0` from number-valued reads. Explicit integer
conversion may restore it, so this setting does not guarantee smaller output.
Native uses a number/NaN helper instead of the legacy integer helper. Selection
adds no search candidates, and target, checks, effort or tactic permissions
cannot override it. Receipts and cache identities include the setting; unknown
values are errors. Qualified port/default adoption belongs to D3/V2.

## The schema

```toml
[language]
char_code_at = "legacy"       # legacy | number; UTF-16 read result and out-of-range behavior
field_initialization = "legacy" # legacy | explicit; source contract, independent of effort
absence = "legacy"            # legacy | unified; internal absence and boundary contracts

[objective]
codecs = ["brotli"]           # any nonempty subset of raw | gzip | brotli; separate results
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
checks = "production"         # "development" throws where a precondition fails: an index read out of range, a crossing of the wrong type
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
version = 3                  # explicit runtime permissions; omitted version keeps legacy 2

[policy.tactics]              # auto | on | off per tactic; `off` is a veto
# inlining = "off"

[policy.resources]            # hard ceilings for one compilation
# logical_work = 1000000000
# retained_bytes = 268435456
# wall_time_ms = 60000

[policy.search]
codec_schedule = "staged"     # staged | immediate
proxy_pruning = "on"          # on | audit | off; terminal proxy rejection
deferred_naming_starts = "auto" # auto: effort 14+; on: 13+; off: never
deferred_naming_polish = true # combine them with permitted naming/field refinements
render_batch = 8
diversity_interval = 4

[target.javascript]
format = "auto"               # libraries: esm; application scripts: iife; explicit bare for an embedding

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

## Guideline for optimization controls

Start with the objective and effort level, then change individual controls for
a concrete reason. Level 13 remains the default size-focused build; levels 14
and 15 permit increasingly expensive exploration for smaller possible wins.
An effort level is a compiler work policy, separate from gzip's compression
level or Brotli's quality setting. No level guarantees a global minimum.

Every optimization control's documentation must state:

1. What behavior it controls and a situation where that behavior is useful.
2. Accepted values, the default, effort gates, target availability and
   prerequisites; distinguish requested permission from actual application.
3. The expected size, compilation time/memory and runtime/startup/allocation
   tradeoffs. Say when there is no measured evidence or no guaranteed size win.
4. What disabling it removes, how it interacts with related controls, and
   whether an explicit setting overrides an automatic effort gate.
5. Any semantic assumption or public-boundary change, separately from work
   limits. Link representative measurements with their objective, effort and
   compiler version; never present a workload result as a universal promise.

Use `--print-policy` to check the resolved request and an explanation receipt to
check what ran and what won. Compare complete outputs under the selected
objective: a raw-byte saving can increase gzip or Brotli bytes. Preserve useful
expensive options behind explicit settings or higher effort when measurements
show small returns at substantial compilation cost. Raising effort never
overrides a tactic veto; the diagnosed level-16 startup-risk compatibility
exception is described below.

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
  the compiler never sees. Private-field mangling preserves these keys and
  excludes them from new assignments; unproved or public shapes also keep
  their declared keys.

Declared shapes do not need an unsafe property-read assumption. Their `data`
fields carry that checked guarantee; `accessor` fields retain observable reads
and writes. `checks = "development"` validates data descriptors and recursive
shape/struct fields at host crossings, adding helper code and traversal work.
Production emits only the operations required by the public representation.
`policy.tactics.property-mangling = "off"` also preserves private shape keys;
public keys, optional-key omission and value-copy adapters are semantic
contracts at every effort level, including 0. Raising effort does not weaken them.

## Objective and effort

`[objective] codecs` names the objective: the compiler minimizes the delivered
file's bytes under that codec. `raw` counts bytes, `gzip` is zlib 1.3.1 and
`brotli` is Google Brotli 1.1.0. `[objective.brotli]` and `[objective.gzip]`
set the codec's parameters (law B2: the judge is the configured codec). They
are fingerprinted and printed in the policy, and every exact judgement and
reported size of the build uses them. The defaults are the canonical settings
(Brotli quality 11, window 22, generic mode; gzip level 9, window 15), which
`lilscript-codec` and the benchmark contract always use. The walk's proxy is
Brotli at min(quality, 5) with the objective's window and mode. A single codec
may also be written as a string, `codecs = "gzip"`. Lists must be nonempty and
contain no duplicates. List order does not affect policies or search order.

Request several codecs when publishing independently optimized alternatives or
comparing their results, for example `codecs = ["raw", "gzip", "brotli"]`.
The CLI requires `--out-dir dist` and writes each complete tree under
`dist/raw`, `dist/gzip` or `dist/brotli`. The files are JavaScript optimized for
that codec; this setting does not emit compressed `.gz` or `.br` files. Relative
imports and compiler-scored bytes are preserved. The root manifest identifies
every objective, policy, encoder setting and file hash; see
[delivery manifests](modules-and-delivery.md#files-names-and-the-manifest).
Stdout and `-o FILE` are ambiguous for several results and are rejected before
compilation. Single-objective output paths and manifest version 3 are unchanged.
With `--target all`, one C file and executable are also written under
`dist/native`, named after the source entry; they are outside the JavaScript
manifest. Native-only targets do not repeat work for JavaScript objectives.

In the Rust build API, `ServiceOptions.objectives = Some(Objectives::One(codec))`
overrides the file's objective through the same policy resolver, retaining its
encoder settings. `Some(Objectives::Two(first, second))` requests two codecs,
`Some(Objectives::All)` requests all three, and `None` follows the TOML set.
`Objectives::from_codecs` builds a normalized API set. Each result is independent:
each has its own codec policy, spelling, structural frontier, naming walk and
qualified winner. It shares source discovery, checking, semantic defaults and
safe analysis/measurement caches. The report's `javascript_policies` and
`search.objectives` expose each policy and search; `javascript_policy` remains
the configured primary codec if requested, otherwise the first requested codec
in raw/gzip/Brotli order. For several objectives, `--print-policy` reports both
the whole-set fingerprint and each individual policy fingerprint; it retains the
primary policy under `policy` and its hash under `primary_fingerprint`.

A combined request first admits every mandatory baseline, then divides the
remaining optional work among the remaining objectives in raw/gzip/Brotli order.
Unused work remains available to later objectives. Memory and the cooperative
deadline are common hard limits; consumed work never resets. Each search retains
its baseline if optional work cannot proceed. With sufficient resources, results
match separate requests; tight shared limits can stop searches earlier. This
avoids repeating checking, but it still pays for each requested objective search
and retains each qualified result, even when some bytes are identical. Selecting
one codec avoids that additional compilation and memory cost. Selecting several
does not increase effort or grant additional runtime assumptions.

`[effort] level` (0 to 16, default 13) is a work budget with a versioned
schedule (`--print-policy` prints it). Policy version 3 never grants runtime risk through effort. Version 2 retains
the diagnosed level-16 startup grant for compatibility. Architecture
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
  widens with the level. Levels 14–16 additionally revisit proxy-rejected
  naming starts by default; level 13 requires an explicit opt-in for that tail.

The fast-tier walk extends a fixed sequence of moves (the replay checks in
[testing.md](testing.md#the-effort-schedules-monotonicity-m35)). Each build
retains its best admitted artifact. Wider structural search can explore
different paths, and hard limits can stop them at different points; output
that never grows between every pair of effort levels remains Q3 work.
`candidate_search = "off"` (and `--mode development`) keeps only the level-0
artifact at any level.

Level 13 is the default. It enables structural search and repeated walks from
multiple starting artifacts. Each walk keeps its best admitted output; bounded
search does not establish a global minimum. Changes to defaults or legality
can change the starting artifact, so batch size gates remain independent for
raw, gzip and Brotli. Historical measurements behind the effort default are in
[007](../finer/hypotheses/007-level-13-sweet-spot/README.md).

Policy algorithm 4 avoids rerunning rules proved unchanged until another rule
edits their input. It preserves the fixed point while reducing counted work;
the same finite allowance can therefore reach more existing candidates.
Optional timing reports include `js_rule_calls` and `js_rule_ms` for actual
target-rule executions, nested within `js_formation_ms`.

Policy algorithm 8 reuses exact statement-mention facts within target
single-use forwarding. Each move invalidates its changed receiver and removes
its deleted declaration; the region releases the cache before advancing.
Cache construction, lookup and backing storage are accounted. This internal
reuse keeps the existing transformation proofs and needs no configuration
switch. Its compilation-time effect is measured separately from size wins.

Policy algorithm 13 separates reachable-node/strictness analysis from capture
facts, so passes request only the facts they use. Both results share one
traversal, with admitted backing storage and counted work. Result lists reserve
their node bounds once; capture analysis summarizes reference owners per binding.
Temporary facts
release after use, including refusal and unwind; each existing transformation
round still recomputes its analysis. This is internal behavior without a new user control.
Previously uncounted allocations and work now participate in hard limits, so
tight limits may stop a build earlier. Compilation cost requires separate
measurement; fewer unused facts alone do not establish a CPU improvement.

`[policy.search]` fixes the search's cadence: `codec_schedule` (`staged`
groups renders before codec measurement, `immediate` scores each at once),
`render_batch` and `diversity_interval` (every Nth expansion serves an old
pending cursor). Remaining budgets never change these values. `[policy.resources]` sets hard ceilings on logical work,
retained bytes and cooperative wall time. Exhausting one stops optional work
and keeps the best artifact found; a ceiling below what the mandatory artifact
needs fails the build.

`proxy_pruning` controls terminal-walk proxy rejection. `on` preserves the
default: a sufficiently worse proxy score skips exact judging. `audit` also
records the exact score of each rejected move; those diagnostic scores do not
select winners. `off` exactly judges every reached, nonidentical move instead
of rejecting it from its proxy. All three modes retain the effort schedule,
legality checks and hard resource limits; none is exhaustive search. Auditing
and disabling pruning spend additional work and may reach fewer later moves
under a hard limit. Exact audit codec probes appear in the reported count.
A negative audit delta is a potential missed win; artifact admission still
has to pass before that candidate could replace the incumbent.

An exact raw/gzip proxy can also reject a larger *starting point* whose later
refinement would win. Audit scores alone cannot detect that path loss; compare
final results with pruning off. The [calibration protocol](../benchmarks/calibration/README.md)
keeps these measurements separate and freezes training/evaluation inputs before
tuning. One saved byte remains an exact saving; its extra compilation cost and
robustness determine whether the strategy belongs at 13, at 14/15, or behind
an explicit permission. There is no universal byte threshold for that decision.

`deferred_naming_starts` defaults to `"auto"`: enabled at effort 14–16,
disabled at the default effort 13 and below. `"on"` opts in at 13;
`"off"` vetoes it at every level. Older explicit booleans remain aliases
(`true` → `"on"`, `false` → `"off"`); omitting the key follows the new default.
`--print-policy` records both the requested mode and its effective permission.
No mode adds this tail below level 13 or overrides a naming-family veto.

The tail revisits naming starts rejected by the ordinary proxy. The compiler
first completes the existing search and naming/property refinements, then
exactly measures each rejected naming start, refines it using ordinary move
pruning and replaces the completed winner only on an exact improvement. This
can find a smaller result through an initially larger spelling. With
`proxy_pruning = "off"`, no rejected starts remain to revisit.

This is an expensive search option for builds where even small byte savings
justify additional compilation. In the
[first frozen port measurement](../benchmarks/migration-results/2026-09-30-q3-deferred-naming/README.md),
the tail took 1.7–3.5× the compilation CPU and improved one of nine objective
outputs; the generic corpus saved ten gzip bytes. Those results explain the
level-14 default gate; they are measurements of that version and workload,
not a forecast for every build. Level 13 retains the established naming and
private-field refinements. Higher effort adds exploration, not a guaranteed win.

`deferred_naming_polish` (default `true`) also tries the existing local naming
and permitted private-field refinements on each admitted deferred start before
settlement. `false` keeps its ordinary walk only; disabling the entire deferred
tail also disables these combinations. Family vetoes still apply. The extra
work uses the same hard resource limits and appears in the receipts.

### Choosing search controls

These controls spend compilation work; they do not grant runtime-risk
permissions or weaken semantic proofs. A tight hard limit can prevent later
trials, so compare the resulting bytes and resource receipt for your chosen
objective. More candidates are useful only when their result justifies the cost.

| `[policy.search]` key | Default and useful situation | Tradeoff and effect of disabling/changing it |
|---|---|---|
| `deferred_naming_starts` | `"auto"`; use levels 14–16 for expensive attempts at the last few bytes, or `"on"` to opt in at 13 | Can refine an initially larger naming seed into a winner. May multiply compile time with no size change. `"off"` removes only this tail; completed ordinary winners remain protected |
| `deferred_naming_polish` | `true`; when the deferred tail runs, combine its names with local allocation and private fields | Extra formation, memory and scoring can reveal combinations the ordinary walk misses. `false` leaves the deferred ordinary walks. No work when the tail is disabled |
| `proxy_pruning` | `"on"`; avoid costly exact judgments of unpromising moves | A proxy can miss useful starts or moves. `"audit"` spends extra exact probes to diagnose misses without selecting from them; `"off"` explores every reached nonidentical move within the same limits. Neither is exhaustive |
| `codec_schedule` | `"staged"`; group rendered structural candidates for scoring | `"immediate"` scores each as it arrives. Cadence affects which candidates reach exact judgment before a limit; neither schedule is always smaller or faster |
| `render_batch` | `8`; number of structural renders between staged scoring events | Larger batches defer feedback and can retain more artifacts; smaller batches bring feedback sooner. Positive integer, meaningful for staged scoring; it does not increase the hard budget |
| `diversity_interval` | `4`; periodically serve an older pending cursor or artifact | Smaller values spend more opportunities away from the current priority leader; larger values favor that leader longer. Positive integer; useful for experimentation, with no universal size direction |

`policy.resources.logical_work` bounds counted work, `retained_bytes` bounds
accounted retained storage and `wall_time_ms` sets a cooperative deadline.
Omitted TOML ceilings inherit the service ceilings described below. Lower limits
can reduce compilation cost and exploration; higher limits permit more work but
cannot force a better result. A wall deadline depends on machine load and is
unsuitable for reproducible size comparisons. Retained storage accounting is
not a measurement of whole-process peak RSS.

`--proxy-pruning on|audit|off` overrides TOML. The deprecated
`LILSCRIPT_WALK_AUDIT` adapter enables `audit` when present (including an empty
value), overriding TOML only for CLI builds; an explicit CLI flag wins. Remove
the variable to use TOML alone. The library API never reads that variable.
The resolved mode is included in `--print-policy` and its fingerprint.

Exact codec measurements are reused across identical file bytes within one
compilation, including candidates whose artifacts were discarded. A bounded
table keys the byte digest, codec settings and exact/proxy role. Reuse pays the
cold logical work tariff and checks the cold encoder scratch requirement;
it cannot buy extra search or replace artifact permission/admission checks.
Callback-local measurements release with that output; a retained search memo
lasts while at least one artifact is live and releases with the last artifact.
With `LILSCRIPT_TIMING`, `codec_lookup` and `codec_reuse` report this work
separately from physical `canonical_gzip`, `canonical_brotli` and
`proxy_brotli` encodes. These timing buckets report elapsed time, not CPU.

## Permission: tactics

Every optional transformation belongs to a tactic in `[policy.tactics]`, each
`auto` (the default), `on` (permitted, never forced) or `off` (vetoed in direct
and searched use). `--print-policy` generates the tactic reference from the
registry: requested permission, effective state, target availability, producer
stages, prerequisites, analysis requirements and defaults. `auto` follows the
tactic's own default and its effort gate. A missing producer cannot be enabled
by a flag: `recurring-reconstruction`
currently reports unavailable, with a diagnostic when explicitly requested on.
Native supports shared scalar replacement and final-use ownership transfers; call specialization still has no native producer.
Disabling identifier mangling also disables its dependent naming search and
alphabet trials, with the reason in the policy diagnostics.

For private calls, `inlining` permits body movement and codec-judged expression
duplication. Larger bodies can expose simplification but increase output and
compilation work; the retained-call alternative remains available when that
tradeoff is uncertain. Single-use nested closures keep a fresh capture bank
whenever repeated activation is possible; native owners keep their original
scope exit. Static call frequency is an upper bound, not a runtime profile.

`call-specialization` independently permits product-argument transport choices
and specialization of private primitive helpers on uniform literal arguments.
Removing arguments can save bytes; spelling a constant repeatedly can cost
bytes, particularly under a different codec. Each objective judges its own
complete output, and explicit `off` vetoes these alternatives.

`helper-sharing = "auto"` permits exact primitive-body sharing under the
maximum preset. `"on"` additionally permits parameterized sharing: similar
bodies pass their differing literals as extra arguments, adding work on each
call. The matcher requires complete direct callers, unobserved identity and
arity, compatible primitive types, identical captures and safe initialization.
Receiver/member feedback sites and calls are outside its neutral proof. Matching
is bounded and extra candidates consume the configured compilation budget;
`"off"` vetoes both optional forms. Mandatory language-runtime helpers remain
independent of this setting. Effort 13 retains the ordinary alternatives;
14–15 provide more search budget, without changing these semantic permissions.

Call representations can be selected **per helper**. Retaining a large body while
inlining another small one is a legal mixed choice. Uniform constants and shared
bodies use the same site/alternative interface as layouts and table encodings;
all are judged after naming and printing. Raw estimates order trials and do not
exclude an offered alternative from gzip or Brotli selection.

`representation-joints = "auto"` enables extra bounded combinations at effort
14 and above. Use `"on"` to explore them at lower effort, or `"off"` to avoid the
additional formation/codec work. These moves help when several choices pay for
one helper/decoder together, or when specialization makes another inline choice
useful. Gains can be very small or absent. Individual choices, existing family
defaults and canonical rollback remain available with this flag off; the actual
selected families still require their own permissions. A retained result records
joint-search provenance and cannot replay under a veto.

The service's finite work and memory ceilings also enter the resolved resource
policy and its fingerprint. TOML `[policy.resources]` restricts those ceilings;
the smaller value wins for each resource. The CLI retains its default of
40,000,000,000 work units and 256,000,000 retained bytes. `--logical-work UNITS`
and `--retained-bytes BYTES` set explicit service ceilings. Embedded clients
use `ServiceOptions` (default 200,000,000 work units and 256,000,000 bytes) and
`ServiceOptions::resolve_policy` to obtain the single-objective policy (or the
configured primary policy for `All`); a combined build reports every resolved
policy as described above. Whole-build cache keys must include the objective
request, source graph and compiler identity as well as resolved policies.
`ProjectConfig::resolve_policy` describes configuration before service ceilings.

`LILSCRIPT_SEMANTIC_WORK` remains a deprecated CLI adapter when `--logical-work`
is absent. It must be a positive 64-bit integer; invalid values fail instead
of silently selecting the default. An explicit flag overrides that adapter;
the TOML hard limit still applies. The library API does not read this variable.

`--print-policy` includes resolved `diagnostics` alongside the canonical policy;
build reports expose them per target in `policy_diagnostics`, and ordinary CLI
builds print them as warnings. Diagnostic wording is outside the policy
fingerprint. The version-2 level-16 compatibility grant is diagnosed until the relevant
reconstruction permission is explicitly set or the configuration selects version 3.

`property-mangling` permits an exactly judged private-field alternative at
default and higher effort. Construction and typed access share one assignment
through inheritance. Unrelated classes and sibling-only fields reuse short
names; fields that coexist on an instance remain distinct. Private generic
storage is eligible, while the reflected-set closure protects erased payloads,
trusted host views, public results, callbacks and thrown values. Any reflected,
external or published member protects its entire inheritance family.

A retained JavaScript class identity alone does not expose storage keys, but its
assignment-based field initialization can encounter inherited setters. Such
classes qualify only with `javascript.assume_pristine_builtins = true`, and a
`__proto__` field still protects the family because its inherited setter exists
even in a pristine realm. This flag is a host contract, not a size-only switch.
Property trials do not move field initializers or add runtime work. They cost
formation and codec judgments, and the unmangled candidate remains eligible.

These trials require `target-compaction` and are independent of
`identifier-mangling`. `off` vetoes direct formation, cached heads and search;
`on` permits the trial without forcing its selection. Codec measurements choose
between candidates separately for raw, gzip and Brotli.
`mangle.preserve_properties` keeps requested keys and excludes those names from
new assignments; other eligible private fields can still rename.

`statement-spellings` controls optional conditional/exit/loop/logical statement,
compound-assignment and quote choices. Exact null/undefined guards can also
choose optional member access from ES2020, and binding-only conditional writes
can choose logical assignment from ES2021; older editions keep ordinary forms. Whole-family defaults supply seeds;
individual sites can override them, and every candidate is scored as a complete
file for its selected codec. This can improve repetition even when a local form
is longer. Discovery and extra judgments cost compilation work. It adds no
runtime effects, requires `target-compaction`, and `off` vetoes direct, cached
and replayed alternatives. Mandatory JavaScript syntax lowering remains.

`receiver-aliases` (automatic from 13) tries a local binding for four or more
receiver reads in one function. It preserves getter count/order and keeps direct
`this` eligible. It requires hidden frames and excludes derived-constructor
`super` timing, direct eval and nodes shared across receiver owners. It adds a
local binding/read; compression benefit and runtime evidence decide whether
that tradeoff is useful. `on` permits trials earlier; `off` vetoes retained choices.

`declaration-order` (automatic from 14) tries grouping function declarations and
primitive literal bindings by syntax/literal prefixes within legal initialization
groups. Calls, effectful initializers, module boundaries and anchored root rows
stop a group. It can improve gzip locality and alter naming, but more discovery
and judgments may yield only small wins. Source order remains eligible. Explicit
`on` permits the choice at 13; `off` vetoes direct and reused assignments. It adds
no runtime operation and requires `target-compaction`.

`target-compaction` permits the JavaScript target's optional rule schedule.
Its folding, function inlining and object scalarization also require
`constant-folding`, `inlining` and `scalar-replacement`, respectively.
`string-pooling` controls repeated literal bindings; `string-array-packing`
independently controls packed strings decoded with `split`. Initial output
choices respect these vetoes for raw, gzip and Brotli. Explicit output
assignments and reused artifact evidence are checked against the same
permissions. Enabling one of these tactics does not override another tactic's
veto or force the search to choose that representation.

Constant-table decoder representations require `startup-reconstruction`.
Select `[policy] version = 3` for effort-independent runtime permissions:
`auto` keeps both decoders and string-array packing off at every level, `on`
permits the selected family at any level, and `off` vetoes it everywhere.
`string-array-packing = "on"` permits startup and callable sites, so its decode
work can recur. These permissions do not force encoding or promise runtime
parity. Public output, search and cached admission use the same rules.

Existing files with omitted `policy.version` or explicit version 2 preserve
the prior behavior: `auto` grants startup reconstruction and startup-only
string-array packing at effort 16, with compatibility diagnostics. It grants
no recurring packing. To migrate, add `version = 3` and explicitly enable the
families whose startup/recurring costs you accept; leaving them automatic can
increase output while removing decoding work. Version and effective permissions
enter the fingerprint. Legacy aliases remain supported, with conflicts refused.

Compatibility correction: older compilers ran table decoders regardless of
`startup-reconstruction` and treated all string-array packing as neutral.
To retain those alternatives intentionally at level 13, set the relevant
family to `on`. Correct enforcement can grow output while removing decoding
work. Increasing effort still never permits recurring reconstruction unless
that family was explicitly enabled.

Constant folding includes bounded evaluation of small functions at constant
call sites. Evaluation preserves argument effects and declines unsupported
control flow, unknown captures and exhausted limits. Structured loops may fold
when evaluation completes within the existing shared 1,024-step bound, including
nested break, continue and return. Nontermination is never assumed away.
The supported scalar methods include constant `toInt()` conversion and
`codePointLength()`, preserving signed-32-bit wrapping and UTF-16 surrogate
behavior, including through nested constant calls. Primitive numeric `toInt()`
needs no host assumption: it lowers directly to the numeric conversion.
Host-backed methods still require `javascript.assume_pristine_builtins = true`
(native compilation uses its fixed runtime). The rule receipt's `folded_calls`
counts calls replaced with constants; it does not claim a compressed-size win.

The same `constant-folding` permission owns finite primitive propagation,
path-based folds, immutable/reaching-value forwarding and reuse of total
primitive computations. The lattice retains at most four values; a larger set
becomes unknown. Pointwise evaluation considers at most 64 combinations, and
function execution is bounded by eight call levels, 64 operations per body and
1,024 steps, with shared work/storage limits. These bounds trade opportunities
for compile time and never turn an incomplete proof into an optimization.
Forwarding preserves allocation owners and avoids creating a second snapshot
for a mutable local. Common computations reuse storage already needed by
multiple uses; introducing a new temporary is a representation choice rather
than an unconditional semantic rewrite.
`dead-code-elimination` independently controls removing newly unread storage,
unreachable operations, unused parameters and unused results. Inlining keeps
its own permission.

Private literal-default transport retains semantic arguments and explicit
callee guards in the shared program. JavaScript may omit matching trailing
literals and use native default syntax when argument count, reflected length,
activation access and effects allow it. Native execution keeps the semantic
arguments. Disabling `constant-folding` also disables this transport. Receipts
report actual value/set/path folds, omitted arguments, removed default checks,
permitted native defaults, successful evaluator attempts and refusal categories.
Evaluator attempt counters include repeated analysis rounds; they are distinct
from the number of calls ultimately removed. The optimizations remove runtime
work without adding reconstruction or changing the runtime-risk permissions.

`constant-folding` also forwards fixed private fields and known lexical
functions in constant namespaces. `dead-code-elimination` removes unread fields
and overwritten field stores while keeping argument and initializer effects in
order. It also collects a contiguous fresh-object construction prefix into its
allocation. Adding missing ordinary-object properties requires the pristine
builtin contract because assignment can otherwise invoke a prototype setter;
records use their null-prototype contract. Neither permission grants scalar storage when `scalar-replacement` is off.

`scalar-replacement` uses allocation identities, complete uses and initialization
to replace closed array, object and class storage with fields. Immutable aliases
and captured references share the same bank, with a separate bank for each
activation. Identity observations, dynamic keys, unknown writers, host escape
and unsupported capture timing retain the aggregate. The analysis tracks at
most eight origins, 64 propagation rounds and 16,777,216 propagation operations;
a missing or exhausted proof never permits a rewrite. Additional scalar/alias
scans have bounded work, fields are limited to 256 per allocation, and aggregate
analysis declines programs above 524,288 combined values and cells. This spends compilation work to remove allocations and
accesses; introducing field locals can change raw and compressed sizes. For
builds including native output, captured local banks retain packed storage if
splitting would require more than two cell boxes (the original handle plus
aggregate); JavaScript-only builds do not have that native allocation limit.

The existing record and product families continue to offer codec-judged layouts.
Record aliases with proved initialization now share their canonical cell before
family discovery; record absence normalization stays with that family. Array
loads that normalize nullable elements retain their original storage, and
scalar cells retain declared field types even when individual reads are narrowed. Escaping,
reassigned or dynamic records retain storage. Strings, floats, booleans,
references and value structs keep their checked payload rules.

For value structs, complete source uses can prove that a fresh product and its
nested products have no observable snapshots. JavaScript then updates their
fields directly, including inside branches and loops, without rebuilding every
ancestor. A live whole-value copy retains persistent storage. Native formation
can transfer a managed SSA value at its single use within the same region,
then clear the consumed temporary, avoiding a retain/release pair. An outer
value reused by a loop cannot be transferred on each iteration. These paths
are part of `scalar-replacement`; `off` vetoes them in direct output and reused
formation as well as search. The existing `ref` contract is unchanged.

These bounded proofs use the existing effort policy, including default 13;
there is no new expensive search tail or level-14/15 strategy in S2. Rule receipts
separate exposed allocations, folded fields, flattened namespace calls, removed or collected
stores, scalar banks and elided aliases from analysis work. Native delivery
reports actual `ownership_transfers`. These counts describe changed work, not
measured runtime speed or guaranteed compressed-size wins.

`naming-alphabet` permits joint trials that reorder identifier characters by
their frequency in the currently delivered JavaScript. It also requires
`naming-search` and `identifier-mangling`. The sequential alphabet remains the
allocator seed; `off` vetoes alternative alphabets in direct rendering, search
and retained admission. `on` permits a trial, never forces its result. Each
objective compares the complete renamed artifact, and its receipt records
the alphabet actually delivered. Walk schedule version 3 added these moves
after the existing literal and naming-style alternatives.

Policy algorithm 6 and walk schedule version 5 add a local frequency-order
trial at the default effort and above, after the existing objective walks and
naming restarts finish. Under scoped naming, each non-root scope can
give its most-used bindings the shortest available names. This is independent
of root read ordering. The earlier winner stays protected; a winning local
trial can then be refined by the existing moves. Declaration order remains
the seed, and raw, gzip and
Brotli judge the complete alternative separately. `naming-search = "off"` or
`identifier-mangling = "off"` vetoes the trial in rendering, search and retained
admission. The explanation records the delivered `local_read_order`; permitting
the trial does not force it. Scope interference, required public names and host
identifiers constrain both orders.

Policy algorithm 16 / walk schedule 10 add `naming-compaction`, a final
compact-allocation trial. Its `auto` gate is effort 14; explicit `on` enables
it at 13. The initial small and medium qualification cohorts found no byte
savings from the additional work, so default effort 13 keeps it disabled. It uses the verifier's surviving bindings, their printed
first-occurrence order and all legal identifier continuation characters (digits
never start a name). Removed bindings no longer consume short names. Scope
reuse and the independent root/local frequency choices remain available. This
is useful after inlining and dead-code removal leave holes in the binding arena;
compressed benefit is measured, not inferred from shorter identifiers.

The trial follows the completed search and retains its winner. It can add a
render and exact codec judgment, plus refinements if it wins, without adding
runtime work. `naming-compaction`, `naming-search` and `identifier-mangling`
control it in rendering, search and replay; `naming-alphabet` independently controls reordered alphabets.
The receipt records `compact_order`. Equal exact-objective scores prefer fewer
complete delivered raw bytes, then stable identities. No rename runs after
selection. The allocation does not change public/host spellings, reflection,
direct eval constraints or capture interference.

Several older keys set a tactic's permission. An explicit `true` is `on` and an
explicit `false` is `off`; a `[policy.tactics]` value that contradicts one is an
error. Contradictions between explicit legacy aliases also fail, including
`mangle` settings versus the compression allowlist and specialization/sharing
settings versus their allowlists. File ordering does not choose a winner.

### Choosing tactic permissions

All entries below use `auto | on | off` in `[policy.tactics]`. Their initial
permission is `auto`. “Preset” means enabled under the default
`optimization.preset = "maximum"`, disabled under `"none"`; explicit settings
still apply. A permission allows proved transformations or candidates, never
forces their selection. Fast tiers visit fewer search choices; level 0 still
runs permitted ordinary simplifications. Runtime effects need workload evidence
even when source behavior is preserved.

The [generated tactic reference](knowledge/config/schema.md#policytactics--closed)
is the authoritative per-family table: useful situation, automatic behavior,
availability, prerequisites, supported risks and size/compile/runtime tradeoffs.
It comes from the same registry as resolution and admission, so adding a family
must supply its documentation and invalidation owners in the same change.

The existing assumption and preservation controls in the contract section
serve a different purpose: `assume_*` changes what foreign behavior must be
supported, and `keep_*`/`preserve_properties` keeps required observations stable.
Choose these from the application's boundary, not from its compilation budget.
The alias table below maps older flags to the same behavior and tradeoffs.

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
off. `helper-sharing` controls optional private implementation sharing; an
explicit allowlist entry also permits its recurring parameterized form.

## Retired keys

The full table, generated from the source, is in
[schema.md](knowledge/config/schema.md#retired-keys). In summary:

- **Renamed (moved, with a warning).** `javascript.cost_model` is now
  `[objective] codecs`, and `javascript.optimization_level` is now
  `[effort] level` (schema v3, plan M3.1). When both names are set, their values must agree;
  contradictory values fail. Equal aliases are removed with a warning.
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
- `[target.javascript] format` defaults to `auto`: `esm` for a library
  (`--target js-module`) and a private `iife` for an application script
  (`--target js`, or the JavaScript part of `--target all`). The wrapper is
  included in every exact size judgment and keeps the script's original
  strictness and lexical `this`. Repeated application loads own separate roots.
- Explicit `bare` delivers unwrapped application code for an embedding that
  owns its private root scope. It retains the same private-root contract; it
  does not publish globals or make their names stable. `esm` requires module
  execution; `iife` and `bare` currently require a closed application without
  static foreign imports. Embedded host modules remain supported.
- Library-global IIFE/UMD and CommonJS exports are still D1 work. Unsupported
  containers are refused, never emitted as mislabeled ESM.

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
