# Testing the compiler: the case runner and the port runner

Two versioned runners check a compiler binary. Both pin the binary by SHA-256, both compare failures against an **expected-failure ledger**, and both exit 1 only on a failure the ledger does not list.

| Runner | Plan task | Replaces |
|---|---|---|
| `scripts/cases.mjs` | M2.2 | `finer/tools/semantic-census.mjs` (development mode only) and the knob configs in `tests/config/` |
| `scripts/ports.mjs` | M2.6 | `finer/tools/semantic-port-tests.mjs`, `finer/tools/portgate.mjs` and the unversioned `~/lilscript-work/tools/*.sh` |

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
| `--cc` | C compiler for the C lane, run with `-std=c11 -O2 -fno-fast-math -ffp-contract=off … -lm` | `/usr/bin/cc` (GCC 13 here) |
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
| `X.toml` | Configuration keys merged into the lane's configuration. A key joins the lane's table of the same name, so there is one `[javascript]` table, never a second. A case key overrides the lane's value; a case may not set `strip_console` |
| `X.module-probe.mjs` | Module lane only. The case is built with `--target js-module`; the runner imports the output as `m` and awaits the probe's default export, called with `m` |
| `// harness: "use strict"` in the first lines of `X.lil` | On the script lane, `"use strict";` is prepended to the run file |
| `X/` | The case's other modules, which the entry imports as `./X/…` |

Every lane compiles with `[javascript] strip_console = false`, because `print` is the observation channel.

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

### Per-target feature masks

The mask is declared once, in `FEATURES` in `scripts/cases.mjs`. Detection is lexical, on source with comments and string text removed. `export` counts only in the entry module, because an imported module's exports are internal linkage. Every other feature counts in any module the entry imports.

| Feature | Runs on | Why | Lifted by |
|---|---|---|---|
| `.host.js` prelude | script, module | defines externs in a JavaScript realm | — |
| `.module-probe.mjs` | module | imports the ES module's exports | — |
| `JsValue` | script, module | JavaScript-only (language-v0.1) | — |
| `extern` | script, module | C rejects host declarations | M11.3 |
| `export` in the entry | script, module | the exports are a module ABI; C has none yet | M11.8 |
| `JS.` operations | script, module | JavaScript-only | — |
| `async`, `await`, `Task` | script, module | native rejects them (language-v0.1) | M11.6 |
| `throw`, `try` | script, module | native rejects exceptions | M11.6 |
| `Regex` | script, module | native rejects it | M11.6 |
| `generator` | script, module | native rejects generators | M11.6 |
| `object { … }` | script, module | JavaScript-only | — |
| `JSON.parse` | script, module | returns `JsValue` | — |

The first five rows are the base rule: a case with a `.host.js`, a `.module-probe.mjs`, or source mentioning `JsValue`, `extern` or `export` is JavaScript-only. The other rows are the native rejections that [language-v0.1.md](language-v0.1.md) states. A compiler gap is never a mask: native `Record<T>` is missing, so it is a ledgered failure owned by M11.4.

### Artifacts and comparison

For every compiled (case, lane), the report records the artifact's byte size and SHA-256. Every JavaScript artifact is also measured with the canonical codec (`raw`, `gzip9`, `brotli11`). The summary gives each lane's total in its own objective.

`--compare previous.json` prints, per lane, how many artifacts changed and the byte and objective totals before and after. It also lists state changes and the largest per-case byte changes; the JSON has all of them. Two runs of the same binary must compare identical; two runs of the pre-M1 binary did, on every compiled artifact of every lane.

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

## The expected-failure ledgers

| Ledger | Entry |
|---|---|
| `tests/cases/expected-failures.json` | `case`: an id, a glob or a list of them. `lanes`: in the `--lanes` syntax; omitted means all. Plus `reason` and `owner` |
| `tests/ports/expected-failures.json` | `port` and `tests` (exact failing names or pseudo-names), plus `reason` and `owner` |

The rules:
- Every entry names the plan task that owns the fix (`owner`, from [the migration plan](migration/index.md)) and says why it fails (`reason`). The runners refuse a ledger entry without them.
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
