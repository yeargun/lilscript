# Testing the compiler: the case runner, the port runner and the ratchet

Three versioned runners check a compiler binary. Each pins the binary by SHA-256, compares what fails against a ledger whose entries name an owner task, and exits 1 on anything the ledger does not cover. Two ledger tests guard the source tree itself.

| Runner or test | Plan task | Replaces |
|---|---|---|
| `scripts/cases.mjs` | M2.2 | `finer/tools/semantic-census.mjs` (development mode only) and the knob configs in `tests/config/` |
| `scripts/ports.mjs` | M2.6 | `finer/tools/semantic-port-tests.mjs`, `finer/tools/portgate.mjs` and the unversioned `~/lilscript-work/tools/*.sh` |
| `scripts/ratchet.mjs` | M2.13 | the always-failing hard gates of `comparison/cases/run.mjs` and `comparison/algorithms/run.mjs` as per-change evidence |
| `scripts/monotone.mjs` | M3.5 | — |
| `src/no_library_knowledge_tests.rs` (NO3) and `tests/idiom-debt.json` (NO4) | M2.13 | — |
| the admission parse, inside the compiler (`src/admission_parse.rs`) | M2.5 | the old route's syntax-only parse of terminal leaves |

Run both with Node 24 (`~/.nvm/versions/node/v24.11.1/bin/node` on the build host). There is one compiler, so neither selects a route. The generated differential batch, a third check, is described in [differential-testing.md](differential-testing.md).

## The case runner

```sh
node scripts/cases.mjs --compiler target/release/lilscript
node scripts/cases.mjs --compiler <bin> --lanes production --json out.json --compare previous.json
node scripts/cases.mjs --compiler <bin> --lanes 'formation-only/*/c' --filter regressions/irjs- --cc ~/toolchains/clang-18/clang
```

| Flag | Meaning | Default |
|---|---|---|
| `--compiler` | the binary under test (copied into the work directory and run from there) | required |
| `--lanes` | lanes to run: comma-separated items, unioned. `mode/codec/target` matches component-wise and allows `*`; a single word such as `production` or `c` matches any component | `all` |
| `--filter` | case ids to run: comma-separated substrings, or globs with `*` | every case |
| `--json` | where to write the report | `<work>/report.json` |
| `--compare` | an earlier report: prints byte changes per case and lane, and state changes. This is evidence, never a failure | — |
| `--jobs` | parallel (case, lane) tasks | CPU count − 2 |
| `--work` | scratch directory | `target/verify/cases` |
| `--cc` | C compiler for the C lane, run with `-std=c11 -O2 -fno-fast-math -ffp-contract=off … -lm` (a `cc default flags` case: `-O2 … -lm`) | `/usr/bin/cc` (GCC 13 here) |
| `--codec` | `lilscript-codec` for raw/gzip/Brotli sizes of every JavaScript artifact; `none` to skip | the codec beside the compiler, else `target/release/lilscript-codec` |
| `--ledger` | the expected-failure ledger; `none` for no ledger | `tests/cases/expected-failures.json` |
| `--timeout` | seconds allowed per program run | 30 |
| `--verbose` | also list ledgered failures and ledger entries that could be narrowed | off |

### The corpus

A case is a `.lil` entry with an expected-stdout `.out` file beside it. The runner finds cases in `tests/cases` (recursively) and `tests/modules`. A directory named after a case holds that case's other modules and is not searched for entries. Case ids are paths relative to `tests/`, for example `cases/07_deep_functions`, `cases/regressions/irjs-…` and `modules/main`.

The `.out` file is the oracle. It is never written from the output of the compiler under test.

Optional files next to a case, following the conventions of `tests/cases/regressions/*-MANIFEST.md`:

| File | Effect |
|---|---|
| `X.host.js` | A prelude that defines the program's externs on `globalThis`. The prelude and the compiled program are written into one file and run in one realm. Host scripts that print a trace do so at process exit |
| `X.toml` | Configuration keys merged into the lane's configuration. A key joins the lane's table of the same name, so there is one `[javascript]` table, never a second. A case key overrides the lane's value; a case may not set the retired `strip_console` |
| `X.module-probe.mjs` | Module lane only. The case is built with `--target js-module`; the runner imports the output as `m` and awaits the probe's default export, called with `m` |
| `// harness: "use strict"` in the first lines of `X.lil` | On the script lane, `"use strict";` is prepended to the run file |
| `// harness: cc default flags` in the first lines of `X.lil` | On the C lane, the program is built with `-O2` and the C compiler's own language and floating-point defaults instead of `--cc`'s qualified flags, so the emitted C must keep binary64 semantics by itself (plan M11.2) |
| `X/` | The case's other modules, which the entry imports as `./X/…` |

`print` is the observation channel: it is a program effect, which the compiler never strips (the retired `strip_console` key once did), so no lane sets a logging key.

### Lanes

There are 18 lanes: `{formation-only, production} × {brotli, gzip, raw} × {script, module, c}`.

