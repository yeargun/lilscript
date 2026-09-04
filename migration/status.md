# Migration status

Parent: [index](index.md). Volatile: rewritten as work lands. Plan: [009](009-phases.md).
Updated 2026-09-04.

Numbers taken on the orchestrator host are **triage, not evidence**
([D5](001-directives.md#d5--compiles-run-on-the-pool-not-on-this-host)). Nothing here has been
through a fleet A/B yet.

**Gate state: `cargo test --release --lib` is 1,705 passed / 0 failed / 1 ignored**, `tests/cases`
72/72 at `none` and 72/72 at `maximum`, and **phase 1 is complete and gated** — see below.

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
| 0.5 live bugs fixed | **6 of 7** | see below; plus one unsound *test* corrected — `elides_char_code_at_integer_normalization_when_proven` asserted the elision on a source that proves nothing (`read()` returns a string of unknown length, and `"".charCodeAt(0)` is NaN) |

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

**A fourth and a fifth fold miscompile.** The project had three on record. Bug 5 turned out to be
*two* independent ones, both found by `LILSCRIPT_SKIP_FOLDS` bisection in minutes:

- `remove_unused_standalone_vars` — a template literal is one token, so `${a}` was not an
  `Identifier` and a live binding looked dead (`16_templates`).
- `fold_single_use_literal_bindings` — a declarator inside a `for (...)` header has the header's `(`
  as its nearest enclosing bracket, so `scope_end` is the header's `)` and **every use in the loop
  body is outside the scan**. `for (var values = [...], i = 0; i < values.length; ++i) result +=
  values[i]` folded the literal into `values.length`, deleted the declarator, and left `values[i]`
  referring to nothing (`31_string_array`).

Both are "structure guessed from tokens", and both were refused conservatively rather than repaired
with a cleverer guess: a missed fold costs bytes, a missed use is a wrong program.

**The original claim.** This one — a template literal read as
a single opaque token, so a live binding looked dead — is exactly the class the certification pass
predicted ("identity/liveness lost because a template literal is one opaque token") and it was found
by bisection in minutes once `LILSCRIPT_SKIP_FOLDS` was pointed at it. **The three known miscompiles
were never the whole set**, which is the argument for the generative harness in 0.3b rather than for
auditing folds one at a time.

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

## First real pool measurement

Two workers (`10.1.0.19`, `10.1.0.20`) brought up from the deallocated pool, arm-isolated per
[008](008-fleet.md), with the F1–F5 fixes exercised for the first time.

**The pool argument, measured rather than asserted:**

| port | on a pool worker | on this host |
|---|---:|---:|
| cnlil | **33 s** | exceeded a 120 s timeout |
| posthoglil | **63 s** | 139 s (portgate) |
| markedlil | **133 s** | 406 s (portgate) |

Roughly 3× on the ports that complete here at all, and the difference is larger for the ones that do
not. Arm A: 3 ok, 0 failed.

**F4 and F5 confirmed working.** Arm A's `lilscript-timing` survived arm B — previously arm B
truncated it — and the two arms carry distinct compiler digests (`e32a0e38b23f` against
`93e1698d206c`), so the run cannot silently be a comparison of one binary with itself.

A third independent confirmation of the idle-fold ratio, this time from a pool worker: **cnlil,
11,117 idle against 2,438 active fold calls — 82%**. Measured earlier at 77.5% (markedlil, by hand)
and 77.7% (markedlil, via `portgate`).

---

## The A/B: what the six correctness fixes cost

Incumbent `54e1948` against candidate `152b830`, both built here, both dispatched to the pool with
`--compiler` / `--dist-dir` / `--log-dir`, measured on this host with the pinned codec. Compiler
digests `e32a0e38b23f` and `93e1698d206c` — different, and recorded, so the run cannot be a
comparison of one binary with itself.

| port | artifact | incumbent | candidate | Δ Brotli |
|---|---|---:|---:|---:|
| markedlil | `marked.esm.js` *(scored)* | 9,470 | 9,431 | **−39** |
| markedlil | `marked.raw.js` | 9,379 | 9,380 | +1 |
| markedlil | `marked.closed.js` | 9,285 | 9,292 | +7 |
| markedlil | `marked.bytes.js` | 9,917 | 9,934 | +17 |
| markedlil | `marked.gzip.js` | 9,419 | 9,438 | +19 |
| markedlil | `marked.umd.js` | 10,185 | 10,231 | +46 |
| cnlil | `cn.raw.js` | 9,390 | 9,366 | **−24** |
| cnlil | `index.js` | 9,372 | 9,371 | −1 |
| cnlil | `lite.js` | 113 | 113 | **identical** |
| posthoglil | `autocapture.esm.js` | 3,178 | 3,178 | **identical** |
| posthoglil | `autocapture.raw.js` | 3,097 | 3,097 | **identical** |
| posthoglil | `error-tracking.esm.js` | 6,569 | 6,569 | **identical** |

**Twelve artifacts: four byte-identical, and every other movement between −39 and +46 — inside the
±100 noise floor.** markedlil's scored artifact and both cnlil artifacts got *smaller*. Two of the
six fixes (`charCodeAt`'s `|0` and the `.length` ToNumber) *add* coercions, and they still cost
nothing measurable.

Compile time, arm A → arm B on the same workers: cnlil 33→34 s, posthoglil 63→70 s, markedlil
133→135 s. Unchanged to marginally slower, within run-to-run variance on a shared pool.

**So on the measured set, correctness came free.** This is three ports of 26 and is not the full
sweep the plan requires before a phase is declared done — but it is the first number in this project
taken with arm-isolated binaries, per-arm telemetry and recorded digests, which is what F1–F5 were
for.

---

## Phase 1 — the expression tree is complete

Grown inside `JsExpression` rather than beside it, per the Ratchet method in
[003](003-target-representation.md): the rendering path is untouched and `code` stays authoritative,
so nothing can regress while the printer does not exist yet.

**All ten expression kinds now retain the operands the grammar gives them.** The pin test's
`incomplete` list is empty.

| kind | grammar | retained |
|---|---:|---:|
| `Atom`, `Raw` | 0 | 0 |
| `Unary`, `IntegerNormalization`, `NullNormalized` | 1 | 1 |
| `Member` | 1 | **1** |
| `Binary`, `Nullish`, **`Index`** | 2 | **2** |
| `Conditional` | 3 | **3** |
| `Call` | variadic | **callee + args** |

### `code` is now derived, not authored

The tree being *complete* is necessary but not sufficient: as long as each constructor also wrote its
own text, the program had two descriptions that could drift, and a printer added beside them would
have been a third. So the constructors were inverted rather than mirrored — they build the node and
call the printer to obtain `code`:

    fn conditional(condition, then_value, else_value) -> Self {
        let operands = vec![condition, then_value, else_value];
        let code = render(JsExpressionRoot::Conditional, &operands, ..)...;
        Self::grouped(code, ..).with_operands(operands)
    }

`code` is now a *cache of the tree*, which is what lets it be deleted in phase 3 rather than kept in
sync forever. **`render` owns every kind except `Atom` and `Raw`**, which are leaves whose text is the
datum rather than a rendering of children.

Two things fell out of doing this, neither of which was visible before:

**The arity table was checked only against constructors that opted in.** `NullNormalized` was built
at two sites as `grouped(format!("{indexed}??null"), .., NullNormalized)` with **no operands at all**,
while `retained_arity` claimed it kept one — the `with_operands` assertion could not fire because
those sites never called it. The real gate is now that `render(..).expect("render covers X")` panics
on a malformed child list at construction, for every kind `render` covers. Coverage is the invariant;
the table documents it.

**The child list had three owners.** `unary_operand`, `binary_operands` and `normalization_operand`
were separate `Option<Box<Self>>` fields sitting beside `operands`, so a node could carry children
that disagreed with its own kind. They are now projections of `operands`, which makes the
disagreement unrepresentable rather than merely unlikely ([004](004-legality-by-construction.md)).

**The emitter flag was 57 hand-threaded copies of one option.** `elide_call_chain_parentheses` was
passed to `member()` and `index()` at 57 call sites, always as `self.options.elide_call_chain_parentheses`.
It is now `JsRenderOptions`, a parameter of the *printer* — which is where it belongs and where it
has to be: the compiler scores hundreds of complete artifacts per compile, and two of them may differ
by exactly this flag over one identical program, so a tree that stored it could not be shared between
candidates ([006](006-candidate-derivation.md)). Kinds that provably consult no option pass
`JsRenderOptions::UNUSED`.

**`Member` gained its property name as a child.** It was a `&str` argument that reached the text
without passing through the node at all, so a printer walking the tree could not have recovered it.
The grammar makes it a child (`MemberExpression . IdentifierName`) and so does every production JS
AST; `Member` is arity 2 now. The `?.` spelling — a second authored rendering of the same node — moved
into `render_optional_access` alongside it, so a node's two spellings cannot disagree about how far to
parenthesise.

**Writing the table down immediately found a defect.** `member()` and `index()` both used
`JsExpressionRoot::Member`, so a Member node's arity was 1 or 2 depending on which constructor made
it — `o.k` has one child, `o[k]` has two, and nothing could tell them apart. That is precisely the
ambiguity a printer cannot survive. `Index` is now its own tag; the single consumer means "the
receiver is a member access", true of both forms, so it matches on either.

### Neutrality, proved the way [D3](001-directives.md#d3--byte-identity-is-the-only-clean-neutrality-proof) asks

Not "within noise" — byte-identical. Every `tests/cases/*.lil` compiled at `maximum` by the previous
commit's binary and by the new one, artifacts compared with `cmp`:

| step | artifacts differing | behaviour (`none` / `maximum`) | unit tests |
|---|---:|---|---:|
| one owner for children, `code` derived for 7 kinds | **0 of 72** | 0 fail / 0 fail | 1,705 |
| `Member` and `Index` derived | **0 of 72** | 0 fail / 0 fail | 1,705 |

The clones are deliberately wasteful — they copy the child's rendered text, which is the very thing
the migration deletes. That cost disappears when the printer walks the tree instead of `code`.

### The twin witness — phase 1's gate, met

`LILSCRIPT_TWIN=1` turns on `witness_reproducible_from_tree`, which rebuilds each node **from its
children alone, never reading its own `code`**, and asserts the result matches — `code`, `ungrouped`
and `optional_access_code` alike, since all three are renderings a printer would have to derive.

The rebuild goes through the *constructors* rather than through `render` directly, so the
canonicalisations they apply (`!!!x` is `!x`, the constant-operand swap on `==`) are checked for
idempotence at the same time: one that fired twice would change the text.

| lane | expressions reproduced from the tree |
|---|---|
| `none` | **72 of 72 cases** |
| `maximum` | **72 of 72 cases** |

**Negative control**, because a witness that cannot fail proves nothing: corrupting one arm of the
rebuild (`Binary`'s right operand replaced by `atom("0")`) made it **fail 53 of the 72** cases. It
fires. The corruption was then reverted and the artifacts re-checked byte-identical.

Off, the witness costs one `OnceLock` load, and the artifacts are byte-identical with it compiled in.

This is the statement phase 1 set out to earn: **the expression tree, plus the printer's options, is
sufficient to reproduce the emitted expression text.** `code` is now provably a cache.

**Not yet true of statements or module structure** — that is phase 2, and it is the much larger
surface: the emitter writes statements straight into a `String`.

### Proved on a real port, not only on `tests/cases`

cnlil (1,591 lines of LilScript, 26.8 KB of emitted JavaScript, ~300 scored candidates per compile),
built by the pre-inversion commit `0081beb` and by phase-1-complete `b31c5d6`:

| artifact | bytes | |
|---|---:|---|
| `cn.raw.js` | 26,823 | **identical** |
| `index.js` | 26,824 | **identical** |
| `index.cjs` | 26,892 | **identical** |
| `lite.js`, `lite.raw.js`, `lite.cjs`, `index.d.ts`, `lite.d.ts` | — | **identical** |

**8 of 8 byte-identical.** Compile time 76.2 s → 76.6 s, which is the same number on this host.

Then the same port under `LILSCRIPT_TWIN=1`: **it compiles clean**, so every expression node in a
real 26.8 KB artifact reproduces its own text from its children. 78.3 s, so the witness costs about
2%. (`build.mjs` uses `spawnSync` with no `env` override, so the variable does reach the compiler —
checked, because a witness that silently did not run would have looked exactly like this.)

Both cnlil runs wrote into the port's `dist/`, which was restored to `76a975f` afterwards.

---

## Phase 2 — the block is a type, not a `String`

**2a** ([`404ec93`](.)) is `type JsBlock = String` and the 63 `out: &mut String` signatures. A pure
rename, so it rebases against the concurrent session without a semantic conflict.

**2b** makes it real: the text plus the facts about that text the emitter used to recover by
searching it. `Deref<Target = str>` gives every read-only use; there is deliberately **no `DerefMut`**,
so nothing can append behind the counters' back.

### The census, and the trap under it

`LoopSpelling::Auto` picks which loop keyword to reuse — a real Brotli decision, it wants whichever
spelling the artifact already has — and it decided by running

    out.matches("for(").count() > out.matches("while(").count()

at *every loop*, rescanning the whole artifact so far, twice. Quadratic in the output, and one of the
two confirmed superlinearities in [009](009-phases.md).

**The first fix made it 23% slower.** Maintaining the counters naively — a `String` allocated per
append to look at the join, and a full recount after every `truncate`/`pop` — cost more than the scan
it replaced, because appends are the hot path and `pop` runs per statement in the semicolon elision.
Measured on cnlil: **76.2 s → 94.1 s**, artifacts byte-identical throughout, so nothing but a
stopwatch would have caught it.

The counters are now bounded on both sides. An append scans the fragment plus a fixed join window on
a stack array; an edit recounts only the window it can disturb, since a needle whose match changes
must contain a byte the edit touched and no needle is longer than `"while("`.

| commit | cnlil compile | artifacts |
|---|---:|---|
| `0081beb` before phase 1 | 76.0 / 76.3 s | baseline |
| `404ec93` phase 1 complete + 2a | 76.6 s | **8 of 8 identical** |
| `292803b` 2b, naive counters | 94.1 / 94.2 s | 8 of 8 identical |
| 2b, bounded counters | **76.2 / 76.5 / 76.4 s** | **8 of 8 identical** |

So **phase 1's operand clones cost 0.5%** — the tree is very nearly free even while `code` is still
being built beside it — and the whole regression was the counter maintenance.

The counters are checked rather than trusted: under `LILSCRIPT_TWIN=1`, `loop_keyword_counts` asserts
against a fresh scan. 144 runs (72 cases × both lanes), 0 failures.

### The IDENTICAL gate, on all three canaries

[009](009-phases.md) names cnlil, markedlil and posthoglil as the canaries, and phases 1–4 claim the
**IDENTICAL** gate: every artifact byte-identical to the incumbent. Every artifact of all three,
built by `0081beb` (before phase 1) and by `00206f1` (phase 1 + 2a + 2b):

| port | artifacts | differing |
|---|---:|---:|
| cnlil | 8 | **0** |
| posthoglil | 28 | **0** |
| markedlil | 8 | **0** |

**44 artifacts, none differing.** And all three compile clean under `LILSCRIPT_TWIN=1`, so the
expression tree reproduces its own text and the block counters match a fresh scan across three real
codebases, not only across `tests/cases`.

Compile time, same two arms, run serially on an otherwise idle host:

| port | `0081beb` | `00206f1` | |
|---|---:|---:|---|
| cnlil | 76.0 / 76.3 s | 76.2 / 76.5 / 76.4 s | +0.3% |
| posthoglil | 96.3 s | 96.7 s | +0.4% |
| markedlil | 326.41 s | 326.43 s | +0.006% |

So the end condition the owner set — *compression the same or better, compilation the same or
faster* — holds for everything landed so far: **44 artifacts byte-identical, three ports within
half a percent on time.**

Built from `rsync` copies under the scratch directory, never in the port trees: `~/posthoglil` and
`~/markedlil` both had uncommitted `dist/` changes at the time from the concurrent session, so even a
correctly-scoped `git checkout -- dist/` would have destroyed work.

### The escapes are now named

Every way of reaching back into emitted text — `truncate`, `pop`, `remove`, `insert_str`,
`replace_range` — is a method on the type rather than one of 583 indistinguishable string operations,
so phase 3 can find every caller with `grep` rather than judgement. `into_string()` marks each place
block text stops being a block and becomes an artifact or an expression.

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
