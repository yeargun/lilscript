# 002 — Phase 0: repair the instrument

Parent: [index](index.md). Directive: [D4](001-directives.md#d4--repair-the-instrument-before-trusting-it).
Status: **blocking. No migration step may begin until this phase is green.**

---

## The thesis

This migration's entire safety argument is "measure every step against the ports." That argument is
worthless if the measurement can report a false green — and today, almost every gate it would rely
on can.

The proof is not theoretical. Two wrong programs were found by reading the source in a single
session, both reproducible in under a minute, both shipping right now. Neither is caught by any
existing gate. Three more (the fold miscompiles) shipped earlier and were found only by a downstream
library's test suite failing.

**You cannot migrate 47,000 lines of backend with an instrument that cannot detect a wrong program.**
Phase 0 builds the instrument. Everything else waits.

---

## What is broken, verified

Each row was checked directly on 2026-09-04 against `HEAD c9d0d3c`.

| # | Gate you would assume exists | Reality | Evidence |
|---|---|---|---|
| G1 | The release gate proves ports still work | **Runs zero port suites.** 74 stages: benchmarks, solid-client, lilastro, web, vscode-extension. None of the 25 `~/*lil` ports. | `scripts/release-check.sh` |
| G2 | A port's `npm test` tests the compiler | **18 of 25 never rebuild.** They test a stale committed `dist/`, so they validate whatever binary produced it — not yours. | `package.json` `scripts.test` across `~/*lil` |
| G3 | The 7 rebuilding ports cover production config | **mobxlil tests `--dev`.** `npm test` runs `build.mjs --dev`, compiling `config/dev.toml`; its level-13 `lilscript.toml` artifact is never exercised by the suite. | `mobxlil/scripts/build.mjs:105-116` |
| G4 | zodlil's 1,353 tests guard the fold layer | **They run with the fold layer off.** `npm test` → `--compile` → `lilscript.dev.toml` at `optimization_level = 8`; `ParsedPeephole::minimum_level() == 9`, so the peephole never runs. | `zodlil/scripts/build.mjs:82`, `zodlil/lilscript.dev.toml`, `src/config.rs:1497` |
| G5 | `debug_assert!` protects invariants | **Compiled out of every fleet build.** `Cargo.toml` has no `[profile.release]` section, so `debug-assertions` is off in release. All 4 optimizer assertions, and any migration witness built this way, are no-ops where it counts. | `Cargo.toml` |
| G6 | A sweep at the default level covers the ports | **61 config files across 7 levels** (0, 3, 6, 8, 12, 13, 15). 25 files pin 15, 16 pin 13, 11 pin 12. A level-13 sweep misses most of the corpus, and 9 files sit below the peephole threshold entirely. | `~/*lil/*.toml` |
| G7 | Differential testing is generative | **Pinned seed.** `0x6c696c7363726970` is hardcoded and `verify.sh` never overrides it, so the same 64 programs have run on every commit. Its domain also excludes classes, closures and host calls — where every known miscompile lives. | `src/bin/lilscript-differential.rs:21`, `docs/differential-testing.md:26` |
| G8 | A stage that fails is reported | **124 production sites discard errors** via `.ok()?`, `if let Ok`, `let _ =`. A stage that silently no-ops narrows the search invisibly. Four such bugs are already on record. | `src/compiler.rs` (55), `src/codegen_ir_js.rs` (32), others |

Read together: **nothing in this repository routinely proves that a compiler change preserves the
behaviour of the shipped libraries.** That is not a gap in coverage. That is the absence of the gate.

---

## The live wrong programs

**Seven, found by reading and by a targeted hunt, all reproduced.** They are here because they are
the acceptance test for Phase 0: when the instrument is repaired it must catch every one. If a
repaired gate still reports green on these, the repair is incomplete.

| # | Wrong program | Trigger | Verified |
|---|---|---|---|
| [1](#live-1) | `extern` rewritten by source spelling | any of 105 names | reproduced |
| [2](#live-2) | closure `this`/`arguments` rebound by arrow spelling | `function_spelling = "arrow"` **and the default search** | reproduced; port fix landed in 061 |
| [3](#live-3) | `lilscript.toml` silently ignored | a bare relative filename | reproduced; **fixed in this phase** |
| [4](#live-4) | `charCodeAt` out of range yields `NaN`, not `0` | **the default `priority = "size-first"`** | reproduced |
| [5](#live-5) | a module global's binding is deleted while a use renders its name | `[optimization] preset = "none"` | reproduced |
| 6 | `JS.number(x["length"])` loses its `ToNumber` | **the default `priority = "size-first"`** | agent-reproduced |
| 7 | `optional_constructor_callback` fails to compile: "SSA value 3 has no emitted name" | every config except `preset = "none"` | agent-reproduced |

Three of these fire in a **default** configuration, and one of them — #5 — breaks the
optimizer-ablation *control* lane that `scripts/verify-matrix.sh` runs every case through. A control
that miscompiles cannot certify the thing it controls for.

Four share one class: **a profitability knob silently changed legality.** That is exactly the
boundary `JavaScriptCompilationContract` and `JavaScriptOptimizationObjective` exist to hold — *"the
objective can choose among programs admitted by the compilation contract, but cannot alter that
contract"* — and in each case it was held by a conditional rather than by a type.

### Live-1 — the optimizer rewrites user externs by spelling {#live-1}

`src/optimizer.rs` collects every `extern` function keyed on its **source spelling** and matches it
against 105 hardcoded names. `ExternDecl` carries no host identity — only `name` — and
`FunctionKind::Extern` is a unit variant, so spelling is the only identity that reaches the
optimizer.

```lilscript
extern float mathRound(float value);
extern void noop();
export float go(float v) { noop(); return mathRound(v); }
```
```js
// emitted:
function d(n){return Math.round(n)}export{d as go}      // noop() deleted, mathRound → Math.round
```
Rename the externs and the same program compiles correctly. The reserved list includes `noop`,
`stringify`, `apply`, `typeOf`, `createElement`, `getAttribute`, `addEventListener`, `appendChild`,
`isFalse`, `dateNow`, `call0`–`call4`.

This violates [D6](001-directives.md#d6--no-glue-and-no-package-knowledge) and the language
contract's own rule for the adjacent case: *"an unrelated extern with the same spelling has no
special behavior"* (`docs/language-v0.1.md:405-407`). The frontend already resolves intrinsics by
identity and `src/semantic.rs:23-28` explains why — *"prevents later stages from guessing from
identifier spelling"* — and then the optimizer does exactly that.

**Class:** identity recovered from spelling.
**Made impossible by:** an extern that declares its host binding, with host knowledge keyed on the
declared binding. An extern with no declared binding is inert by construction. See
[003](003-target-representation.md).

### Live-2 — a profitability knob silently changes legality {#live-2}

`emits_ordinary_function_expression` (`src/codegen_ir_js.rs:7409`) conjoins the `this`/`arguments`
check with `matches!(function.kind, FunctionKind::Function)`, so a `FunctionKind::Closure` skips it.

```lilscript
extern JsValue arguments;
export JsValue make() {
  auto f = () => { return arguments; };
  return JS.box(f);
}
```

| Config | Emitted | Runtime |
|---|---|---|
| default | `function a(){return Object(function(){return arguments})}` | `OK: [1,2,3]` |
| `function_spelling = "arrow"` | `let b=()=>Object(()=>arguments)` | `ReferenceError: arguments is not defined` |

Same source. A knob documented as a *spelling* choice produced a different program. The language
contract promises the opposite: *"the emitter forces that function to ordinary-function syntax even
when public arrows are requested"* (`docs/language-v0.1.md:415-418`).

This is precisely the boundary `JavaScriptCompilationContract` and `JavaScriptOptimizationObjective`
exist to hold — *"the objective can choose among programs admitted by the compilation contract, but
cannot alter that contract"* (`src/compilation_contract.rs:47-49`). It was held by a conditional,
and the conditional was incomplete.

**Class:** profitability knob changes legality.
**Made impossible by:** `receiver_use: ReceiverUse` as a **required field** on the function node,
with `receiver_use.is_empty()` as a precondition of the arrow spelling — so an arrow that captures
`this` or `arguments` cannot be constructed. See [004](004-legality-by-construction.md).

> Reproduction note: `lilscript.toml` is discovered by walking **up** from the input file. A config
> left in a parent directory silently contaminates sibling tests — this cost one wrong result during
> this session's investigation. Every repro gets its own directory.

---

## What Phase 0 builds

Ordered. Each is small, none depends on the target representation, and all of them are useful
whether or not the migration proceeds.

### 0.1 — Make assertions real

Add to `Cargo.toml`:
```toml
[profile.release]
# debug-assertions stays off: this is the shipped, measured profile.

[profile.release-assert]
inherits = "release"
debug-assertions = true
```
Every witness this migration relies on is either an **`env`-gated runtime check that works in
release** (`LILSCRIPT_TWIN=1`) or runs under `release-assert`. No migration gate may be a bare
`debug_assert!`. The four existing optimizer assertions move to the same footing.

**Exit:** a deliberately broken invariant fails a `release-assert` build and an env-gated release run.

### 0.2 — Make the ports a gate

The corpus is 26 ports, 181 artifacts, 61 config files. Build `finer/tools/portgate.mjs`:

- for each port × config: build **from source** with a named compiler binary, then run the suite
- record per config: artifact SHA-256, Brotli bytes, `emit_ms`/`emit_calls`/`lex_calls`/`codec_calls`,
  wall time, arena high-water, and the **failing-test set** (not a pass/fail bit)
- **fail the run** on any port whose build took under 10 s or produced no `lilscript-timing` line —
  these indicate a stale artifact rather than a pass ([D4](001-directives.md#d4--repair-the-instrument-before-trusting-it))
- isolate binaries per arm and scope `git restore`, because other sessions edit this tree concurrently

Then fix the ports whose own scripts defeat the gate: mobxlil must build its production config for
the suite, zodlil must be tested at the config it ships, and the 18 non-rebuilding ports gain a
`test:compiler` script that rebuilds first. Their existing `test` scripts stay as-is for fast local
iteration.

**Exit:** `portgate.mjs --baseline` records the pre-migration state of all 61 configs as data;
deliberately reverting a known-good compiler commit turns the corresponding ports red.

### 0.3 — Make the differential harness generative

Three changes to `src/bin/lilscript-differential.rs` and `scripts/verify.sh`:

1. randomize the seed in CI and print it on failure — one line, and it starts finding things
2. extend the evaluator's domain to **classes, closures and prototypes**, the domain it explicitly
   rejects today and where every known miscompile lives
3. add the two live bugs above as fixed cases, so they can never silently return

**Exit:** the harness reproduces Live-2 without being told about it.

### 0.4 — Freeze the baseline

Snapshot, at one named commit, on idle pool workers: per config artifact SHA-256, Brotli bytes,
failing-test set, and the resource line. This is the incumbent every later phase is measured
against, and it satisfies rule 2 of the inherited execution rules.

Record explicitly, as data rather than as prose, which ports are **blind** to which defect classes —
zodlil to fold defects at level 8, mobxlil to production-config defects — so a green from them is
never over-read.

**Exit:** the baseline replays byte-identically from its fingerprints on a second worker.

### 0.5 — Fix the two live bugs, under the new gate

Not part of the representation work, and not blocked by it:

- **Live-2** is a one-line fix (drop the `FunctionKind::Function` conjunct) plus a differential case.
  Ship it immediately; it is a wrong program.
- **Live-1** ships its *guard* now — a hard error when a user `extern` collides with a reserved name
  — converting a silent wrong program into a diagnostic. The real fix (a declared host binding)
  belongs to [003](003-target-representation.md) and lands with it.

**Exit:** both reproduce red on the pre-fix binary under 0.2/0.3, and green after.

---

## Cost, and why it is not optional

Phase 0 is roughly one to two weeks of work that produces no bytes and no speed. It will feel like a
detour.

The alternative is migrating the largest subsystem in the compiler while measuring with an
instrument that has already failed five times: three fold miscompiles found by downstream test
suites, and two live wrong programs found by reading. Every later phase in this plan is gated on a
fleet measure. If the fleet measure can report a false green — and
[G1–G8](#what-is-broken-verified) say it can — then every gate in this plan is decoration.

Build the instrument first.