- **production** is the default policy with `cost_model` set to the lane's codec.
- **formation-only** is production plus the policy that turns optional work off:
  - `candidate_search = "off"`;
  - every tactic the compiler's `--print-policy` lists, set to `"off"` in `[policy.tactics]`.

  The tactic list is read from the compiler, so the lane follows the tactic registry. The report records each lane's policy fingerprint, the tactics it leaves enabled and its optional-alternative budget. A formation-only lane that still enables optional work is flagged.
- **script** is `--target js`. It runs as CommonJS (`.cjs`), as the harvest's verifiers ran it.
- **module** is `--target js-module`, run as an ES module (`.mjs`).
- **c** is `--target c`, compiled with `--cc` and executed.

A (case, lane) passes when the program exits 0 and its stdout equals the `.out` file byte for byte. If two lanes emit the same bytes, the program runs once and both lanes report the result.

### States

| State | Meaning | Failure |
|---|---|---|
| `pass` | Ran, exited 0 and printed exactly the `.out` file | no |
| `masked` | The case uses a feature the target does not have (below) | no, and never counted as passing |
| `refused` | The compiler exited with a diagnostic | yes |
| `compiler-crash` | The compiler panicked (exit 101), was killed or timed out | yes |
| `cc-rejected` | The C compiler rejected the emitted C | yes |
| `crashed` | The program exited non-zero, was killed or timed out | yes |
| `wrong-output` | The program exited 0 but printed something else: a miscompile | yes |

For every failure the report gives the first differing line, or the diagnostic's first lines.

### Per-target exclusions

JavaScript format eligibility uses the lexical `FEATURES` table in
`scripts/cases.mjs`: a foreign ESM import needs module syntax, and a module
probe needs an importable artifact. Detection ignores comments and string text.

Native eligibility uses [individual source-bound exclusions](../tests/native/corpus-exclusions.json),
not a lexical `JsValue`, `export`, `async` or `generator` mask. Each row names
its concrete boundary and the digest of the source, imported modules, oracle,
configuration and host/probe files. New or changed cases run native unless a
new explicit classification is recorded. An exclusion is never a runtime pass.

The N2 census accounts for all 403 cases: 161 have native execution oracles;
147 first require a configured C provider; 84 use a declared JavaScript host
facility; seven observe JavaScript prototypes/modules through host scripts;
and four still request an obsolete positional JavaScript ABI (V1).
Missing-provider classification describes the first blocking test boundary,
not a proof about later recipes. Native C library interfaces and providers have
separate executable qualification. The historical N1 inventory remains archived.

The [native matrix](native-support.md) distinguishes source semantics, native
representations and JavaScript host facilities. Target checking and execution
are independent evidence: an admitted source still has to compile and match
its oracle before the runner counts a pass.

Ordinary C lanes link through `lilscript --target native --link-c` and retain its
native toolchain receipt. Their default native objective is `balanced` (`-O2`);
a case can select a different native objective and the runner pins its `--cc`
selection in TOML. Cases explicitly marked `harness: cc default flags` remain
independent numeric source-guard oracles and use the C compiler directly.

### Artifacts and comparison

For every compiled (case, lane), the report records the artifact's byte size and SHA-256. Every JavaScript artifact is also measured with the canonical codec (`raw`, `gzip9`, `brotli11`). The summary gives each lane's total in its own objective.

`--compare previous.json` prints, per lane, how many artifacts changed and the byte and objective totals before and after. It also lists state changes and the largest per-case byte changes; the JSON has all of them. Two runs of the same binary must compare identical; two runs of the pre-M1 binary did, on every compiled artifact of every lane.

`--level N` compiles the production lanes at effort level N, over any level a case's configuration sets.

### The effort schedule's monotonicity (M3.5)

`scripts/monotone.mjs --compiler <lilscript>` runs the case runner once per level of a list (by default `0,1,5,10,11,12,13,14,15,16`: one level per tier of the effort schedule, and every level from the default up) in the production JavaScript lanes (by default `production/*/module`), and compares each (case, lane) between consecutive levels under the lane's codec. Levels 0–12 extend the same one-pass walk. From 13, protected effort checkpoints retain the preceding policy's completed winner across changed frontiers and automatic tactic gates, with per-entry dominance and exact `(objective, raw)` tie breaks. A growth or newly failing lane is reported as a violation; inspect the hard-resource and `effort_refusal` receipts when a complete checkpoint could not be admitted. It prints each level's lane totals.

It also runs the replay check of architecture §9.6. A production build's walk from the level-0 artifact records, in its receipt's `stops`, the size and SHA-256 of the incumbent wherever a lower one-pass level (0–12) would have stopped. The case runner keeps them from `--explain json`. Protected tiers carry these stops from the actual level-12 policy and add complete higher-effort checkpoint hashes. Every recorded stop of a listed level must be the artifact that level's own build delivered, byte for byte; a mismatch is a `REPLAY` violation.

