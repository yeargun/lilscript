# Migration status

Parent: [index](index.md). Volatile: rewritten as work lands. Plan: [009](009-phases.md).
Updated 2026-09-04.

Numbers taken on the orchestrator host are **triage, not evidence**
([D5](001-directives.md#d5--compiles-run-on-the-pool-not-on-this-host)). Nothing here has been
through a fleet A/B yet.

**Gate state: `cargo test --release --lib` is 1,710 passed / 0 failed / 1 ignored** with every change
below applied, alongside the other session's in-flight edits.

**First real `portgate` run (markedlil, `optimization_level = 15`):** `trust: ok`, 406 s, seven
artifacts, **suite passes with zero failing tests**. Its timing line independently reproduces the
idle-fold figure this migration rests on — 40,222 idle against 11,537 active fold calls, **77.7%
idle** — and records 98,226 `lex_calls` and 1,737 CPU-seconds of emission across 1,058 emissions
(1.64 s each).

`dist/marked.umd.js` came out at raw 37,726 / **Brotli 10,224**, against the committed dist's
37,979 / 10,198. **That +26 is not attributable to the fixes above**: the committed artifact was
produced by an older binary that also carries every other change since, so the two arms differ by far
more than this diff. A clean number needs two pool arms over the same source with only these commits
between them ([D5](001-directives.md#d5--compiles-run-on-the-pool-not-on-this-host)) — that is
exactly what `fleet-compare.mjs --require-identical-unless-declared` now exists to run.

---

## Phase 0 — repair the instrument

| Item | State | Evidence |
|---|---|---|
| 0.1 assertions are real | **landed** | `[profile.release]` (explicit, `debug-assertions = false`) and `[profile.release-assert]` in `Cargo.toml`; manifest validated |
| 0.2 ports are a gate | **landed, smoke-tested** | `finer/tools/portgate.mjs`; posthoglil recorded `trust: ok` in 139 s, 22 artifacts with SHA-256 + Brotli, 4 timing lines parsed |
| 0.3 differential is generative | **landed** | `--random-seed` on `lilscript-differential`; seed printed before work starts; `scripts/verify.sh` uses it, `LILSCRIPT_DIFFERENTIAL_SEED` replays |
| 0.3b domain extended to classes/closures | **not started** | the domain where every known miscompile lives |
| 0.4 baseline frozen | **not started** | needs the pool and the F-gaps below |
| 0.5 live bugs fixed | **4 of 7** | see below; plus one unsound *test* corrected — `elides_char_code_at_integer_normalization_when_proven` asserted the elision on a source that proves nothing (`read()` returns a string of unknown length, and `"".charCodeAt(0)` is NaN) |

### Fleet gaps ([008](008-fleet.md))

| | State | What changed |
|---|---|---|
| F1 `fleet.mjs --workers` cannot A/B | **landed** | forwards `--compiler`, `--dist-dir`, `--log-dir`, `--instances`, `--per-worker`, `--vmss`, `--rg`, `--user`; reads the arm's own `last-build.json` |
| F2 A/B tools in an ignored scratch dir | **landed (compare)** | `finer/tools/fleet-compare.mjs` promoted, with `--require-identical-unless-declared`; the baselines table extracted to `finer/tools/baselines.mjs` so it is no longer regex-scraped out of `fleet.mjs`. `fleet-tests.mjs` still to promote |
| F3 skipped compile reports false green | **landed** | `workers.mjs` passes `--force` where the port's script accepts it, and **fails a build that exits 0 without a `lilscript-timing` line** |
| F4 A/B destroys arm A's telemetry | **landed** | `--log-dir` on `workers.mjs`; per-arm `last-build.json` |
| F5 no compiler provenance | **landed** | `last-build.json` records the compiler path and SHA-256 |

---

## The release gate was red at HEAD — now fixed

`scripts/verify-matrix.sh` runs every `tests/cases/*.lil` at `preset = "maximum"` and at
`preset = "none"`, under `set -eu`. `optional_constructor_callback` **fails to compile** at maximum:

    $ target/release/lilscript tests/cases/optional_constructor_callback.lil --target all -o /tmp/x
    error: SSA value 3 has no emitted name in function `sameParity`

So `verify-matrix.sh` aborted, `verify.sh` aborted, and `release-check.sh` — the whole release gate —
could not pass. That was not a regression introduced here; it is the state this work found.

**Fixed.** The case now compiles and produces `true false true false`, matching its golden `.out`
byte for byte.

Minimised to three lines, and it needs neither the class nor the optional parameter:

```lilscript
bool sp(int a, int b) { return a % 2 == b % 2; }
bool use<T>(T x, func(T,T)->bool f) { return f(x, x); }
print(use(4, sp));
```

The trigger is **a generic function with a parameter whose type mentions the type parameter inside a
function type** (`func(T,T)->bool`). A `func(int,int)->bool` parameter on the same generic function is
fine; a non-generic function with the same callback parameter is fine; passing a lambda instead of a
named function fails the same way (in `<closure>` rather than `sp`).

Bisected: `preset = "none"` compiles it, `preset = "maximum"` does not, and **none of the eleven
individual optimizer toggles fixes it** — so the responsible option is one of the five in
`OptimizationOptions` that has no config key at all (`forward_global_aliases`,
`inline_exported_internal_calls`, and the three inline limits).

**Root cause, found by making the error explain itself** (`inlined=? param=? uses=?`):

    error: SSA value 3 has no emitted name in function `sp`
           (inlined=true param=false uses=2 named: [v0=b v1=c v4=a v7=d v8=e])

The value is in `inlined_values` — the emitter holds a `JsExpression` to substitute at its use site —
**and it has two uses**. Substituting at "the" use site is only meaningful for a single use, so a
consumer correctly asked for a name, and the naming loop had skipped it:

```rust
for value in values {
    if inlined_values.contains_key(&value) { continue; }   // ← no name for a 2-use value
    value_names.entry(value).or_insert_with(...);
}
```

Fixed by narrowing the skip to the case it is actually sound for — a single use — and never skipping
a parameter, which is a binding in the signature rather than an expression at all.

This is exactly the shape [004 §8](004-legality-by-construction.md) predicts: the namer and the
emitter are two walks over one function that disagree about which values need a name, and the
disagreement is discovered at emission by a failed map lookup rather than prevented. Under the target
representation the question does not arise — an identifier names a `Bind`, and a node either has a
binding or is an expression, not both.

---

## Live wrong programs

Seven found, all reproduced. Three fire in a **default** configuration.

| # | Wrong program | State |
|---|---|---|
| 1 | `extern` rewritten by source spelling (105 names) | open — the real fix is the declared host binding in [003](003-target-representation.md); a hard error would break the ports that currently rely on the table |
| 2 | closure `this`/`arguments` rebound by arrow spelling | open — diagnosed in 061; the port fix landed (`jquerylil 81250cb`), the compiler fix is a fleet-rule change and wants its own folder. **The obvious fix is wrong** — see [004 §2](004-legality-by-construction.md) |
| 3 | `lilscript.toml` silently ignored for a bare relative filename | **fixed** — `config_search_parent` + regression test |
| 4 | `charCodeAt` out of range yields `NaN` instead of `0` under the default `size-first` | **fixed** — `StringCharCodeAt` keeps its post-coercion range but is no longer elidable |
| 5 | `preset = "none"` deletes a module global's binding while a use renders its name | open — two disjoint declaration paths in `codegen_ir_js.rs`. **This is the optimizer-ablation control lane `verify-matrix.sh` runs every case through** |
| 6 | `JS.number(x["length"])` loses its `ToNumber` under the default `size-first` | **fixed** — and the fix had to go at the *scored family's admission predicate*, not the option default: turning `elide_length_tonumber` off was not enough because the `length-to-number-elision` family flips it back on a candidate and the search takes the shorter, wrong spelling. See [004 §2b](004-legality-by-construction.md) |
| 7 | `optional_constructor_callback` fails to compile: "SSA value 3 has no emitted name" | **fixed** — a two-use value was classified inlinable, so it got no name; the skip now applies only to a genuine single use, and never to a parameter |

Five of the seven were personally reproduced in this session; 6 and 7 carry an agent's repro and
command line and have not been re-run here.

**Four share one class:** a profitability knob silently changed legality. That is the boundary
`JavaScriptCompilationContract` / `JavaScriptOptimizationObjective` exists to hold, and in each case
it was held by a conditional rather than by a type.

---

## Not started

Phases 1–8 ([009](009-phases.md)). Phase 1 cannot begin until Phase 0's remaining items are green —
that is the whole point of [D4](001-directives.md#d4--repair-the-instrument-before-trusting-it), and
five open wrong programs is not a repaired instrument.

Recommended next three, in order:

1. **Finish 0.5.** Bugs 5 and 7 are compile-time/control-lane defects that block trusting any
   measurement. Bug 5 in particular invalidates the ablation control.
2. **0.3b**, extending the differential evaluator to classes, closures and prototypes. Cheap relative
   to its value: it is the only mechanism that could have caught any of the seven.
3. **0.4**, freezing the baseline across all 61 configs on the pool, once F2 lands.

---

## Where the work lives

Branch `migration/target-tree`, in a worktree, isolated from the concurrent session:

| commit | what |
|---|---|
| `fe51558` | Phase 0 — the instrument: profiles, `portgate.mjs`, random differential seed, fleet F1/F3/F4/F5, `fleet-compare.mjs`, config-discovery and `charCodeAt` fixes |
| `5fc2aac` | the naming fix that unblocks the release gate |

---

## Working-tree note

Another session is active in this checkout (HEAD `54e1948`, plus uncommitted work across
`src/ast.rs`, `src/codegen_ir_js.rs`, `src/parser.rs`, `src/lexer.rs`, `src/lower.rs`,
`src/module.rs`, `src/ir.rs`, `src/compress_passes.rs`, `src/js_peephole/**`). Everything landed
above deliberately avoids those files:

- `Cargo.toml`, `src/config.rs`, `src/value_analysis.rs`, `src/bin/lilscript-differential.rs`
- `scripts/verify.sh`, `finer/tools/{portgate,workers,fleet}.mjs`

Bugs 5, 6 and 7 all live in `src/codegen_ir_js.rs` and should be taken up when that file is quiet, or
in a worktree — per [D7](001-directives.md#d7--every-phase-ships).