**The audit lane.** `LILSCRIPT_WALK_AUDIT=1` is a diagnostic that changes no output. It makes the walk also measure each pruned move exactly, and records the exact delta in the move's `audit` field. A pruned move with a negative exact delta is a miss of the proxy (M3.5's no-prune lane). `~/lilscript-work/tools/walk-calibrate.py` counts the misses.

## The port runner

```sh
node scripts/ports.mjs --compiler target/release/lilscript --ports markedlil,zodlil
node scripts/ports.mjs --compiler <bin> --ports all --objective raw --json raw.json
```

| Flag | Meaning | Default |
|---|---|---|
| `--compiler` | the binary under test, copied and pinned by SHA-256. The run fails if the copy changes | required |
| `--ports` | port names, or `all`: the libraries in `benchmarks/libraries/maintained-workloads.json` that exist under `--ports-root` | required |
| `--objective` | `shipped` keeps each port's configurations. `brotli`, `gzip` or `raw` rewrites `cost_model` in every copied `lilscript*.toml` | `shipped` |
| `--json` | where to write the report | `<work>/report.json` |
| `--work` | scratch directory; logs go to `<work>/logs/<port>.{build,test}.log` | `target/verify/ports` |
| `--ports-root` | where the port checkouts live | `~` |
| `--ledger` | the expected-failure ledger | `tests/ports/expected-failures.json` |
| `--timeout` | seconds per build or test step | 2700 |
| `--jobs` | ports built in parallel | 1 |
| `--codec` | `lilscript-codec`: measures `dist/`, and is passed to the port as `LILSCRIPT_CODEC` | the codec beside the compiler |
| `--keep` | keep the scratch workspace | off |

What it does for each port:
1. **Copy.** The port is copied into `<work>/runs/<port>/<port>`, without `.git`, `dist`, `_site`, `.tmp` or `test-output`. Every `node_modules`, at any depth, is linked rather than copied. The scratch parent links the sibling ports and this repository as `../lilscript`, because ports import siblings' sources. **Nothing is built inside `~/<port>`.**
2. **Patch.** `finer/port-migrations/<port>.patch` is applied if it exists.
3. **Objective.** For `--objective` other than `shipped`, `cost_model` is set in every copied `lilscript*.toml`. This is the logic of `~/lilscript-work/tools/cfgvar.py`.
4. **Build.** `npm run build` runs, or `node scripts/build.mjs --compile [--force]` when the npm script omits a `--compile` flag that the script accepts. The environment carries:
   - `LILSCRIPT_COMPILER` and `MOTIONLIL_LILSCRIPT_BIN`, pointing at a wrapper that logs each call to the pinned binary;
   - `LILSCRIPT_ROOT` and `LILSCRIPT_CODEC`.

   The caller's other `LILSCRIPT_*` variables are dropped. `dist/` is then measured with the codec.
5. **Test.** `npm test` runs. A port with no `test` script runs `npm run check:site`. A port with no `package.json` (probelil) runs its differential build as its test.
6. **Compare.** The failing-test **set** is parsed (node:test TAP and spec, jest, vitest and mocha) and compared with the ledger.

The runner also records failures that are not tests, under pseudo-names:
- `(patch failed)`;
- `(build failed)` and `(build timed out)`;
- `(no npm test script)`;
- `(suite failed without named failures)` and `(suite timed out)`;
- `(compiler not invoked)`: a run that compiled nothing is not evidence;
- `(runner error: …)`.

The report pins the compiler and codec digests, each port's git HEAD and dirty state, the patch digest, and every configuration rewrite. It also gives each `dist/` file's bytes, SHA-256 and codec sizes, the number of compiler calls, and the build and test times.

The runners' pure parts (feature detection, lane selection, configuration merging, objective rewriting, failing-test parsing) have unit tests: `node --test scripts/verify-runners.test.mjs`.

## The generic corpus ratchet

`scripts/naming-perturbations.py --compiler <bin> --codec <codec> --work <dir>
--json <report>` exercises captured bindings below and above the one-character
name limit, with three source-name perturbations and two independent closure
activations. It compares alphabet search permitted/vetoed under raw, gzip and
Brotli, checks an independent Node arithmetic/event-order oracle, and retains
sources, configurations, output hashes and all three measured sizes. This is
development coverage, not a held-out calibration set.

```sh
node scripts/ratchet.mjs --compiler target/release/lilscript
node scripts/ratchet.mjs --compiler <bin> --filter 'apps/*,cases/catalog/loop/*' --json out.json --markdown out.md
node scripts/ratchet.mjs --compiler <bin> --update-baseline
node scripts/ratchet.mjs --refresh-bars --compiler <bin> --reference ~/lilscript-work/bin/reference-2026-09-23/lilscript
```

The architecture's NO2 (§18.3) as a blocking gate (L21, plan rule 3). Three corpora, none of them a port:

| Corpus | Items | Oracle | Configuration | Bars |
|---|---|---|---|---|
| `comparison/cases` | 54 canonical folders and the 570 catalog variants; a catalog id carries its behavior family, `cases/catalog/<family>/<name>` | the original JavaScript's stdout; the catalog's oracle digest (`oracle-manifest.json`) must match | `comparison/cases/configs/<codec>.toml` | competitor, old |
| `comparison/apps` | 7 programs written for Closure ADVANCED | `tests/stdout.txt` | `comparison/cases/configs/<codec>.toml` | competitor, closure, old |
| `comparison/algorithms` | 11 host-fed programs | each vector's stdout and the reference program's ordered host accesses | `comparison/algorithms/configs/<codec>.toml` | competitor, closure, old |

Each item compiles once per objective lane (raw, gzip, Brotli; `--target js --format bare --mode production`), runs against its oracle and is measured with `lilscript-codec`. The explicit bare container preserves the historical measurement contract after application builds adopted a private IIFE default. `--format iife` measures that new default separately; reports and new baselines record the container, and a different container blocks baseline replacement even with `--accept-growth`. Its artifact is then compared, in the lane's own metric, with the bars in `tests/ratchet/bars.json`:
- **competitor**: the smallest valid Terser (with and without its safe property lane), Oxc or esbuild artifact, from the recipes in `comparison/cases/recipes.mjs`; the algorithms add their harness's bundler lanes;
- **closure**: Closure ADVANCED (apps: `comparison/artifacts/*/closure-advanced.js`; algorithms: their harness);
- **old**: the frozen old route, `reference-2026-09-23 --backend legacy`, with the configurations it read (`tests/ratchet/old-route/`).

An item **loses** to a bar when it is larger. A lane with no artifact (refused, crashed, wrong output) is a **failure**.

| Flag | Meaning | Default |
|---|---|---|
| `--compiler` | the binary under test (copied and pinned by SHA-256) | required |
| `--codec` | `lilscript-codec` | beside the compiler |
| `--sets` | `cases`, `apps`, `algorithms`, comma-separated | all three |
| `--filter` | item ids: comma-separated substrings or globs | every item |
| `--format` | explicit classic-script container, `bare` or `iife` | `bare` |
| `--config-dir` | experimental directory with `raw.toml`, `gzip.toml` and `brotli.toml`, applied to each selected corpus | corpus configurations |
| `--retain-explanations` | retain each compiler's JSON explanation and warnings as a hashed `.explain.txt` receipt in the work directory | off |
| `--jobs` | parallel compiles and runs | min(3, CPUs − 2) |
| `--json`, `--markdown` | reports | — |
| `--bars`, `--baseline`, `--ledger` | the three files below | `tests/ratchet/` |
| `--update-baseline` | write the run as the new baseline; refused while anything blocks, and from a partial run | off |
| `--accept-growth` | with `--update-baseline`, accept recorded local loss growth only if every corpus subtotal is nonincreasing in every objective | off |
| `--accept-corpus-growth REASON` | additionally permit a corpus subtotal increase when the same full input set passes and raw, gzip and Brotli totals each do not grow; retain the reason and exact deltas | off |
| `--refresh-bars` | rebuild `bars.json`: runs both comparison harnesses (they need `benchmarks/popular`'s pinned competitors and Closure) and compiles every item with `--reference` | off |

Reports retain configuration file hashes and resolved policy receipts. Each
lane must resolve its named objective with the canonical encoder settings used
by the ratchet's bars. Configuration changes during measurement block acceptance.
An experimental configuration directory cannot update the ordinary baseline,
including through a symbolic link; use a separate `--baseline` for that request.
The compiler processes run without the legacy `LILSCRIPT_WALK_AUDIT` adapter or
optional `LILSCRIPT_TIMING` instrumentation, so TOML controls their search.

**What blocks (exit 1).** Against `tests/ratchet/baseline.json` (each item's sizes, or failure state, from the last accepted run):
- any loss count grows, per corpus, metric and bar;
- any item's loss to any bar grows, including a new loss (growth from zero), compared over the lanes that passed in both runs: a lane that starts compiling (a refusal fixed) is not growth, but its losses need ledger entries like any other;
- an item lane that passed now fails;
- a loss or failure that no entry of `tests/ratchet/ledger.json` covers;
- the bars changed under the baseline: a bar refresh is a scheduled re-baseline event (BC3), recorded with `--update-baseline`.

**What is reported.** Loss counts that fell, losses that shrank, lanes that now pass, new corpus items, and ledger entries that cover nothing (on full runs). `--update-baseline` tightens the baseline to them. Growth requires an explicit acceptance flag. The ordinary verdict and every local loss stay visible; reports and the baseline retain the acceptance receipt. The corpus-growth override requires unchanged bars/contracts, no unledgered losses and every old/new lane passing on identical inputs. It cannot blend objectives, hide failed lanes or waive other blockers.

**The ledger.** Entries of kind `loss` (items, optional `bars` and `metrics`) or `failure` (items, optional `metrics`), each with a `reason` and an `owner` task; the runner refuses an entry without them. Coverage is not permission to grow: the baseline still blocks growth of a covered loss.

**Bars and time.** The gate needs Node, the compiler and the codec only; the bars, the old route's configurations and the algorithms' reference host traces are committed. A full run is about 40 s at `--jobs 3` on the build host, and runs in CI's gate job.

### Baseline: head-a1, 2026-09-27

Bars refreshed on 2026-09-27: Terser 5.50.0, Oxc through Rolldown 1.2.4, esbuild 0.28.1, Closure v20260804, and the reference binary (SHA-256 `df85958…`). The baseline was first recorded on `head-d1d48c4c` (SHA-256 `47048e41…`), then re-recorded on `head-a1` (SHA-256 `7e720be7…`, batch A1 merged): A1 changed no loss count and no `comparison/apps` or `comparison/algorithms` artifact, and shrank 18 loss rows of three host variants (`host-callable-fn`, `host-callable-as-value`, `host-raf-or-null-type`: a method is its own function). A build of this branch reproduces every item's sizes.

Totals, ours against each bar over the items where both exist, and how many items lose (Brotli):

| Corpus | Items passing | vs smallest competitor | vs Closure ADVANCED | vs old route |
|---|---:|---|---|---|
| `comparison/cases` | 612 of 624 | 49,170 / 43,541 (439 lose) | — | 49,073 / 33,656 over 611 (603 lose) |
| `comparison/apps` | 7 | 945 / 870 (5) | 945 / 834 (7) | 945 / 558 (7) |
| `comparison/algorithms` | 11 | 3,250 / 3,039 (9) | 3,250 / 2,703 (11) | 3,250 / 2,305 (11) |

| Loss counts | raw | gzip9 | brotli11 |
|---|---|---|---|
| cases vs competitor / old | 399 / 610 | 400 / 600 | 439 / 603 |
| apps vs competitor / Closure / old | 1 / 5 / 7 | 3 / 6 / 7 | 5 / 7 / 7 |
| algorithms vs competitor / Closure / old | 5 / 11 / 11 | 8 / 11 / 11 | 9 / 11 / 11 |

Twelve cases fail in every lane, both ledgered: eleven are refused on record spread (M10.8) and one on `??=` on a place (M10.9). The 27 catalog variants that called name-keyed host helpers the old route gave bodies to (`mathMax`, `objectHasOwn`, `isFunctionValue`, …; dropped by design in M1), and the canonical `host/math-max`, were rewritten to declared host bindings (`extern class` views of `Math`, `Object` and `Reflect`, `globalThis`) or to the predicate the reference program spells; their JavaScript oracles are unchanged. The case configurations load with no "no effect" or rename warning (BC12): the six retired keys they carried were removed, `strip_console = false` became `strip_debug = false` with an explicit `strip_console_calls = false` after batch A1 split the key, and each configuration's policy fingerprint is unchanged by either step.

## The admission parse (A5)

Every artifact is parsed again before admission accepts it (`ArtifactArena::qualify`, `src/program/artifacts.rs`; plan task M2.5). Oxc, an independent standards parser, reads each delivered file as a module or a script, as the artifact executes (regular expression literals validated; `import`/`export` refused in a script). For a file printed whole, the structure Oxc reads must be the structure the printer printed: both sides reduce to one canonical form and compare digests (`src/admission_parse.rs` states the form and its closed list of spelling equivalences; `src/js/admission.rs` reduces the printed tree). A file that does not parse, or parses to another structure, is refused with a diagnostic that names where: the parser's message and byte offset, or the first group of top-level statements that differs, with an excerpt. The direct path and the search baseline then fail the build with it; a refused search candidate stops the search with the best admitted artifact (the report's `stop`); a refused terminal challenger is `Refused`. A delivery plan's files (M3.3a) are each parsed, as ES modules or, under `format = "cjs"`, as scripts, but not yet compared by structure: they print through `src/js/print_files.rs`, and comparing each file with its part of the tree is owned by M3.3b, next to the plan verifier.

The verdict is computed once per artifact and costs linear work in its bytes. `LILSCRIPT_TIMING=1` reports it as two buckets, `admission_structure` (the tree's digest, once per render) and `admission_parse` (once per qualified artifact). On 2026-09-27, level 13 at the shipped configurations: katexlil 525 ms of 23.9 s (2.2%; 32 renders and 32 parses of 261 KB), markedlil 62 ms of 1.56 s (4.0%).

## Library knowledge (NO3) and idiom debt (NO4)

`src/no_library_knowledge_tests.rs` runs with `cargo test --lib`. It reads every non-test file under `src/` (test files, fixtures and inline `#[cfg(test)] mod … {}` bodies excluded) and counts mentions of ports and upstream libraries (case-insensitive names such as `katex`, `jquery`, `zod`; library spellings only for English words such as `markedlil` or `motionlil`). Each (file, library) count must equal its entry in `tests/no3-allowlist.json`, which gives the reason and the owner task: a new mention fails, and a removed mention must lower its entry in the same change, so the ledger only shrinks. On 2026-09-27 it held 30 mentions in 22 entries, every one a comment that justifies a rule, a default or a schedule constant by one port's measurement; after batch A1 (which removed the katex import pin and added two citations) it holds 31. M8.7 empties it.

`tests/idiom-debt.json` is the idiom debt ledger (schema `tests/idiom-debt.schema.json`, checked by `node --test scripts/verify-runners.test.mjs`). An entry records a port rewrite needed only because the compiler handles the idiomatic form badly: both forms, the idiomatic form's regression case in this repository, their sizes under one codec and binary, and the owner task; the compiler owes size(idiomatic) ≤ size(workaround). It starts empty; the pairing tasks of M12.6 and the language slices file into it.

## Delivery checks

Multi-file delivery (plan M3.3: several entries, `split`, `preserve-modules`, lazy files, manifest v3) is checked in three places; each expected output comes from the sources' ES module semantics, never from the compiler under test.

| Check | What it asserts |
|---|---|
| `src/build_delivery_tests.rs` (unit tests) | Scenarios per load sequence in Node: shared instances and identities across entries, per-entry order, a throwing initializer, setters, lazy loads that evaluate what the importer has not (in the target's order), `import()` of a module another entry imports statically, carried host code per entry, names under the default templates, foreign specifiers from nested files, content-safe `[hash]` names, the M3.3a refusals |
| `scripts/delivery-contract.mjs` | Each `tests/bundles` fixture's behaviour and its manifest v3: bytes and SHA-256 equal the files, `side_effects` equals the anchored files, no file imports an entry or lazy file, every file with effects in an entry's closure carries its label, rows equal the closure's codec bytes; then the command line: identical names and bytes under `-j 1` and `-j 4`, from another working directory, a copy at another absolute path and reordered `--entry` flags; `-o FILE` naming; stale-file removal; the refusals before a compile |
| `scripts/delivery-plan.mjs` | The exact files, roles and labels of each fixture; a change is legal only with an explanation in its commit |

`scripts/verify-bundles.mjs` runs both scripts, then the `--target all` and package-lock checks; `scripts/verify.sh` runs it. The case runner does not build several entries yet: the design's delivery cases (a `[delivery]` table in a case's `.toml`) are an open item of M3.3.

```sh
node scripts/verify-bundles.mjs target/release/lilscript
```

## Paired compilation cost

`scripts/compile-cost.py` runs pinned compilers sequentially on a manifest of
identical inputs/configurations. Each workload has `id`, `cwd`, `entry`,
`config`, optional `target` (default `js-module`) and optional `arguments`.
Relative working directories resolve from the manifest. Supply `--previous`,
`--candidate`, `--codec`, `--manifest`, `--work` and `--json`; `--rounds`
defaults to five alternating pairs after one separate warm-up per compiler.

The JSON retains every measured process-CPU/wall sample, order and host load,
summary spread, runner/manifest/input/configuration and binary identities, artifact hashes
and exact codec sizes. Warm-up explain reports and elapsed phase telemetry
are outside timed samples. Input changes, policy mismatches and unstable
output fail the run. A median alone is insufficient evidence of a speed win;
inspect the retained samples and explain changed work counts. This runner
measures compilation, not the generated program's runtime.

`LILSCRIPT_TIMING=1` includes each target rule's `js_rule_<Rule>_ms` and
`js_rule_<Rule>_calls`. These count actual executions, including failures, and
sum to the aggregate rule call count. Their elapsed times are nested inside
`js_rule_ms` and `js_formation_ms`; they must not be added to those totals.
Use the separate timed samples for compilation-cost claims.

An intentional compiler schedule/tactic revision can use
`--allow-policy-change "reason"`. The report retains the reason and both
effective policies. Target availability, program contract, objective encoder
settings, runtime priority, effort and resource/acceptance limits must still
match; this option cannot relax them. Without it, any policy mismatch fails.
Resource comparison validates the requested, configured and effective ceilings
in both reports, including when older policy objects omit the service ceiling.
Moving the same effective limit into policy identity is allowed with a reason;
different limits, contradictory receipts or missing evidence fail. Reports keep
the matched limits, and runner/manifest changes during measurement also fail.

## Paired generated-program runtime

`scripts/runtime-cost.py` measures generated programs against a reference on a
shared workload. Every sample gets a fresh Node process, the same declared host
setup and oracle, then separate import/startup and warmed-work timers. Pairs
alternate reference/candidate order. The report preserves each sample, median,
range, standard deviation, paired ratios, host load and process CPU/wall time.
A short synthetic run qualifies the machinery; it cannot establish port parity.

```sh
python3 scripts/runtime-cost.py --manifest runtime.json --work /tmp/runtime-pair-1 \
  --json /tmp/runtime-pair-1.json --rounds 5
python3 scripts/runtime_cost_test.py
```

The report path must be new and the work directory empty. The manifest is JSON:

```json
{
  "schema": 2,
  "workloads": [{
    "id": "sum",
    "contract": {"exports": ["twice"], "target": "ESM", "semantics": "bounded integers"},
    "workload": "workload.mjs",
    "iterations": 100,
    "warmup_runs": 1,
    "expected": 9900,
    "counters": {"calls": 100},
    "inputs": [],
    "implementations": {
      "reference": {"entry": "reference.mjs", "inputs": []},
      "candidate": {"entry": "candidate.mjs", "inputs": ["candidate.lil", "config.toml"]}
    }
  }]
}
```

All paths resolve relative to the manifest. Declare every transitive source,
artifact, data and dependency file in `inputs`; the runner does not infer a
module graph. It hashes these files, both entries, the workload, manifest,
engine and runner before sampling and checks for drift afterward. The report
retains those identities and the effective engine versions/arguments. Workers
use a fixed environment (`PATH`, UTC timezone and C locale), without ambient
Node preload flags. A workload must declare other external dependencies in its
contract and make them reproducible; this tool does not pin operating-system
libraries or external services.

The common workload module exports `run(namespace, iterations)` and optionally
an asynchronous `install()` for host setup before artifact import. It must not
preload either artifact. Each `run` returns `{oracle, counters}`; every warm-up
and measured invocation is checked against the manifest's independently derived
`expected` and declared counters. Counters describe workload operations, not
compiler instructions. A workload must reset its state as required by its
contract so that successive invocations have that same expected result. Keep
logging off stdout, which carries the worker protocol.

Only artifact import and the measured `run` are inside their respective timers;
worker verification and forced GC are outside them. Warm-up has its own retained timers. Any checks authored inside a common workload are part of that workload on both sides. `startup_cpu_us` and
`steady_cpu_us` measure Node process CPU; wall fields are nanoseconds. Separate
`process_*_seconds` fields include process launch, host setup, warm-up and GC.
The optional `retained` result field keeps a workload object alive across GC and
enables post-GC heap/ArrayBuffer deltas. These noisy deltas can be negative and
are preserved without clamping; they are **not allocation totals or peaks**.
Without `retained`, those two fields are null. `rss_bytes` is the post-GC process
resident size. Nonpositive baselines have no memory ratio.

Wrong oracles/counters, worker failures, timeouts and changed inputs leave an
incomplete report and fail the command. An incomplete report cannot qualify an
optimization. Choose enough work and pairs to distinguish a change from the
retained sample spread; inspect startup, steady state and memory separately.

Runtime manifest **schema 2** adds three reference identity controls by default.
Each loads the same reference bytes with a distinct import URL in a fresh
process. Sample positions rotate and reverse; controls must pass the same
oracle. These measure process/JIT/order variation, not encoder noise or a
permission for runtime regressions. Schema 1 remains compatible without
controls; `--controls 0..3` is explicit. A control-sensitive import must declare
that limitation instead of treating a different program as a no-op control.

Reports retain per-warm-up CPU/wall observations and deterministic 95% paired
bootstrap intervals (2,000 resamples, seed 0). They resample process pairs,
never inner iterations as independent observations. Optional `runtime_limits`
map a metric to a nonnegative tolerated regression fraction, for example
`{"steady_cpu_us": 0.05}`. Fewer than five pairs, missing controls, a control
spread larger than the tolerance, or an interval crossing the limit is
**inconclusive**. A lower interval bound beyond the limit is a regression;
an upper bound within it establishes only the declared limit for that workload.
`--require-limits` fails unless every declared workload establishes its limits.
No universal percentage is a compiler policy: choose the bound and workload
before measuring an affected change, retain startup/memory separately, and
increase work or sample count when the result is inconclusive.

`lilscript-runtime-inventory delivered.mjs` independently parses a delivered
artifact and counts constructor/array/object/function/call/rest/arguments and
forwarding-body syntax sites. Attach its JSON as an implementation's `inventory`;
the runner verifies the artifact hash. These are static audit counters, not
allocation totals, materialized arguments objects, proven adapter identities or
execution frequencies. Dynamic workload counters and engine profiles serve
those separate purposes. Unknown measurements stay unknown.

`benchmarks/runtime/prepare.mjs` freezes compiler-written reference-port cores,
independently bundled upstream entries and generic allocation/call/string
workloads. Its receipt retains source/tool/artifact identities; expected values
come from authored behavior fixtures or independent arithmetic, never from
candidate output. Runtime qualification and compiler tactic permissions are
separate: allowing a risky tactic in TOML does not establish parity.

## Fair competitor contracts

Install the pinned tools once with `npm ci --ignore-scripts` in
`benchmarks/contracts`, then run:

```sh
node scripts/bars.mjs --manifest bars.json --lock /tmp/bars-lock.json \
  --write-lock --work /tmp/bars-first --json /tmp/bars-first.json
# Replay requires exact source, binary, runner and installed-dependency content:
node scripts/bars.mjs --manifest bars.json --lock /tmp/bars-lock.json \
  --work /tmp/bars-replay --json /tmp/bars-replay.json
```

The manifest declares open-library, closed-application or consumer-bundle
boundaries separately, ES2020 delivery, exact exports/externals/reflected keys,
language guarantees and host assumptions, source-equivalence reasoning,
compiler/config provenance and one common independent observer. It identifies
our artifact for **each** raw/gzip/Brotli objective. Open ESM entries are generated
from the declared surface before bundling and minification. The observer runs
against the unminified input, each candidate and each competitor in a fresh
process. Wrong behavior cannot establish a bar. API-wide equivalence still
requires adequate fixture coverage and review of the declared source contract.

Recipes include Terser, SWC, the Oxc minifier, Rolldown as a separate bundler,
esbuild, Closure ADVANCED and a declared upstream artifact. The pinned Closure
CLI recipe refuses an open ESM boundary; closed script consumers can use
ADVANCED, and open script APIs need externs. Other inapplicable boundaries or
failed recipes retain reasons. A missing recipe is never a zero-byte result.
Property renaming is permitted only under the declared reflection/extern
contract. No raw-size similarity threshold substitutes for behavior.

The runner selects the minimum eligible passing competitor independently per
objective. It scores complete actual artifacts with the canonical codec, retains
Brotli quality 5 and gzip level 6 robustness rows (with their Node codec versions),
static inventories and complete-process wall cost. Recipe CPU is additionally
reported where available; native child-process CPU is explicitly unavailable.
Installed tools, including native binaries and runtime dependencies, are
content-locked, not merely version-named. No wrapper, banner removal or
post-minifier changes the candidate after scoring.

Runtime debts are checked with `node scripts/runtime-ledger.mjs REPORT tests/runtime-evidence.json`. Each entry names its owner, reason and receipt and
matches the exact engine, inputs, artifacts, contract, metric and disposition.
A changed candidate cannot inherit an exemption. Unexpected or stale entries
fail; an accounted-for inconclusive result still reports
`parity_established: false`.

## The expected-failure ledgers

| Ledger | Entry |
|---|---|
| `tests/cases/expected-failures.json` | `case`: an id, a glob or a list of them. `lanes`: in the `--lanes` syntax; omitted means all. Plus `reason` and `owner` |
| `tests/ports/expected-failures.json` | `port` and `tests` (exact failing names or pseudo-names), plus `reason` and `owner`. `intermittent: true` marks tests whose outcome depends on the host, such as an upstream wall-clock timeout on a loaded machine: they may fail, and passing is not reported for removal |

The rules:
- Every entry names the plan task that owns the fix (`owner`, from [the migration plan](migration/plan.md)) and says why it fails (`reason`). The runners refuse a ledger entry without them.
- A failure the ledger does not cover fails the run (exit 1). The runner prints it with its lanes and first difference.
- **A ledgered failure that passes is reported for removal.** The case runner calls an entry stale when it covers no failure at all, and with `--verbose` lists entries that also cover passing lanes. The port runner lists each ledgered test name that now passes.
- An entry is added only with its owner, in the same commit as the change that exposes the failure. It is removed in the commit that fixes it.
- A ledger never holds a miscompile without an owner, and an expected output is never rewritten to make a case pass.

## Baseline: the pre-M1 binary, 2026-09-24

Binary `~/lilscript-work/bin/pre-m1/lilscript`, SHA-256 `01c5e298…`, built from `0c17237e`.

**Case runner.** 361 cases × 18 lanes = 6,498 case-lanes, in about 35 s with 6 jobs.
- The 72 `tests/cases` programs and `modules/main` pass on every lane.
- Of the 288 harvested regressions, 29 fail somewhere. All 29 are ledgered.
- Each script lane passes 343 cases, with 14 ledgered and 4 masked. Each module lane passes 343 to 345, with 16 to 18 ledgered. Each C lane passes 115, with 10 ledgered and 236 masked.
- The C lanes give the same counts under Clang 18 (`--cc ~/toolchains/clang-18/clang`). The emitted C is byte-identical between the formation-only and production lanes.

**Port runner.** markedlil and zodlil are green under each objective (`shipped`, `brotli` and `raw`): 29/29 and 1,353/1,353 tests pass, with 4 and 2 compiler calls. Each port takes about 5 s to build and 0.5 s (markedlil) or 12 s (zodlil) to test.

| Artifact | `brotli`: raw / Brotli | `raw`: raw / Brotli |
|---|---|---|
| `markedlil/dist/marked.esm.js` | 36,604 / 9,264 | 35,154 / 9,566 |
| `zodlil/dist/zod.core.js` | 125,776 / 27,790 | 112,651 / 28,903 |

The ledgered ports behave as their entries say:
- mdast-util-from-markdownlil: 743/744;
- mobxlil: 766/780, with 3 failures;
- react-markdownlil: the build fails and 1/8 tests pass;
- monacolil: its `check:site` passes, 3/3.
