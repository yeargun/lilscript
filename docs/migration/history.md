# Migration history

What each batch of the one-compiler migration landed, measured and left open, oldest first. You do not need this file to pick up the next task: [index.md](index.md) says where the migration stands and [plan.md](plan.md) says what each task is. Come here for a batch's evidence, its reasons, or what it left open.

Before 2026-09-23 (milestones 001–013, their receipts and the Closure ADVANCED inventory 013-T7): [record-2026-09.md](record-2026-09.md). Owner briefs: `finer/intent/`.

**Adding a record.** A batch appends its section at the end before its first build, headed `## <date> <batch>: <what it does>`, with its pre-registration (plan rule 5). When it lands, the section gets three parts: what landed; evidence (binary and SHA-256, the runners, sizes, compile time, runtime); and what stays open, each item with its owner task. The same commit adds its row to the index below and ticks its tasks in [index.md](index.md).

## Index

| Date | Record | Tasks | Branch |
|---|---|---|---|
| 2026-09-23 | [Where we started](#2026-09-23-where-we-started) | — | — |
| 2026-09-23 | [M0: record and freeze](#2026-09-23-m0-record-and-freeze) | M0 | `one-compiler` |
| 2026-09-24 | [M1: one compiler](#2026-09-24-m1-one-compiler) | M1.1–M1.9, M2.2, M2.6 | `one-compiler` |
| 2026-09-24 | [Batch I: effects and initialization order](#2026-09-24-batch-i-effects-and-initialization-order) | M6.1–M6.3, M7.2, M6.5, M4.3 (part) | `m6-effects`, `m6-init` |
| 2026-09-24 | [Green CI and the interim release](#2026-09-24-green-ci-and-the-interim-release) | M2.1, M2.8, M2.9 | `one-compiler-m2` |
| 2026-09-24 | [Batch T: the terminal challenger stage](#2026-09-24-batch-t-the-terminal-challenger-stage) | M5.4, M9.2, M9.3 (first family) | `m5-terminal` |
| 2026-09-27 | [Batch D: choices and data tables](#2026-09-27-batch-d-choices-and-data-tables) | M9.1 (first slice), M9.8, L2 spellings | `m9-data` |
| 2026-09-27 | [Batch N: nominal identity](#2026-09-27-batch-n-nominal-identity) | M4.1 | `m4-nominal` |
| 2026-09-27 | [katexlil released with its font metrics in LilScript](#2026-09-27-katexlil-released-with-its-font-metrics-in-lilscript) | step 1 | katexlil `07d90d6` |
| 2026-09-27 | [Design version 1 and the plan's revision](#2026-09-27-design-version-1-and-the-plans-revision) | plan revision | `design-final` |
| 2026-09-27 | [M3.3a: the delivery contract, first slice](#2026-09-27-m33a-the-delivery-contract-first-slice) | M3.3a | `m3-delivery` |
| 2026-09-27 | [Batch A1: canonical formation without tree analysis](#2026-09-27-batch-a1-canonical-formation-without-tree-analysis) | M8.2 A1, M8.1 (imports), M2.3 (`print`), M9.5 (seed), M11.2 | `m8-a1` |
| 2026-09-27 | [The tools, first part: M2.13, M2.5 and M2.14](#2026-09-27-the-tools-first-part-m213-m25-and-m214) | M2.13, M2.5, M2.14 | `m2-tools` |
| 2026-09-28 | [Owner decisions: counted budgets, decide statically first, verify per batch](#2026-09-28-owner-decisions-counted-budgets-decide-statically-first-verify-per-batch) | AM1, L22, plan rules 3 and 8 | — |
| 2026-09-28 | [Batch F1: the floor slice, first part](#2026-09-28-batch-f1-the-floor-slice-first-part-edit-kernel-dce-exact-values-folding) | M5.1, M6.4a, M7.8a | `m5-floor` |
| 2026-09-28 | [Batch F2: the floor slice, second part](#2026-09-28-batch-f2-the-floor-slice-second-part-graft-removal-only-inlining-typed-defaults) | M5.1, M7.5a, M7.3 | `m5-floor` |
| 2026-09-28 | [Batch F3: the floor slice, third part](#2026-09-28-batch-f3-the-floor-slice-third-part-parameters-and-returns-the-tree-twins) | M7.3, M5.1, M5.3b | `m5-floor` |
| 2026-09-28 | [Batch B1: the counted walk](#2026-09-28-batch-b1-the-counted-walk-m35-first-part) | M3.5 | `m3-budget` |
| 2026-09-28 | [Batch B1b: the upper levels try more](#2026-09-28-batch-b1b-the-upper-levels-try-more-m35-am2) | M3.5, AM2 | `m3-budget` |
| 2026-09-28 | [Batch B2: level 0 without a codec, the replay check, the audit lane, the counts](#2026-09-28-batch-b2-level-0-without-a-codec-the-replay-check-the-audit-lane-the-counts-m35) | M3.5 | `m3-budget` |
| 2026-09-28 | [Batch B3: the objective and effort axes](#2026-09-28-batch-b3-the-objective-and-effort-axes-m31-first-slice-m35-codec-settings) | M3.1, M3.5 | `m3-budget` |
| 2026-09-28 | [Batch S1: the dynamic type's syntax](#2026-09-28-batch-s1-the-dynamic-types-syntax-m42-m102-first-part) | M4.2, M10.2 | `language-slice-1` |
| 2026-09-28 | [Owner decisions: compile time scales, test on a clock, track each change](#2026-09-28-owner-decisions-compile-time-scales-test-on-a-clock-track-each-change) | AM3, plan rules 3, 5 and 8 | — |
| 2026-09-28 | [Batch S2: receivers and rest parameters](#2026-09-28-batch-s2-receivers-and-rest-parameters-m104-first-part) | M10.4 | `language-slice-1` |
| 2026-09-28 | [Batch S3: the dynamic type's tests and conversions](#2026-09-28-batch-s3-the-dynamic-types-tests-and-conversions-m102-second-part) | M10.2 | `language-slice-1` |
| 2026-09-28 | [Batch S4: iteration and spread](#2026-09-28-batch-s4-iteration-and-spread-m1016-m104-second-part) | M10.16, M10.4 | `language-slice-1` |

---

## 2026-09-23 Where we started

**Two compilers share one binary.** `[compiler] backend` or `--backend` selects between them.

**The old route: `compiler.rs` → `lower.rs` → `ir.rs` → `optimizer.rs` → `codegen_ir_js.rs` → `js_peephole/`, plus `codegen_native.rs`.**
- It is about 149.5K lines with tests, 42% of `src/`, and holds 1,555 of the 3,092 unit tests.
- It re-emits the whole program 267–381 times per build and reasons over its own printed text.
- Its typed interprocedural optimizations are still ahead on small closed programs:

  | Measure (binary b80) | Old route | Remaining compiler |
  |---|---|---|
  | 72 census cases, Brotli, `strip_console=false` | 5,866 | 8,576 |
  | `comparison/cases`, Brotli wins/ties/losses against the smallest competitor | 53/1/0 | 9/6/38 (+1 refusal, 1 wrong output) |
  | `comparison/algorithms`, Brotli total | 2,305 | 3,345 |

- Its native C works under GCC 13 but not under Clang ≥ 16 (an undeclared `strdup`). It supports native `Record`/JSON and a user-facing C extern ABI, which the remaining compiler refuses.

**The remaining compiler: `compiler_service` → `semantic_program` → `structured_js`, plus `semantic_program/native*`.**
- It is 3–12× faster at equal effort with the search off (katexlil 1.0 s against 4.3 s), and about 100× cheaper per candidate.
- It has zero census miscompiles, and its native C is clean under ASan, UBSan and LSan.
- On the 13 goal boundaries (the six reference ports and the react-markdown family), patched scratch builds beat the strongest pinned bar on 11:
  - katexlil, markedlil, posthoglil and jquerylil beat theirs on both objectives. katexlil's Brotli margin, −374, is inside D4's strict-win threshold of 630.
  - The react-markdown family beats all seven bars (react-markdownlil browser: 27,248 against 31,082).
  - These numbers come from scratch builds with `finer/port-migrations/*.patch` applied. micromark's shipped file can be an esbuild reprint.
- **Open losses:**
  - motionlil: +8,612 Brotli. Its 49,644 is esbuild plus Terser over our output, not a compiler-written file.
  - zodlil's package: +16,396 Brotli as compiled, or +12,211 after esbuild's whitespace minifier.
  - zodlil's core: about 1.5–1.7K above the upstream slice.
- Its bytes come mostly from about 40 hand-ordered rewrites of the finished JavaScript tree. Typed facts barely reach them: every user call is an unknown effect, class field identity is erased at elaboration, and native gets none of these optimizations.
- **The migration's test harvest found real miscompiles here** (M1.9).

**Routing and ports.**
- The default was flipped to the remaining compiler on 2026-09-23 (`d362338f`). These still run the old route regardless:
  - the LSP, the playground and lint (on the old IR);
  - `--write-lock` effect summaries and `--print-dependencies`' module discovery;
  - every `lib.rs` compile function.
- No port pins a route. Committed port `dist/` files were last rebuilt 2026-08-29 to 09-04, all on the old route, and the Pages sites show them.

**Configuration.**
- About 55 configuration keys are consumed only by the old route and about 12 more are translated but read by nothing. Only three of them warn.
- Some port configs will be refused by an honest resolver: mobxlil's `priority = "realistic-performance-first"`, and cnlil's `public_aggregate_abi = "positional"`.

**Status.** CI has not been green since 2026-08-25. 0 of 14 old milestones are verified.

---

## 2026-09-23 M0: record and freeze

- Batches 29–32 and the default flip committed (`d362338f`).
- The owner briefs recorded verbatim.
- The architecture written; `docs/compiler-design.md` became a pointer to it.
- The plan rewritten; the history moved to `record-2026-09.md`.
- The reference binary frozen with its SHA-256; the pre-M1 binary preserved.
- Migration work happens on branch `one-compiler`, in worktrees under `~/lilscript-work/wt/`, so parallel sessions on the main tree are not disturbed.

---

## 2026-09-24 M1: one compiler

**There is one compiler.** Branch `one-compiler`, merged into `finer/059-idiom-directed-naming`.

**What M1 removed and changed.**
- The old route is deleted: about 157K lines, including its 1,555 tests. The test-only tree experiment, the fixed two-file resource cut and interaction pairs are also deleted: about 18.7K lines.
- `--backend` and `[compiler] backend` are gone.
- Modules are named by role: `check`, `program`, `js`, `build`; `compile_source` and `compile_path` are the entry points.
- The LSP, playground and lint compile through the one compiler.
- 61 retired configuration keys warn ("no effect") or refuse, from one table applied before parsing.
- Old-pipeline docs moved to `docs/knowledge/history/`, and `docs/current-status.md` is rewritten.

**Correctness and coverage.**
- Four wrong programs were fixed, each with regression coverage: an async body inlined as its return value; field defaults created before constructor arguments; `==` on a `JsValue` lowered to `===`; and a raw-objective forwarding into an assignment target.
- 288 regression cases were harvested from the old route's tests.
- Two runners with expected-failure ledgers (`docs/testing.md`) replace the old census and port scripts.

**Evidence (binary `m1-merged`, then the M1.7 rename, which changed no artifact).**
- **Unit tests:** 1,462 pass; the binaries' tests pass.
- **Case runner:** 361 cases × 18 lanes, with no failure outside the ledger. Against the pre-M1 binary, no lane's size total grew; the production Brotli module lane went from 31,056 to 30,833.
- **Port suites:** all green (markedlil, zodlil, katexlil, jquerylil, posthoglil, motionlil, micromarklil). A pre-deletion fleet run found no compiler-caused failure in 26 ports; five known failures are ledgered with owners.

**Carried into later phases.**
- The M1.9 coverage gaps: struct values passed to an `extern`, generic struct transport, and the adapter-name spec question.
- Four production bugs found by porting the experiment's tests, now ignored tests with reasons:
  - `??=` on a place;
  - generic methods;
  - an explicit `JS.undefined()` argument still takes the default;
  - a detached `charCodeAt`.

  They are owned by M4 and M10.

**The M1 tasks as planned.**

| Task | Content |
|---|---|
| M1.1 Move shared pieces | Four pieces move and one import is re-pointed:<ul><li>The manifest moves to the JS delivery code; each file is measured once with the canonical codec.</li><li>Diagnostics move to `diagnostics.rs`.</li><li>The codec helpers move to `compression.rs`.</li><li>`reference_parameter_span` moves into the checker, so the interpreter no longer calls `lower.rs`.</li><li>`typed_array.rs` imports `primitive::Intrinsic`.</li></ul>Tactic defaults move into the `TacticId` specs. There is one contract builder, and `public_function_spelling` is deleted: it is always `None` on the one compiler. Output is byte-identical |
| M1.2 Tools | <ul><li>The LSP uses a check-only session.</li><li>The playground compiles with `strip_console = false` and gets a smoke test on its printed output.</li><li>Lint's AST rules move onto the module graph and checker the build uses; its IR rules move onto the Program IR. The IR rules now see the program *before* program rules, and move after M7 when those land.</li><li>`performance/aggregate-escape` returns with the escape fact (M6.6).</li><li>`LintRuleContext` exposes the program (a breaking change, accepted).</li><li>`--write-lock` stops writing effect summaries.</li></ul> |
| M1.3 CLI | <ul><li>`--backend` and its dispatch are deleted; a hidden flag fails with "there is one compiler". `--profile-template` is deleted.</li><li>`--explain human` is readable.</li><li>One target-to-request function serves both `--print-policy` and the build.</li><li>`--target all` writes every chunk.</li><li>Native uses one strict driver.</li><li>`--print-dependencies` moves onto the one compiler's discovery, host modules included.</li><li>Lilpack and Vite keep `--mode`, `--delegate-bundling` and `-o`.</li><li>The repository tools that pass `--backend` (`finer/tools/semantic-census.mjs`, `semantic-port-tests.mjs`, `semantic-service-baseline.mjs`) stop doing so.</li></ul> |
| M1.4 Configuration | <ul><li>`[compiler] backend` is refused.</li><li>One retired-key table is applied before strict deserialization. Each old-route-only or inert key gets a printed "no effect in this compiler" warning or a refusal.</li><li>`priority ≠ size-first`, `[policy.constraints]`, `public_aggregate_abi = "positional"` and `for_of_specialize_family` are refused at load with actionable messages.</li><li>The old option derivations, the second contract builder, the legacy ABI manifest and contract fields with no reader are deleted.</li><li>`schema.md` is regenerated and `configuration.md` updated.</li></ul> |
| M1.5 Tests | <ul><li>**Harvested:** 288 regression cases from the old route's executing tests (`tests/cases/regressions/`, with per-prefix manifests: irjs 202, compiler 65, opt 21). Each carries expected stdout derived from the test's own assertions, never from a compiler.</li><li>**Re-pointed:** the cross-route tests (`structured_js/tests.rs`, `semantic/narrowing_input_tests.rs`, `semantic.rs`, `compilation_facts_tests.rs`, `parser_admission_tests.rs:205`) go to the one compiler or the interpreter; the interpreter's `for_of_family` test is deleted.</li><li>**Mined:** `migration/target-tree` for live-bug cases, then retired.</li><li>**Kept:** keyword-spacing cases, ported to the `structured_js` printer.</li><li>**Rewritten:** `comparison/cases/.../host/math-max`, with a declared host binding.</li></ul> |
| M1.6 Delete | <ul><li>The old route: `compiler.rs` (after M1.1), `lower.rs`, `ir.rs`, `optimizer.rs`, `value_analysis.rs`, `compress_passes.rs`, `codegen_ir_js.rs`, `codegen_js.rs`, `codegen_native.rs`, `js_peephole/`, `decision_registry.rs`, `artifact_memo.rs`, `profile.rs`, `for_of_family.rs`, `compiler_rest_capture_tests.rs`.</li><li>`js_externs.rs`, whose platform-name data moves to a neutral data module.</li><li>The module linker and the AST's linker fields, with `analyze_program`'s linked-program path edited.</li><li>Package effect summaries, `timing.rs`'s old buckets and the pass-ablation benchmarks.</li><li>The test-only annotated-tree experiment in `structured_js`, including `selection::select`.</li><li>The fixed two-file resource cut.</li><li>`search_pairs.rs`, off by default, and the `LILSCRIPT_DEBUG_HELPERS` read in the search loop.</li><li>`examples/structured-slice.rs`, and the checker's dead `EscapeState`.</li></ul>**Kept:** `interpreter.rs`, `primitive.rs`, `typed_array.rs`, `literal.rs`, `js_string.rs`, `js_regex.rs`, `js_syntax_target.rs`, `compression.rs`, `stable_hash.rs`, `scalar_transfer.rs` |
| M1.7 Names and docs | <ul><li>**Renamed by role:** `semantic.rs` and `semantic/` → `check/`; `semantic_program/` → `program/`; `javascript*.rs` and `structured_js/` → `js/`; `native*.rs` → `native/`; `compiler_service` → `build`.</li><li>The rename lands as one announced commit when no session holds uncommitted changes to the renamed files.</li><li>**Docs:** every doc a grep finds describing the old pipeline is rewritten or retired. That includes `language-v0.1.md`, `configuration.md`, `current-status.md`, `optimization-coverage.md`, `modules-and-delivery.md`, `web-platform.md`, `differential-testing.md`, `knowledge/compilation/*`, `knowledge/verification/config-matrix.md`, `finer/objective.md` and `finer/status.md`.</li><li>`language-v0.1.md` states which clauses wait for M10: `inline for`, `@pool`, and string semantics (UTF-16 code units).</li></ul> |
| M1.8 Verify and gate the deletion | Before M1.6, **every maintained port (27) builds and runs its suites on the pre-M1 one-compiler route**. Each failure becomes an owned entry in the expected-failure ledger (M2.6). After M1.6, the same corpus is rerun on the new binary, and byte identity is compared against the pre-M1 binary: census × 3 lanes, `comparison/cases` × 3 codecs, probelil and the 13 goal boundaries |
| M1.9 Correctness debts found by the harvest | Fixed as bugs (D3), with the harvested cases as their reproductions:<ul><li>`async int f(){return 1;}` is folded to `1`, breaking `.then`;</li><li>a field default (`new Map`) is evaluated before a constructor argument (D3.4 order);</li><li>`JsValue == 0` is lowered to `===`, while the language spec says dynamic equality coerces. The fix follows the spec until M10.9 changes it by owner ruling;</li><li>struct values passed to an `extern` are refused;</li><li>a wrapper's inferred name disagrees with the spec, in one of two sibling cases;</li><li>under the raw objective, a module whose inlined callback assigns a root binding fails inside the compiler with "assignment requires a reference" (`compiler-drops_unread_scheduler_store_and_host_value`, found by the case runner).</li></ul>The first three are fixed (`93967ef0`, with a service-level regression test) |

**Exit, checked by script.**
- A grep over `src/`, the CLI help, configuration and current docs finds no route-selecting `backend`, and no "legacy route" or "semantic route" outside history files.
- Every binary (`lilscript`, `-lsp`, `-lint`, `-fmt`, `-codec`, `-differential`, `-playground`, `lilpack`) builds and uses the one compiler.
- `cargo test` passes on all targets.
- M1.8's corpus is byte-identical to the pre-M1 binary, or every difference is explained.
- The expected-failure ledger lists every red gate with its owner.

---

## 2026-09-24 Batch I: effects and initialization order

*Written on 2026-09-27 from the messages of commits `ff27361f` (effects) and `94354617` (initialization order); the plan had no record of this batch.*

**What landed.**
- **Derived views** (`src/program/views.rs`). A `Fact<T>` is known with its dependencies, unknown for a reason, or truncated by a bound. The program caches its views, and a cached view is checked against its dependencies before it is served.
- **The call graph (M6.1**, `src/program/call_graph.rs`). Per call, the body it runs; per body, whether its address is taken, its complete call set when one exists, and Tarjan components callees first, with array callbacks as edges.
- **Effects (M6.2**, `src/program/effects.rs`, the one owner). A per-operation transfer and per-unit summaries computed bottom-up over the components: reads and writes by region, parameter mutation mapped to each caller's arguments, throws (unless a `try` catches them), divergence, user code, fresh identity and suspension. D3.6 as settled: a call is removable only with a termination proof.
- **The `pure` contract (M6.3).** A declared `pure` body whose summary writes outside its own cells and allocations, mutates a parameter, touches the host or runs unknown code is a check-phase diagnostic at its declaration.
- **Discarded effect-free calls (M7.2).** Demand reads the summary: a discarded call whose callee has no observable effect, cannot throw, terminates and has its obligations discharged is not a root, and DCE removes it. Deleted: `facts.rs`'s own per-operation classification, demand's `required`, `helper_family`'s `merge_memory`.
- **Initialization order (M6.5**, `src/program/initialization.rs`). Per module-level cell, the root statement that settles it; per unit, the first root statement during which it may run; per access, whether the storage is initialized there. `effects.rs`, `demand.rs` and `facts.rs` ask the owner, and their own rules are deleted.
- **M4.3, the part M6.5 needs.** The checker's `assigned` is split into `reassigned` and `observable_before_initialization`, and each occurrence records a `ReadInitialization`.

**Evidence.**
- Effects: `cargo test --release --lib` 1,481 pass; the case runner 369 × 18 with no failure outside the ledger and no lane grown; the port suites byte-identical to `release-m1`.
- Initialization order (binary `~/lilscript-work/bin/init-1`): 1,493 pass (9 ignored); the case runner 374 × 18 with no failure outside the ledger, production module Brotli 31,470 → 31,439; the ports' failing sets identical to `effects-3`; `zod.core.js` −44 and `katex.raw.js` −51 Brotli; katexlil's level-13 wall time 7.6 s against 8.5 s; in zodlil all 116 reads of its 41 kind constants inside functions are proven initialized.

**Open.**
- The tree's re-derivations (`quiet.rs`, `inline.rs`, `js/mod.rs`, `root_constants.rs`) stay until M5.2. Rule 2 was waived for this batch, so M6.1–M6.3 and M6.5 count as *computed, not consumed* until M5.2 deletes them (design version 1, "Facts landed without consumers").
- The representation families' exact-initializer proofs (helper, record, product; `native_plan`) stay, owned by M9 and M11.

---

## 2026-09-24 Green CI and the interim release

*Written on 2026-09-27 from commit `335bcf15` and the release report; the plan kept only the phase states for these.*

- **M2.1, green CI** (`335bcf15`). One Linux job gates every push: `cargo fmt`, the doc link graph, the release build, the library and binary tests, the codec contract, the runner tests, and the case runner in 18 lanes against the expected-failure ledger. Publishing (web catalog, VS Code packaging, Playwright, Closure) stays in `scripts/release-check.sh`, run on demand.
- **M2.8 and M2.9, the interim release** ([report](../reports/2026-09-24-release.md)). Twelve of the thirteen goal ports were rebuilt by one pinned post-M1 compiler with no post-minifier and released with their Pages sites. Their headline files went from 288,827 to 247,172 Brotli (−14.4%) against the last release. 11 of 12 win on Brotli against Terser, Oxc/Rolldown, esbuild and upstream's minified file; katexlil was 18 bytes above `katex.min.js` and tied its Terser bar. The port rewrites moved from `finer/port-migrations/*.patch` into each port's repository (M12.1, brought forward). motionlil was released on 2026-09-27 on its newer origin; its delivered files are still esbuild plus Terser over the compiler's output (M12.2).

---

## 2026-09-24 Batch T: the terminal challenger stage

**What landed.**
- **The terminal challenger stage (M5.4).** After the search picks each objective's winner, `program/search_terminal.rs` offers it a declared, ordered schedule of challengers (`js::Challenger`). Each one is formed from the winner's candidate, rendered with the winner's naming style, source names and literal mode, admitted by the same verifier and policy admission, and scored by the objective's exact codec. It is kept only when the complete artifact shrinks. The budget is the number of challengers the codec judges; it comes from the effort ladder (`terminal_challengers` in the policy receipt: 0 through level 7, then 3, 4, 5, 7 at 13, 9, and all from 15 or with `candidate_search = "always"`). The build report and `--explain` list every challenger with its outcome and byte delta. Formation reuses the candidate's demand plan and a formed head (everything before the families) for every challenger.
- **The objective as a judge (M9.2).** `OutputTactics.raw_structure` is gone. `OutputTactics.families` (`js::OutputFamilies`) holds block inlining, flat blocks, the statement spellings, string pooling, loop heads and logical statements. The objective only seeds them, and the stage offers the other alternatives: the raw families under gzip and Brotli, the codec seed under raw, the naming plan's raw spelling both ways, and each objective's whole other seed. Print decisions stay on the tree.
- **The first M9.3 family.** `compress_statements` is split. Same-exit merge, trailing-statement dedup and exit to `break` run under every objective. Conditional values, exit points, loop fusion, conditional returns and logical branches are five spelling groups, each a challenger.

**Evidence (binary `d2188075`, then a change to refusal handling only).**
- **Unit tests:** 1,474 pass. Two deep-tree tests overflow the default 2 MiB debug test stack on this host, on the base commit too; they pass with `RUST_MIN_STACK=16777216` and in release.
- **Case runner:** no failure outside the ledger, and formation-only lanes are byte-identical. Production lane totals fell: Brotli −498 and −706 (script, module), gzip −481 and −861, raw −95 and −186. Two case-lanes grew by 4 gzip bytes (`loop_carried_early_return`, exit to `break`).
- **Reference ports, main entries, against `release-m1`:**
  - Brotli objective: every port smaller, −517 in total. markedlil −18, zodlil −42, katexlil −75, jquerylil −44, posthoglil −54, motionlil −255, micromarklil −29.
  - Raw objective: none larger, −1,280 in total. motionlil −1,030 (block inlining off), posthoglil −161 (raw spelling off).
  - Gzip objective: every port smaller, −755 in total.
- **Port suites** (`scripts/ports.mjs`, with main's `--patches none` for the five ports whose repositories carry their rewrite): green at the shipped objective and under `--objective brotli` and `--objective raw`, the same as `release-m1`. The one exception, identical with `release-m1`, is katexlil's browser-performance test: its site build calls `git ls-files` in a copy without `.git`.
- **Delivered `dist/` files, per objective, against `release-m1`:** every file the compiler wrote shrank or held. Brotli: −1,650 over the seven ports. Raw: −6,026. Files the ports derive from ours moved either way: a banner prepended (markedlil's `marked.esm.js`, +2), esbuild's `.cjs`, `.min.js` and bundles (motionlil's `index.cjs` +133 while its `full.js` fell 201). zodlil's esbuild `index.cjs` grew 52 Brotli while `zod.core.js` fell 42, so zodlil's `dist/` total is +10 Brotli.
- **Compile time (level 13, three alternating pairs, main entries):** markedlil 0.80→1.14 s, zodlil 0.80→1.96 s, katexlil 3.84→8.74 s, jquerylil 30.6→32.2 s, posthoglil 0.12→0.39 s, motionlil 38.8→44.0 s and micromarklil 0.63→2.04 s. The cost is exact codec scores. It is open against rule 3's phase-end gate; the bounded codec pool (architecture §9) and estimator-ranked finalists (M9.4) are where it is recovered.

**Open.**
- Monotone selection holds within a search and within the stage (tests in `search_terminal_tests.rs`), and every level is at most the search-off result. Between two search-enabled levels whose search winners differ, it is not guaranteed. Beam width, retained capacity and the effort-gated tactics shape the trajectory. Replaying the lower level's schedule as the higher one's first phase closes this, with M9.10's recalibrated ladder.
- `pool_strings` and `pack_string_arrays` still ignore the `string-pooling` and `string-array-packing` permissions, and packing's startup risk, as before this batch.

---

## 2026-09-27 Batch D: choices and data tables

**What landed.**
- **The choice kernel (M9.1, first slice; `js/choices.rs`).** A `ChoiceKey` names one site of one family by a stable identity (for data, the root binding a table initializes, never an expression id formation renumbers); an `AltId` names an alternative; a `ChoiceMap` is an immutable, sorted, shared assignment on the artifact's output assignment (`OutputTactics.choices`, next to the families), so every decision is data on the tree. Formation records each site it found (`Module.choice_sites`): its alternatives with the estimator's raw saving, its seed and what it applied.
- **Choices in the terminal stage.** Before the family challengers, the stage surveys the search winner's tree once and offers every site's other alternatives, largest estimated stake first, each formed, admitted and scored like a challenger and kept only on a strict whole-artifact win. When two or more sites apply an encoding, the first move puts every site back to its literal at once: decoders share text a codec matches across tables, so one site at a time cannot leave a state where several pay together (the families' `other-objective-seed` is the same whole move). The choices have their own budget, `terminal_choices` in the receipt (the challengers' ladder, counted apart so a table never displaces a family challenger calibrated on its own schedule). `--explain` lists every choice trial and each delivered site with its seed.
- **Data tables (M9.8; `js/tables.rs`).** Any root `let` whose value is a tree of constant data is a site: the literal; the front-coded string table (now counting UTF-16 units, so an astral prefix decodes exactly); **columns** (a schema derived from the table: collections become key, count and item columns; arrays of one length and objects of one key sequence are transposed; array-index keys are delta-coded; a number column whose values are all `n/10^p` is spelled as those integers; strings are joined; values of mixed kinds keep their literals in a column) with a decoder specialized to the schema; and a **value dictionary** over the columns. `MIN_ENTRIES`/`MAX_SHARE` are gone: the seed is the largest positive raw estimate, else the literal, and every objective can keep any alternative. Exactness is the legality: the same insertion order (index keys enumerate first either way), `Object.is` values, prototypes and plain data properties; no `__proto__` or duplicate key, no non-finite number, no observed literal; module root and pristine builtins only. Every encoding is decoded in Rust with the decoder's semantics and compared with the literal before it is offered.
- **L2 spellings.** A string key that is a canonical integer of at most 2^53 − 1 prints as a number in object literals, member accesses (`o[32]`, and `o[-1]` where a sign is legal) and a dynamic import's members (Closure's `isSimpleNumber` set). The lexer reads `.5`, `.25e-2`, `1e3`, `15e-5` (floats) and `0x1F` (an int under the signed 32-bit rule).
- Prior art (rule 7): Closure `closure-compiler@0da58e1 CodeGenerator.java:1596-1622` and `AliasStrings.java`; Terser `terser@8fa44c8 lib/output.js:2245`, `lib/compress/index.js:3580`, `lib/compress/common.js:170` (`best_of_expression`, a local printed-size judge); esbuild `esbuild@f6058f8 internal/js_ast/js_ast_helpers.go:2684`, `js_parser.go:14605,14993`; Oxc `oxc@591966d oxc_minifier/src/peephole/substitute_alternate_syntax.rs:1389`, `convert_to_dotted_properties.rs:17`; old route `d362338f:src/decision_registry.rs` (whole-program flags, no sites). **No competitor re-encodes constant data**, and none has a codec to judge an encoding.

**Evidence (binary `data-7`, `6d307b5f…`, against `merge-i`).**
- **Unit tests:** 1,519 pass (lib), the binaries' tests pass. New: `js/tables_tests.rs` forms six table shapes under every alternative and compares Node's observations with the literal's own run; `build_terminal_tests` delivers `tests/cases/data_tables` under all three objectives; the choice schedule and the numeric-key spelling have their own tests.
- **Case runner:** no failure outside the ledger. The one compared artifact that changed is the numeric-key case (−9 Brotli per production lane); every lane total fell or held. The new `data_tables` case (Node's own run as its oracle) delivers a different assignment under each objective, against `merge-i`: Brotli 2,243 → 2,108, gzip 3,142 → 2,682, raw 12,607 → 7,975.
- **Port suites** (13 ports, `--patches none`, plus motionlil from its release commit `0eeca0c`): failing sets identical to `merge-i` in every port. motionlil's current head (`69cd2cf`) fails to build on both binaries (a duplicate type declaration, M4.1); the four family site tests that compare committed receipts fail on both.
- **Sizes, `dist/` against `merge-i`:** markedlil, zodlil, posthoglil, motionlil, unifiedlil, mdast-util-to-hastlil and remark-rehypelil byte-identical. katexlil's compiled `katex.raw.js` +26 Brotli (stitched `katex.esm.js` +11), jquerylil `jquery.raw.js` +3, micromarklil `micromark.closed.js` −21 and `micromark.raw.js` −6, react-markdownlil `react-markdown.raw.js` +29 (`closed.js` −46), from-markdown and remark-parse ±25. Over all 13 ports' JavaScript files: +387 Brotli of 1,843,355 (+0.02%), inside the fleet noise law (appendix B). Decomposed on katexlil: the numeric-key spelling alone lands +26 (same search recipe, a different family path); on jquerylil it moves the search to another recipe whose winner is 2 bytes smaller and whose terminal path ends 3 larger.
- **katexlil with its font metrics in LilScript** (patch `~/lilscript-work/portwork/katex-data/katexlil-font-metrics-in-lilscript.patch`, no stitching; scratch build): `katex.esm.js` 60,281 Brotli, 72,187 gzip, 261,313 raw, against the shipped 62,704 / 75,914 / 274,680: −2,423 Brotli. Against the bars: Terser 63,044 (−2,763, −4.4%: a win under D4) and the CDN's `katex.min.js` 62,686 (−2,405, −3.8%: a win, where it was a tie). The table seeds its dictionary (raw estimate −19,855) and the codec keeps columns of ×1e5 integers with delta keys (−930 more); every shipped JavaScript file of `dist/` falls, −3,077 Brotli over 25. `npm test`: 21/21 node:test (browser parity 30/30) and the official Jest suite 1,230/1,230.
- **Compile time (level 13, three alternating pairs, main entries, medians):** markedlil 1.15→1.20 s, zodlil 2.07→2.01 s, katexlil 10.4→15.6 s, jquerylil 34.3→30.8 s, posthoglil 0.37→0.37 s, motionlil 16.4→16.4 s, micromarklil 2.04→2.27 s; katexlil with its data 12.1→19.0 s. The cost is the choice trials' exact codec scores (katexlil offers 13 alternatives on six sites and its configuration asks for every one).

**Open.**
- The estimator is raw bytes under every objective. A small site's seed moves the search like any tree rewrite on every emission (the terminal challenger law), so ports without a large table move by tens of bytes either way. A per-objective estimator (M9.4) seeds codec objectives better.
- Joint moves beyond "every site canonical" are not explored; decoders are declared at the root's start in the first module, as the string tables were (code splitting places them per chunk in M3.3); decoders become LilScript prelude code in M8.5.
- The numeric-key spelling is a rule, as the owner asked, not a codec choice.
- M9.1's rest: the product, record and string families and function layout still run their own enumerations.
- katexlil's patch keeps `src/fontMetricsData.js` for `scripts/audit-parity.mjs` and `scripts/attribute-core.mjs`, which still read it.

---

## 2026-09-27 Batch N: nominal identity

**What landed.**
- **Identities.** `NominalId` names every nominal kind: struct, class (with extern classes and objects) and enum, tagged by kind. `Type::Class`, `Type::ClassInstance` and `Type::Enum` carry a `NominalType { identity, name }` that compares by identity; the name is display data. The checker's class and enum registries are vectors indexed by id.
- **Per-module scopes.** Each module has one type scope, `type_bindings` (name → id), filled by its declarations and its type imports, aliases included. Two modules' private classes, extern classes or enums of one name are two identities. One scope still refuses a second declaration. A mismatch names both declarations ("distinct class declarations"). `language-v0.1.md`'s Modules section states the rule.
- **One checker entry.** `check::analyze` checks a single source as a module graph of one module, through `analyze_modules_in`. The phases hand typed products to each other: graph → declarations (ids, type imports, published classes) → schemas (enums, structs, classes, hierarchies, observed classes) → signatures (functions, typed bindings, value interfaces, `import()`) → bodies, in initialization order. An `auto` export is published after its module's bodies and is refused only when another module imports it.
- **The Program IR.**
  - `ClassDefinition` holds `identity`, `module`, `base: Option<NominalId>`, `observed`, `value`, `published` and `prototype`. `Program::class(id)` looks classes up by identity, and `EnumDefinition` has an `identity`.
  - Class field accesses are `Place::ClassField { receiver, field: FieldRef { nominal, slot } }`: the declaring class and its flattened slot. Instances are `AllocationKind::Instance { class, keys }`.
  - Formation, native, effects, demand, verify and lint read these. Formation prints the field's name, which is display and ABI data.
  - `src/program/nominal_identity_tests.rs` fails if a name-keyed class or enum lookup returns. It checks 22 spellings, ignoring whitespace and comments. Run against the base commit, it finds 40 such lookups.
- **`export constructor`.** Some classes stay JavaScript classes (the checker fact `ClassInfo::observed`, which generalizes host-derived classes): a published class, its internal bases, and every class that extends an observed class. For these:
  - the class's value binding holds its constructor unit (`UnitData::constructor_of`);
  - `new` is `ConstructClass`, and `super(...)` is `SuperConstruct`;
  - a class without `init` gets JavaScript's implicit constructor;
  - a published chain gets prototype methods that call the static bodies.

  Published constructors and prototype methods are interface escapes in the call graph. Their parameters are not typed-defined, so they keep their defaults. Natively, these classes allocate and run their constructor units. One name may export both a class's type and its constructor. A generic kept class is refused.
- **Prior art** (rule 7):
  - Closure `0da58e1`: `JSTypeRegistry.java:218,1133-1136,1508-1526` keys named types by (scope root, name), so each module body is its own row. `EqualityChecker.java:229-245` compares resolved nominal types by reference.
  - esbuild `f6058f8`: `internal/ast/ast.go:374-388` defines `Ref{SourceIndex, InnerIndex}`. `internal/renamer/renamer.go:580-628` names the second `Node` only when printing.
  - Oxc `591966d`: `crates/oxc_semantic/src/scoping.rs:20,281-298` has per-scope name → `SymbolId` maps over flat arrays.
  - Rolldown `5c676e5`: `crates/rolldown_common/src/types/symbol_ref.rs:8-13` defines `SymbolRef{owner, symbol}`.
  - Old-route prior art: `d362338f:src/lower.rs:413-470`, the constructor-export rules.

**Evidence (binary `~/lilscript-work/bin/nominal-2`).**
- **Unit tests:** 1,510 lib tests pass (9 ignored, as on the base) and 21 binary tests pass. `cargo fmt --check` and `check-doc-links` pass.
- **Case runner:** 382 cases × 18 lanes, with no failure outside the ledger.
  - The ledger drops from 10 entries to 9. The M4.1 entry's 6 cases now pass: 42 case-lanes went from refused to pass.
  - Six new cases in `tests/cases/nominal` cover same-named private classes and enums in three modules (JS and C), an extern class shadowed by a private class, class fields across modules through an imported and a renamed class, a published chain across modules (JS and C), and the `export constructor` ES boundary (a module probe).
  - Against the merge binary `merge-i`, one artifact per JavaScript lane changed, `irjs-an_internal_class_extending_a_host_class_is_a_real_subclass`. Its constructor's binding is now the class's own (`Problem`, previously a synthetic `init`). In production that is −1 Brotli and −1 gzip in both the script and module lanes. The unmangled formation-only lanes spell the name, so they grow +6 raw and +1 Brotli. C lanes are byte-identical.
- **Reference ports** (`scripts/ports.mjs --patches none`): all seven are green, and every delivered file is byte-identical to `merge-i`:

  | Port | Files | Brotli |
  |---|---|---|
  | markedlil | 7 | 65,061 |
  | zodlil | 7 | 61,698 |
  | katexlil | 26 | 596,424 |
  | jquerylil | 5 | 127,208 |
  | posthoglil | 6 | 33,786 |
  | micromarklil | 9 | 187,217 |
  | motionlil (built from `portwork/motionlil-reint`) | 629 | 237,216 |

  - motionlil `main` needs `finer/port-migrations/motionlil.patch`. With the patch, its 22 files are also byte-identical (280,587 Brotli). `main` alone fails on both binaries: on `merge-i` with the duplicate-type refusal, and after it with an unimported `Math`.
  - katexlil's browser-performance guard failed once on `merge-i` while two runs shared the host. It passed when `merge-i` was rerun alone.
- **Port renames reverted** (patches in `~/lilscript-work/portwork/nominal/`; no port repository was changed):
  - The "16 port class renames" are 16 class renames in one port, motionlil, made by the 013 batch-4 patch. Reverting all 16 on `main` with the patch builds green (9/9 suites), and the 22 files are byte-identical. `merge-i` refuses the reverted source with duplicate type declaration `GroupAnimationWithThen`.
  - On the integrated rewrite (`motionlil-reint`), 7 renames remain revertible and are reverted: 629/629 files identical, 22/22 suites green. `JSAnimationFull` keeps its name, because its file imports the extern `JSAnimation`.
  - micromarklil renamed the extern views `Point`/`Token` to `TokenizerPoint`/`TokenizerToken` and `AttentionPoint`/`AttentionToken`. The revert builds green (1,963 tests) and its 9 files are identical. `merge-i` refuses it with duplicate type declaration `Point`.
  - mdast-util-from-markdownlil, remark-parselil and react-markdownlil carry copies of the same micromark sources, with the same renamed views; the same patch applies to each copy. unifiedlil's `UnifiedPresetView` and `UnifiedFileView` are new names, not renames.
  - katexlil and zodlil renamed nothing.

**Open.**
- M4.2: the `$js` type parameter; interned types without source lifetimes. `NominalType` carries a `&'src str` display name, so `Type` grew to at most 56 bytes.
- M4.3: checker facts transported.
- M4.4: node ids. Type scopes are still name maps per module, and spans still key checker facts.
- M4.5: contracts at check time.
- M4.6: the operation catalog.
- Gaps left by this batch:
  - kept generic classes;
  - D2 adapters for a published constructor's struct parameters;
  - importing a name that exports both a type and a constructor binds only the type;
  - field facts (M6.7), property renaming (M9.6) and layout choice (M9.7) now have `FieldRef` and `Instance { class }` to key on.

---

## 2026-09-27 katexlil released with its font metrics in LilScript

The plan's step 1. katexlil commits `07d90d6` and `f039133`, pinned binary `~/lilscript-work/bin/release-d`: `katex.esm.js` at 60,281 Brotli, live on its Pages site. That is a win over Terser (63,044), SWC (62,399) and `katex.min.js` (62,686). Its code lane still loses (53,238 against SWC's 53,029 as published); the full file wins through M9.8's data encoding of the font metrics ([batch D](#2026-09-27-batch-d-choices-and-data-tables)). The site's runtime figure is a single noisy draw (M2.12 replaces it).

---

## 2026-09-27 Design version 1 and the plan's revision

The owner asked whether the migration is being done properly and asked for one final design, architected rather than iterated ([2026-09-27](../../finer/intent/2026-09-27.md)). The architecture answers the question in its §1.1: **partly no**, because mechanisms were deleted before their replacements existed, choices were built before rules, gates reported instead of blocking, and the language was left unfinished while the compiler recovered facts. Seven independent reviews of the design and the code, a verified diagnosis of the entries that still lose, a runtime bisection of katexlil, and three adversarial critiques of the first draft produced version 1. This revision of the plan re-scopes and re-orders every remaining phase to converge on it. Every earlier record is unchanged.

**What the reviews measured** (binary `head-d1d48c4c` unless stated):

| Finding | Evidence | Source |
|---|---|---|
| The compile-time gate (rule 3) fails | Level 13 at the ports' shipped configs, against pre-M1: zodlil 4.0×, posthoglil 3.9×, katexlil 3.7×, micromarklil 3.4×, markedlil 1.5×. Outside katexlil's one data-table choice, the extra time bought −88 to +7 Brotli. katexlil's package build went 9.6 → 54.9 s, against KaTeX's own build at 14.4 s | review-compile-time §2.5; measurements-2026-09-27.md |
| Exact Brotli-11 is the compile | 55–74% of wall time at level 13 on every reference port; raw bytes, the ranking the search uses, orders only 5 of 18 real pairs 200–1,000 bytes apart; Brotli q5 orders all 99 pairs ≥ 200 bytes apart at 1/60 the cost | review-compile-time §2.2, §4 |
| Effort above 8 buys almost nothing, non-monotonically | The search winner is byte-identical at levels 8, 13 and 15 on five of six ports; on jquerylil it is 25,466 at level 8 and 25,421 at 13. katexlil's level 13 is 98 bytes larger than its level 8 (two interleaved terminal budgets); level 0 ships 20–28% larger because naming is gated at 8 | review-compile-time §2.3–2.4; `compile-time-data/jquery-L{8,13}-production.explain.json` |
| The per-objective procedure misses its own optimum | katexlil's gzip-objective build is 188 Brotli smaller than its Brotli-objective build (posthoglil −28) | review-objectives §1.1 |
| The runtime gap came mostly with M1, and the adapter part is fixable | Steady-state Node ratio to upstream KaTeX: 1.102 for release-d; pre-M1 1.041; M1 added about 6 points. The recorded "20%" was an outlier of one recording run. Five shared receiver-adapter thunks serve all 141 methods, 9.6–10.0% of JS self time, one GENERIC call site. The adapter change alone (independent re-measurement): katexlil Brotli −62, gzip +2, raw +2,379; Node −0.041 [−0.058, −0.029]; Chromium −0.019 to −0.034; micromarklil −283 Brotli; mobxlil +130 Brotli unless constructors get formals; parity 30/30, Jest 1,230/1,230 | `~/lilscript-work/out/diagnosis/katex-perf-2026-09-27.md`, "Verification"; diagnosis addendum |
| The program rule layer is empty in production | Only liveness (with M7.2) runs on the Program IR; 54 calls to 39 passes run on the JS tree with their own effect and initialization models. `comparison/apps`: 945 against Closure ADVANCED's 834 Brotli (old route 571); `comparison/algorithms`: 3,250 against 2,756 (old route 2,305); all 18 cases lose | review-closure-advanced §2 |
| Local completeness trails every competitor | `comparison/cases`: 37 of 54 canonical cases and 380 of 570 catalog variants lose on Brotli to the best of Terser, Oxc and esbuild; `if (false)` bodies survive | review-competitors §1.5 |
| The language forces the ceremony the diagnosis measures | About 1,040 of the ~1,800 verified recoverable Brotli bytes on the five losing entries; 0 of 11 M10 items started | review-language §2.3; review-code-reality §5.1 |
| Facts landed without consumers | M6.1–M6.5 are computed on the program; the tree still re-derives effects and initialization (rule 2 was waived in `ff27361f`) | review-code-reality A6 |
| Bars and consumers | SWC is the strongest general-purpose bar on 4 of 5 measured boundaries (katex 62,399; remark-parselil's gzip win becomes a tie). A consumer importing one zod export pays 28,006 Brotli from zodlil against 11,749 from upstream (Rolldown) | review-competitors §1.1, §1.4 |
| Native is a subset | 236 of 361 corpus cases masked in C; no maintained library compiles natively ("native exported ABI"); the C output runs 3.3× slower than Node running our JavaScript for the same program | review-performance-native §3.4–3.5 |

**What version 1 changed against the 2026-09-23 page (architecture).**
- **The answer and stability.** §1.1 answers the owner's question; §22 fixes laws, IR, pipeline, procedure and contract behind owner-approved amendments and freezes ids.
- **Laws.** L3 now decides rule against choice by measurement (a removal with a codec-stable sign over the calibration corpus); L7 governs choices only; L20 (replace, then delete) and L21 (gates block) are new; L14–L19 are kept.
- **Objective.** Lexicographic: correct, not slower, smallest, within budget. Runtime parity is defined (architecture §16.4) as the owner ruled; it is not an open question.
- **One decision procedure** (architecture §9.6), stated once: a level-independent move list walked as a prefix; the level sets only the prefix, an exact-judgement budget and a work budget; proxies only prune; monotone across levels by construction; several objectives walk in lockstep with checkpoint offers; a gzip shadow for Brotli builds; the structural beam becomes one move until M9.1's rest deletes it. Program-level moves re-run the rules on what they change.
- **Rules.** One lexicographic measure, fixed structural order, never truncated by effort; transitional rules until replacements land; field-store, unit-merge, class-strictness, construction-order and regex-placement legality written down; `exact` folds only.
- **Annotations.** Binding facts and evaluation behaviour join the tree columns, each with a transfer function and debug verification, before `quiet.rs` goes.
- **Delivery.** Consumer annotations only where `discardable()` holds and only in the bundler-facing file (M3.8a/b); the IIFE default for application scripts with a global-facing refusal; a delta for the M3.3 session.
- **Native.** Shifts stay defined; `volatile` goes only with in-source FP guards; QuickJS's `libregexp`; trial-deletion cycle collection; named portable cores as M11's exit.
- **Language.** [language.md](../language.md) is the v1 target contract, tagged per clause; R2's spelling-observing operations are refused, R4 keeps in-place places with copy-on-store, R6's reflected set is a whole-program closure, R8 uses ABI values at reflected positions, R12 keeps JavaScript's `==` and adds `unknown`, R5 adds intersections, R9 adds specialization over const data.
- **Compile time.** Frozen baselines; targets per entry and per package build; levels 1–4 against Terser's time; B9's exchange rate and ceilings; CPU-time pairs per batch.
- **Runtime.** Hot-site alternatives ordered by runtime class before bytes; a runtime ledger with owners; per-batch timing gates in a quiet window.
- **Benchmarks.** BC5 maps language guarantees; BC10 judges shipped artifacts; the per-cell threshold (Y6); a corrected held-out set with a failure procedure; calibration never on ports; a closure ledger per losing cell (architecture §18.4).
- **Owner decisions.** Sixteen open questions became seven yes/no questions (Y1–Y7), each amending an earlier owner choice; the rest are decided with their reversal cost (architecture §21).

**What version 1 changed in the plan.**
- Rules 2, 3, 4, 5, 7 and 8 are sharpened; rules 9 (order), 10 (not overfitting) and 11 (stability) are new. Every gate blocks; phase gates are ratchets over ledgers; "no losing cell" is M12.3's exit only.
- **New tasks:** M2.10–M2.15 (benchmark contract, held-out set, perf runner and runtime ledger, generic ratchet, frozen baselines, calibration corpus), M3.8a/b, M5.3a/b, M5.7, M6.4a/b, M6.8, M7.5a, M7.8a, M7.10, M9.11, M9.12, M10.12–M10.19, M11.10–M11.12, M12.6.
- **Merged:** M3.6 and M9.4 into M3.5; M3.9's incremental formation into M5.7; M8.1 into M5.2 (annotations) and batch A1 (import identity).
- **Batch A** splits into A1 (formation fixes that need no tree analysis) and A2 (after M2.5, M4.6 and M5.2).
- **Order.** A floor slice of exact program rules and language slice 1 come right after A1; the budget model follows the floor slice; the core is split into batches by the ruling each needs; the critical path waits on no ruling (see the plan's ["Effort and critical path"](plan.md#effort-and-critical-path)).

---

## 2026-09-27 M3.3a: the delivery contract, first slice

**What landed** (the judged design `~/lilscript-work/out/design/m3.3/design.md`, as architecture §10.6 adopts it).
- **One program, several roots.** Entries come from `[delivery.entries]`, `--entry NAME=PATH` and `INPUT` (its stem made a valid name). The checker checks one module graph whose roots are the entries in name order; the canonical schedule is the post-order over them, with each entry's own order and closure beside it. `Program::entries`/`entry_exports`; one cell exported by several entries or under several names is one identity (DL6). Refused, naming both: two entries, or an entry and a module one of them loads with `import()`, entering one static import cycle at different modules (M3.3d).
- **Root rows.** Formation writes one `RootRow` per root statement (module, anchor, origin) from the demand plan's per-operation effects. A `Definition` is a `Let` or `Function` whose operations cannot throw, diverge, run user code, re-enter or suspend and write only the cell they initialize, reading write-once cells; that is `discardable()`. Every rule edits the root through the row helpers (`remove_statement`, `remove_statement_into`, `move_statement`, `insert_statement`, `splice_statements`, `truncate_statements`, `drain_into`, `fuse_row`, `prepend_roots`, `push_root_admitted`), and admission refuses rows that disagree with the root.
- **Placement** (`src/js/delivery.rs`), once per formed tree before naming, stored on the tree:
  - labels are entry bitsets; the fallback plan puts each (label, module) group of anchored statements in its own file, attaches definitions by anchor index and gives each entry a facade;
  - a merge is kept only when a simulation of every static entry's ES module evaluation, and of every lazy load after it, reproduces the sources' order;
  - `preserve-modules` is the reference; `single` with several entries gives each entry its own instance;
  - a write to another file's binding goes through a setter; the plan verifier (P1–P10) runs before printing.
- **Dynamic entries** are the `import()` targets some entry can load without reaching them statically. A lazily loaded file (`split`) or a module file behind a lazy facade (`preserve-modules`) evaluates what the target reaches and the importer has not, in the target's order. Where `import()` is built in place (`single`, or a target without dynamic import), a module an entry reaches only through `import()` that runs code is refused (M3.3d).
- **Render, score, deliver.** Files are printed from the plan with one naming of the whole output. Name templates are `[name] [index] [hash:N] [path] [ext]`, a hash covering every file a file can load or preload. Relative foreign specifiers climb from nested files. Each file is scored by the objective's codec; an entry's row is what it loads plus declared costs; the search ranks by the sum of rows, and the terminal stage keeps a challenger only when no row grows (dominance). Manifest v3 reads the measured sizes and never encodes again. The CLI takes `--entry`, `--out-dir`, `--delivery` (`--mode` is still development/production) and `--format esm`; `-o FILE` writes a one-entry delivery with FILE as the entry file; a rebuild removes only regular files inside the output directory that the previous manifest listed.
- **Deleted with their consumers (L20):** `BundleSpec`, `partition`, `chunkable`, `lazy_modules`, `plan_files`, `select_split`, `Delivered::deploy_cost`, `ChunkCostConfig`, the manifest's re-encoding and `lilscript::javascript_bundle`. `[bundle]` translates to `[delivery]`. `scripts/verify-bundles.mjs` (which failed on head too) runs `scripts/delivery-contract.mjs` and `scripts/delivery-plan.mjs`, over fixtures with `strip_console = false`.
- **The review of the first two commits** (`50d0a787`, `aa5c3dc2`; three adversarial reviews, a small-graph generator, hand-built load sequences) found these defects, each fixed with a test in `ae8b5bb0`:
  - lazy loads skipped the effects of modules the importer had not evaluated;
  - `import()` of a module another entry imports statically never ran it, or was refused with an internal P5;
  - the simulator ignored dynamic entries, and the cycle refusal ignored them too;
  - two modules reading one lazy member failed conversion (pre-existing; first reachable with one entry);
  - carried host code ran for the wrong entries, even with one entry;
  - `preserve-modules`' default templates collided for an entry module another module imports;
  - `[hash]` names were not content-safe across `import()`, and templates with a `.` segment broke specifiers;
  - relative foreign specifiers broke in nested files;
  - `format = "cjs"` printed ESM into `.cjs` files;
  - `-o FILE` and the manifest disagreed and a rebuild deleted a user's file; stale-file removal followed symbolic links;
  - single-file builds of `my app.lil` were refused;
  - `--codec-jobs` became an error (architecture §14.2: warn for one release), and `--delegate-bundling` broke Lilpack's projects with entries;
  - the configuration was found from the first `--entry` in flag order (DL10);
  - dominance was missing; multi-file artifacts reported no retained bytes, starving the search's byte floor; row sums could overflow;
  - root classification was quadratic in a long window of operations (katexlil +2 s of CPU at level 13; fixed in `1eb84d9b`).
- **Prior art** (rule 7):
  - esbuild `f6058f8`: `internal/linker/linker.go:3875-3960` (`computeChunks`: a chunk per entry bitset), `:560` (acyclic chunk imports as an assertion), `:1096` (entries import chunks for their side effects); its content hash covers every file a file can load.
  - Rolldown `5c676e5`: `crates/rolldown/src/stages/generate_stage/code_splitting.rs:83-110` (preserve_modules), `:200-240` (execution order, never the bitset value), `compute_cross_chunk_links.rs:38-46` (no internal export named `then`).
  - Closure `0da58e1`: `JSChunkGraph.java:360-400` (`getSmallestCoveringSubtree`), `CrossChunkCodeMotion.java:40-66`, `ConvertChunksToESModules.java:81-84` (cross-chunk assignment refused there, a setter here).
  - Old route `d362338f:src/compiler.rs:889` (partition after whole-program optimization).

**Evidence (binary `~/lilscript-work/bin/m33a-3`, SHA-256 `af93a5b782634861…`, commits `ae8b5bb0`, `9ec91db1` and `1eb84d9b` on the first two).**
- **Unit tests:** 1,538 lib tests pass (9 ignored, as on the base), 22 binary tests pass; `cargo fmt --check`, `check-doc-links` and `finer/tools/config-schema.mjs --check` pass. The new tests cover each fixed defect and the design's §14.1 scenarios (per load sequence in Node: shared instances and identities, per-entry order, disagreeing entries, a throwing initializer, setters, lazy loads, host code per entry, `interface_hazard`, dominance).
- **Generator** (`~/lilscript-work/out/verify/m33a-fuzz.mjs`): 300 random module graphs on the final code, 150 of them with a lazy `import()`. Each has 3–6 modules, 2–3 entries, effects, host reads, state and writers across modules. Each was built in `split`, `preserve-modules` and `single` and run for every entry alone and every ordered pair with calls in between, against ES module evaluation of the sources: no mismatch. Every refusal was an expected one (the lazy-only rule, or `single` building `import()` in place). Run on the batch as received, the same generator had no static mismatch but exposed the lazy-load defects above.
- **`scripts/verify-bundles.mjs`:** passes. The contract checks, the pinned plans, determinism (identical names and bytes under `-j 1` and `-j 4`, from another working directory, a copy at another absolute path and reordered `--entry` flags), `-o FILE`, the refusals, `--target all` and the package lock.
- **Case runner:** 384 cases × 18 lanes, no failure outside the ledger (9 entries, 174 case-lanes), no stale entry. Against `cases-head-d1d48c4c.json`, 0 artifacts changed on every lane: every single-entry `single` build is byte-identical.
- **Port runner** (`--patches none`, ten ports, against a `head-d1d48c4c` run):
  - All 706 delivered files are byte-identical to head's. katexlil's were compared from one snapshot of its working tree, which another session was editing between the two runs.
  - Green, with the same suites as head: markedlil 29/29, zodlil 1,353/1,353, katexlil 1,258/1,258, jquerylil 7/7, posthoglil 21/21, motionlil 22/22, micromarklil 1,963/1,963, react-markdownlil 123/123.
  - mdast-util-from-markdownlil (744/746) and remark-parselil (16/18) fail the same three site tests on head. Their `site/results.json` records the sizes of dist files committed from compiler `aa2052f0`, and both binaries now write `from-markdown.esm.js` at 71,654 bytes (71,802 recorded) and `remark-parse.esm.js` at 71,918 (71,995 recorded). The owner is the ports' rebuild with the current compiler (M12.2).
- **Compile time** (level 13, main entry, three alternating pairs against `head-d1d48c4c`, medians, process CPU then wall; load average 5.6–11.2 on the shared host): markedlil 1.83 → 1.82 s (2.00 → 1.98 s), katexlil 19.83 → 20.00 s (19.83 → 20.26 s), micromarklil 2.59 → 2.32 s (2.60 → 2.33 s); each output byte-identical to head's. The deterministic work units are within +0.005% of head on all three. The batch as received charged katexlil 4.4× head's render work and micromarklil 1.3×: root classification re-read every operation since the previous root statement at each operation, which is quadratic. It cost katexlil about +2 s of CPU, and was fixed to a running fold with no change to any row or byte.

**Open** (each with its owner).
- R1 and R2 (§5.4): every entry's interface escapes at the one `END`. No consumer reads a flow-sensitive root fact across entries today; the design's `interface_hazard` case guards it (a unit test). M3.3c.
- `RootRow::completes` (a prototype store completing its class's definition): such stores stay anchored, which only costs mobility. M3.3c.
- The case runner has no delivery cases yet (a `[delivery]` table in a case's `.toml`, `--out-dir`, the probe's `delivery.run`); the §14.1 scenarios are unit tests and `scripts/delivery-contract.mjs` checks instead. M3.3b, with the script and CJS lanes.
- Scores where no judge needs them: level 0 still measures exactly (M3.5); each render re-scores unchanged files (the codec memo, M3.9).
- `single` with several entries delivers definitions only another entry needs (size; placement choices, M3.3c).
- The LSP and `lilscript-lint` check one root; lint's exclusion reads the first entry. M3.4.
- Manifest module names are relative to the common directory, which is `/` for modules in unrelated trees; `[path]` segments are not percent-escaped; v2 manifests are not read for stale files; the translator names `bundle.cost` rather than the leaf key. M3.3b.
- `entry_graph` and the cycle refusal allocate outside `AllocationBudget`. M5.6.
- Minor deviations from the design: equal sums of rows break ties by candidate, not by the total; a lazy row's depth counts import levels its importer already loaded; the cycle refusal spans the whole entry module, not the import; script builds report `format = "esm"` (`bare` arrives with M3.3b); `javascript.function_scope` still warns "no effect" where design §4 refuses it, pointing to `format = "iife"` (pre-existing). M3.3b and M3.3c.
- Not run: the design's reported-not-gated scratch build of motionlil's ten entries as one program (its `main` needs `finer/port-migrations/motionlil.patch` on this binary as on head). M3.3e.

**As merged** (the plan's step 2 when M3.3a merged, 2026-09-27). Architecture §10.6's delta is in M3.3a: `-j` is the one parallelism flag (`--codec-jobs` warns for one release), every rule edits root statements through the `RootRow` helpers (batch A1's one decoder per schema goes through `prepend_roots`), and a `Definition` row's computation is `discardable()` (M3.8a's premise holds); scores only where the judge needs them stays with M3.5 (level-0 estimates) and M3.9 (the codec memo). M3.3b's work in progress, saved from this worktree, is `~/lilscript-work/portwork/m3.3b-wip.patch`.

---

## 2026-09-27 Batch A1: canonical formation without tree analysis

**What landed** (M8.2 A1 with M11.2, M2.3's item, M9.5's seed and M8.1's imports; three commits on M3.3a's `070a7d4c`: `bd7b8e71` policy, `f704e1f8` formation, `8973b63a` native).
- **A method is its own function** (law P1; `src/program/javascript_methods.rs`). A `JS.methodN`/`methodRest`/`staticRest` callback private to its adapter (a lambda whose one use is the adapter's argument, or a function or lambda in a cell whose one read is) is formed as the adapter's result:
  - its receiver reads `this`; where a nested non-arrow function reads it, or the program assigns it, `let r=this` opens the body; an unread receiver is dropped;
  - a rest list read only as `list[k]` for constant `k` becomes named formals, and the function's declared `length` stays 0; any other rest list reads `arguments`, through `let l=arguments` where a nested function has its own. A declared `length` now prints one initializer, at the first parameter it excludes (`(a=void 0,b,c)`), not one per parameter;
  - the other parameters, `length` and constructibility are the adapter's; it is an arrow only where nothing can construct it or read its `prototype`; its name is the empty name where the contract observes names, and otherwise it may gain the name JavaScript infers (R6; the `method_names` case and a unit test pin it);
  - legality: no suspension, not a constructor unit, no struct across a boundary (its D2 wrapper passes the receiver as an argument), and no read of an enclosing method's receiver or list, or of an ambient `this`/`arguments`, unless the result is an arrow, which sees them as the lambda did (`method_lexical_receivers`).
  - No shared adapter factory serves a private callback. The factories remain, transitional, for callbacks that are not private (a parameter, a function other code reads) until R7 retires the adapters (M10.4). `dissolve_receiver_adapters`, `lexical_receiver` and `receiver_adapter` are deleted with their two calls.
- **Observed classes print with their bodies** (C1). A published class's prototype method whose static body nothing else calls is that body, formed inside the class with its receiver as `this`; a body other code also calls stays shared behind a forwarding method. Under `execution = "script"` a body moves into the (strict) class only when strictness cannot change it: no host member, index or global write, no `JS.set`/`JS.delete`, no ambient `this` or `arguments`. A root class whose constructor takes nothing and does nothing prints without one (`Expr::Class.constructor: Option`). A derived class always has a formed `init` (the checker refuses an inherited constructor export without one), so the exact-forwarding case does not arise. A constructor whose instance is `this` drops the dead `let r=this` alias.
- **Operands are not spilled** (C3). A deferred closure's consumer tree was bounded by a fixed height, so a tall object or array literal holding a closure captured its earlier operands into temporaries. `value_placement::plan_with_closures` records, per deferred closure, the depth its body starts at (the deepest tree holding it), and formation plans each body there; a taller tree is admitted while that depth stays in the first half of the nesting limit. A constant key never carries a sequence: `{[(t=…,"k")]:v}` is `{k:(t=…,v)}`.
- **One decoder per schema** (`src/js/tables.rs`). A `Schema` (collections, per-column scale, dictionary and split, which dictionaries exist) is everything a columns decoder is a function of; tables of one schema call one decoder (katexlil shipped two byte-identical ones). The decoders enter the root through M3.3a's `prepend_roots` with synthetic rows.
- **Imports by identity** (C18, from M8.1). A foreign import's identity is `(source, imported)`: every module importing it shares one binding, and its local name is the naming allocator's (the spelling pinned "for katex's build" is gone). One declaration per specifier, in first-appearance order; the imported name `default` prints `import x from"s"`. Single-file and planned-file printing share it, with M3.3a's rebasing of relative specifiers.
- **`print` is never stripped** (M2.3). `strip_console` is retired as a rename to `javascript.strip_debug` (its value moves there, with a warning that says what became of `print`). `strip_debug` drops `debugLog` calls and `strip_console_calls` drops method calls of the host `console` (a foreign global, never an import); both are off by default and keep their arguments' evaluation. The demand plan owns the predicate (`DemandPlan::stripped_call`); formation asks it. The case runner sets no logging key and refuses the retired one; fixtures, scripts and the repository's `lilscript.toml` drop it.
- **Naming's seed at every level** (M9.5). The allocator's seed (`Style::Scoped`) is the one plan eligible without `naming-search`; the search's alternatives follow it (`Scoped, Global, Source`). Level 0 no longer ships the global baseline: at level 0 markedlil is 11,065 → 9,225 Brotli, micromarklil 26,428 → 21,628, zodlil 34,445 → 26,905.
- **Native arithmetic** (M11.2; `src/program/native_runtime.rs`). The generated file states its build contract in a header comment (C11, the compiler's default floating-point options, no fast-math, unsafe-math, `-fno-signed-zeros` or `-ffp-contract=fast`) and enforces it: `#pragma STDC FP_CONTRACT OFF`, `#pragma GCC optimize("fp-contract=off")` (GCC ignores the standard pragma), `#pragma clang fp contract(off)` and `reassociate(off)`; `#error` on GCC's reassociation, reciprocal and no-signed-zero macros; a startup check that refuses the contraction no pragma stops (Clang's `-ffp-contract=fast`). `ls_f64` is the identity. `ls_mul` multiplies in `int64_t` and wraps while the product is within ±2^53 (exact there), else takes ToInt32 of the double product. `ls_shr` is a plain `>>` under `_Static_assert((-1 >> 1) == -1)`; left shifts stay on `uint32_t`.
- **Tools and ledgers.** The port runner reports a ledgered test as passing only when its suite ran a test (`suiteRan`): mobxlil's three upstream-path entries had been reported "now passing" because no suite loaded. The port ledger gains mobxlil's build and suite failure and the two site-receipt failures of mdast-util-from-markdownlil and remark-parselil, each identical on head, owned by M12.2.
- **Prior art** (rule 7):
  - Closure `0da58e1`: `FunctionRewriter.java:42-44` (off by default for its shared helpers' runtime cost, law P1); this is the inverse of `DevirtualizeMethods.java:39-56` under its preconditions.
  - Terser `8fa44c8`: `lib/compress/index.js:3613-3651` (`arguments[k]` to formals, `keep_fargs` for `length`), `lib/compress/inline.js:169,322` and `lib/compress/index.js:425-437` (`in_computed_key`: nothing is inlined into a computed key), `lib/compress/index.js:236,534-560` (`drop_console`, off by default, undeclared `console` only).
  - esbuild `f6058f8`: `internal/js_parser/js_parser_lower_class.go:1978-1986` (a constructor only when needed), `js_parser.go:12248` (class bodies are strict), `js_parser.go:1790-1807` (runtime helpers keyed by name, one per module), `internal/js_printer/js_printer.go:4726-4732` (the default clause first), `pkg/api/api_impl.go:691-699` (`--drop:console`), `internal/renamer/renamer.go:260` with `internal/linker/linker.go:492` (frequency naming whenever identifiers are minified, whatever else is on).
  - SWC `42e4f7e98`: `crates/swc_ecma_minifier/src/compress/optimize/unused.rs:901-930` and `mod.rs:1827` (empty constructors dropped for base classes only).
  - Rolldown `5c676e5`: `crates/rolldown/src/ecmascript/format/esm.rs:406-452` (one declaration per external module, its default alias first).
  - Old route `d362338f:src/codegen_ir_js.rs:7544-7568` (adapters fused into `this`/`arguments`) and `:2562` (the `(0,function…)` name suppression).
  - No competitor emits C: M11.2's legality is C11 §7.12.2 and §6.5.7, GCC's `-ffp-contract=fast` default outside ISO modes, and Clang's pragma, which `-ffp-contract=fast` overrides.

**Evidence (binary `~/lilscript-work/bin/a1-8`, SHA-256 `7e720be75a6dd9e3…`, the tree of `8973b63a`).**
- **Unit tests:** 1,543 lib tests pass (9 ignored, as on the base) and 22 binary tests pass; `cargo fmt --check`, `check-doc-links`, `finer/tools/config-schema.mjs --check` and `node --test scripts/verify-runners.test.mjs` (8) pass. Each of the three commits checks alone (`cargo check --lib --tests --bins`). New tests: an adapter callback as its own method, with the gained name pinned; class bodies under a classic script; the retired key's rename; `console` stripping and `print` kept; two tables of one schema through one decoder; one declaration per import specifier; `suiteRan`.
- **Case runner** (Clang 18 for C): 395 cases × 18 lanes, no failure outside the ledger (9 entries, 174 case-lanes, unchanged). Eleven new cases with `.out` oracles: `formation/{method_receivers, method_lexical_receivers, method_names, class_bodies (with a module probe), class_bodies_script, literal_operands, shared_decoders, imports_by_identity, print_is_kept}` and `native/{float_default_flags, int_arithmetic_edges}`.
  - Against `head-d1d48c4c` (M3.3a's outputs are byte-identical to it), production lanes: Brotli −609 (script) and −637 (module), gzip −712 and −764, raw −1,604 and −1,550; formation-only Brotli −1,142 and −1,294. 518 JavaScript case-lanes shrank and 6 grew, by 1 to 3 bytes (`irjs-adjacent_binding_merge_ignores_names_inside_nested_initializers` +1/+2, the `-local` adapter-name case +1, `compiler-module_level_assignment_is_not_shadowed_inside_setter` gzip +3).
  - C artifacts carry the stated contract and its guards: +1,427 to +1,491 bytes each, +169,831 over 126. The C lanes also pass under GCC 13 (6 lanes × 128 pass; the C artifacts of the final binary are byte-identical to those of the GCC run). `native/float_default_flags` builds with each compiler's default flags (`-O2`, GCC's `gnu17` contraction included).
- **Port runner** (`--patches none`, `--jobs 1`, the 11 ports from one snapshot of their HEAD commits, `~/lilscript-work/out/ports-a1-clean`, where mobxlil's refused `priority` key is removed in the copy only): no failure outside the ledger. Green with the same suites as head: markedlil 29/29, zodlil 1,353/1,353, katexlil 1,258/1,258, jquerylil 6/6, posthoglil 21/21, motionlil 22/22, micromarklil 1,963/1,963, react-markdownlil 123/123. mobxlil, mdast-util-from-markdownlil and remark-parselil fail exactly as on head and are ledgered (M12.2). Replayed against the new ledger, the recorded head runs (the ports' own directories and the snapshot) are green too.
- **Sizes of the compiler-written files, against head on the same snapshot** (Brotli, gzip, raw):

  | Port | Files | Brotli | gzip | raw |
  |---|---|---|---|---|
  | markedlil | 4 | −15 | −16 | −106 |
  | zodlil | 1 | −70 | −137 | −286 |
  | katexlil | 2 | −166 (`katex.raw.js` −83) | −156 | +1,670 |
  | jquerylil | 1 | +5 | −8 | −590 |
  | posthoglil | 3 | 0 (byte-identical) | 0 | 0 |
  | micromarklil | 4 | −792 (`micromark.raw.js` −352) | −993 | +568 |
  | mobxlil | 3 | +72 (`mobx.esm.js` +32, `…production.min.js` +68, `mobx.dev.esm.js` −28) | −17 | +5,680 |
  | mdast-util-from-markdownlil | 2 | −386 | −437 | +624 |
  | remark-parselil | 2 | −367 | −495 | +555 |
  | react-markdownlil | 1 | −297 | −320 | +784 |
  | **total** | 23 | **−2,016** | **−2,579** | +8,899 |

  The files the ports derive move with them: katexlil's `katex.esm.js` −156 (its `katex.cjs` +69 from esbuild), `contrib/mhchem.{mjs,min.js,cjs}` −343/−371/−378; motionlil's graph files −1,026 (its own splitter groups the new root statements into 625 files, where it made 629); all derived files −7,330.
- **Runtime** (katexlil in Node against upstream KaTeX, 14 alternating fresh-process pairs of 200 rounds with no-op controls, `~/lilscript-work/diag/katex-perf/verify/node-a1-8.jsonl`; load 2.2–2.9 with nothing else running): steady-state ratio 1.097 → 1.047, Δ −0.050 [−0.062, −0.040], every A1 draw below every head draw; late window −0.041 [−0.053, −0.034]. The controls move by −0.006 [−0.017, +0.001] and +0.001 [−0.009, +0.011]. Parity: 0 mismatches.
- **Compile time** (level 13 on the frozen configs, main entries, five alternating pairs against `head-070a7d4c`, process CPU medians; load 2.2–2.5 with nothing else running): markedlil 1.13 → 1.14 s, zodlil 1.68 → 1.56 s, katexlil 11.65 → 11.99 s (+2.9%), micromarklil 1.68 → 1.70 s, posthoglil 0.27 → 0.28 s. All within rule 3's +15%. The case runner's CPU pair was not taken: from 18:04 an upstream baseline build (posthog-js under turbo, 6.5 GB) shared the host, and the owner's rule is one heavy job at a time; it is the one measurement this batch still owes.
- **Generic corpus:** `comparison/apps` 945 and `comparison/algorithms` 3,250 Brotli, byte-identical to head on all 18 programs, every oracle passing (none has an adapter, a kept class, a table or a foreign import).
- **Level 0** (naming's seed): markedlil 11,065 → 9,225 Brotli, micromarklil 26,428 → 21,628, zodlil 34,445 → 26,905.

**Pre-registered against measured.**

| Item | Pre-registered | Measured |
|---|---|---|
| katexlil Brotli | −62 (the adapter change alone) | −83 on the compiler-written `katex.raw.js`; −156 on the delivered `katex.esm.js` |
| katexlil Node steady | −0.04 | −0.050 [−0.062, −0.040] |
| micromarklil | −283 Brotli, −311 gzip | −352 Brotli, −419 gzip on `micromark.raw.js` (−792 and −993 over its four compiler-written files) |
| mhchem | −250 | −343 (`mhchem.mjs`), −371 (`.min.js`), −378 (`.cjs`) |
| zodlil | within noise | −70 Brotli, −137 gzip on `zod.core.js` |
| mobxlil | at or below zero | **missed:** +32 on `mobx.esm.js`, +68 on `mobx.esm.production.min.js`, −28 on the development build (+72 over the three) |

The mobxlil miss, decomposed with diagnostic switches on this tree (`~/lilscript-work/snip/a1/out-restshared`, not shipped): with no method forms at all `mobx.esm.js` is 18,196 Brotli (−35 against head, the other changes); the method forms whose receivers read `this` and whose constant-index lists are formals cost +117; the lists spelled `arguments` save 50 against the shared factory. So the plan's alternative for constructors, a private, unshared function each, cannot help: its lower bound, the shared factory, is larger on all three files (+50, +36, +66), and the batch keeps `arguments` for lists that cannot be formals, constructors included. What mobxlil pays is law P1's floor (one body per method, `this` spelled at each read). A receiver alias `let r=this` from four reads on, measured on an earlier build of this batch, is mixed (mobxlil −28/−47/+52, katexlil +140): a codec-dependent spelling, not a rule.

**Open.**
- mobxlil's +72: law P1's price on a port whose methods read their receiver often. Whether a method aliases its receiver is a spelling choice for M9.3 once M3.5 admits new families; R7 (M10.4) takes the adapters out of the language.
- Callbacks that are not private to their adapter (parameters, functions other code reads) still get the shared factory, transitionally, until R7.
- A rest list read with `.length` or a varying index reads `arguments`; a rest parameter is R7's spelling (M10.4), not formation's.
- The architecture's own "today the default strips it" (`future-architecture.md`, Stripping) now describes the past; the text is the architecture's to amend (§22).
- The mobxlil development bundle needs its host globals defined, and the two site receipts need refreshing: the ports' M12.2 rebuilds (ledgered).

**The batch as planned** (M8.2's text for A1 before it landed):

**Batch A1 (landed 2026-09-27, branch `m8-a1`; "Batch A1" above records what landed, where it departs from this text, and the measurements):**
- **A method is its own function** (law P1): `JS.methodN`/`methodRest` of a private lambda (or a private function referenced only by the adapter) is emitted as a function whose receiver reads `this`; a rest list read only at constant indices with `length` unobserved becomes named formals (katex-perf B2), otherwise it reads `arguments`. Guards: the receiver and rest parameters are never assigned or captured by a nested non-arrow function or a nested class's methods (the prototype's `receiver_reads` misses `Expr::Class` methods and must refuse there); the lambda has no own `this`/`arguments`; arity is kept; a case pins the gained `.name`. Constructors take the formals form or keep a private, unshared function each (the unconditional constructor form costs mobxlil +130 Brotli). *As landed:* a list that cannot be formals reads `arguments` in constructors too, because the alternative measured larger: even its lower bound, the shared factory, is +36 to +66 Brotli on mobxlil's three files. Shared adapter factories (`src/program/javascript_host.rs:43,538-555`) are no longer emitted for private lambdas; `dissolve_receiver_adapters` is deleted. Prototype: `~/lilscript-work/diag/katex-perf/receiver-methods.patch`. **Pre-registered:** katexlil −62 Brotli (the adapter change alone), Node steady −0.04; micromarklil −283 Brotli and −311 gzip; zodlil within noise; mobxlil at or below zero.
- **Observed classes print with their bodies** (diagnosis C1) within architecture §10.2's legality (class-body strictness under `execution = "script"`; a constructor omitted only for a parameterless empty base or an exact forwarding derived constructor); the dead receiver alias is dropped. Then posthoglil's error-tracking port change to `export constructor` (−360 with both halves).
- **Operands are not spilled** (C3): object and array literal operands keep argument order without temporaries; a constant computed key never carries a sequence (mhchem −250).
- **One decoder per schema** at formation, keyed by schema digest (`src/js/tables.rs:1471-1474`), routed through M3.3a's `RootRow` helpers.
- **Imports** (C18), from M8.1.
- **`print` never stripped** (M2.3's open item): `strip_console` splits into `strip_debug` and `strip_console_calls`, both off by default for libraries.
- **Naming's seed at every level** (M9.5): out of the level-8 gate (`src/compilation_policy.rs:240`).
- **Native arithmetic** (M11.2) with its guards.

---

## 2026-09-27 The tools, first part: M2.13, M2.5 and M2.14

**What landed.**
- **The generic corpus ratchet** (`scripts/ratchet.mjs`; [testing.md](../testing.md#the-generic-corpus-ratchet)). `comparison/cases` (54 canonical, 570 catalog), `comparison/apps` and `comparison/algorithms` compile in three objective lanes, run against their oracles (the reference programs' stdout; the algorithms' ordered host accesses too) and are measured with the canonical codec. Each artifact is compared in its own metric with three bars fixed in `tests/ratchet/bars.json`: the smallest valid competitor (Terser with and without its safe property lane, Oxc, esbuild; the algorithms' bundler lanes), Closure ADVANCED (apps and algorithms), and the frozen old route. A run fails when a loss count or any item's loss grows, a passing lane fails, a loss or failure is not covered by an owned entry of `tests/ratchet/ledger.json`, or the bars changed under `tests/ratchet/baseline.json`. Improvements are reported, and `--update-baseline` tightens the baseline and refuses while anything blocks. The gate needs only Node, the compiler and the codec (bars, old-route configurations and reference host traces are committed); it runs in about 40 s at `--jobs 3` and in CI's gate job. The competitor recipes moved to `comparison/cases/recipes.mjs`, shared by the paired-case harness and the ratchet.
- **Harness conformance (BC12).** The six case configurations carried six retired keys (`identical_function_folding`, `function_subsumption`, `pool_numeric_literals`, `stable_local_names`, `local_name_reserve`, `mangle.exports`); they are removed, and every configuration's policy fingerprint is unchanged (the old route's bars keep the originals, `tests/ratchet/old-route/`). Five harvested regression configurations that held only `mangle.exports` or `mangle.extern_fields` are deleted. The 38 failing catalog variants: the 27 that called name-keyed host helpers the old route gave bodies to (`mathMax`, `objectHasOwn`, `isFunctionValue`, `isWindowValue`, `windowSelf`, `defineConfigurable`, `defineIterator`, `typeOf`), and the canonical `host/math-max`, are rewritten to declared host bindings (`extern class` views of `Math`, `Object` and `Reflect`; `extern JsValue globalThis`) or to the predicate the reference program spells, with the JavaScript oracles unchanged; the other eleven are ledgered: ten on record spread (M10.8), one on `??=` on a place (M10.9). The review's "33 on record spread" counted case-lanes.
- **NO3** (`src/no_library_knowledge_tests.rs`, `cargo test --lib`): 30 mentions of ports and upstream libraries in non-test `src/`, in 22 (file, library) entries of `tests/no3-allowlist.json`, each with its reason and owner (M3.5, M7.5a, M8.2 A1, M8.7, M9.3, M9.8, M9.10). A new mention fails; a removed one must lower its entry, so the ledger only shrinks. The review's "about 35" used a wider word list; this one counts English words (`marked`, `motion`, `remark`, `unified`) only in their library spellings. Thresholds without a policy or estimator source are not checked lexically yet (NO3's second half stays with M8.7).
- **NO4**: the idiom debt ledger `tests/idiom-debt.json` with its schema `tests/idiom-debt.schema.json` and a validator in the runner tests. It starts empty.
- **Hygiene.** `rayon` is removed (declared, never used; 46 lockfile lines); the `Cargo.toml` comment no longer claims an admission parse that did not exist; `docs/current-status.md` describes version 1.
- **Prior art** (rule 7): Oxc `591966d` `tasks/minsize/src/lib.rs:49,144-146` keeps minified sizes against esbuild in a checked-in snapshot, reviewed by diff, not blocking; the paired-case and algorithm harnesses (`comparison/cases/run.mjs`, `comparison/algorithms/run.mjs`) gate every case absolutely and so failed on every run (495 of 624 cases), which is why they reported instead of blocking. The ratchet keeps their recipes and oracles and blocks on growth.

**Evidence.** Baseline `head-d1d48c4c` (SHA-256 `47048e41…`); the branch's own build reproduces every item's sizes. Brotli, ours against each bar where both exist:

| Corpus | Passing | vs smallest competitor | vs Closure ADVANCED | vs old route |
|---|---:|---|---|---|
| `comparison/cases` | 612 of 624 | 49,223 / 43,541, 439 lose | — | 49,126 / 33,656 over 611, 603 lose |
| `comparison/apps` | 7 | 945 / 870, 5 lose | 945 / 834, 7 lose | 945 / 558, 7 lose |
| `comparison/algorithms` | 11 | 3,250 / 3,039, 9 lose | 3,250 / 2,703, 11 lose | 3,250 / 2,305, 11 lose |

Loss counts (raw / gzip / Brotli): cases against the competitor 399 / 400 / 439 and against the old route 610 / 600 / 603; apps against the competitor 1 / 3 / 5, Closure 5 / 6 / 7 and the old route 7 / 7 / 7; algorithms against the competitor 5 / 8 / 9 and against Closure and the old route 11 in every metric. On the unchanged catalog the harness reproduces the review's standing exactly (canonical 9/6/37/2, 4,058 against 3,721; catalog 59/93/380/38, 42,743 against 37,689). The old route's apps total is 558, not the review's 571: the bars compile the corpus's own configurations (level 15, `always`); its algorithms total, 2,305, matches. The algorithms' Closure bar is the smaller of the harness's two Closure lanes (single file and module graph), 2,703 against the review's 2,756. The old route itself fails three lanes: one wrong output (`cases/catalog/number/edge-number-flow-n1nn0p75n4n3`, Brotli) and two refused by its own standards-parser admission; those lanes have no old-route bar. **Re-baselined after batch A1** on `head-a1` (SHA-256 `7e720be7…`): no loss count changed and `comparison/apps` and `algorithms` are byte-identical (945, 3,250); 18 loss rows of three host variants shrank (a method is its own function), and the cases' Brotli total fell from 49,223 to 49,170. After A1 split `strip_console`, the case configurations spell `strip_debug = false` and `strip_console_calls = false` (policy fingerprints unchanged).

**M2.5, the admission parse: what landed.**
- **Every delivered file is parsed again inside admission.** `ArtifactArena::qualify` (`src/program/artifacts.rs`) runs Oxc 0.143 over each file of an artifact before policy admission and before any score is compared: as a module or a script, as the artifact executes, with regular expression literals validated and `import`/`export` refused in a script (Oxc leaves that to its semantic pass). The verdict is computed once per artifact and reused by its qualification under each codec.
- **Its structure must be the printed tree's (A5).** Both sides reduce to one canonical form and compare SHA-256 digests. `src/admission_parse.rs` states the form: which tokens make which statement and expression and how they nest, with a closed list of spelling equivalences applied identically to both sides (literals are one kind, so `!0`, `void 0` and observed strings compare equal; `x|<literal>` is `x`; `(0,x)` and the `{n:f}.n` name wrapper are their operand; `x op= y` and `++x` are `x=x op y`; the statement spellings of `if` (`a&&b`, `a||b`, `x=c?a:b`) and of `return` (`if(c)return a;return b`) read back; declarations split; a `for` head is a block; a body is its statement list; an arrow's expression body is a `return`; identifiers are not compared; the host-module prelude is opaque). `src/js/admission.rs` reduces the printed tree, following the printer's single-file layout (one import declaration per specifier since batch A1) and its native parameter defaults (one `=void 0` at the reflected length since A1) and nothing else of its decisions; a new spelling family must name its equivalence there, or admission refuses its output.
- **A refusal names where.** `CandidateError::AdmissionParse` carries the parser's message and byte offset with an excerpt, or the statement count and the first group of top-level statements whose digest differs (the digest keeps 16 bucket digests of the top-level statements for this), with an excerpt. The direct path and the search baseline fail the build with it; a refused search candidate stops the search with the best admitted artifact (`stop` in the report); a refused terminal challenger is `Refused`.
- **Delivery plans** (M3.3a, merged into this branch): every file of a plan is parsed, as ES modules or, under `format = "cjs"`, as scripts; comparing each file with its part of the tree (`src/js/print_files.rs`) is owned by M3.3b, next to the plan verifier.
- **Tests:** the canonical form's equivalences and its distinctions (`src/admission_parse_tests.rs`: 25 spelling pairs read as one structure, 14 grouping and nesting pairs do not, unparsable text refused with where); a printed tree admitted and misprinted trees refused, namely a statement moved out of an `if`, dropped braces, dropped grouping parentheses and a dropped separator (`src/js/admission_tests.rs`); through `qualify` itself, a keyword run into its operand (`return17`) and a dropped brace refused and the printed file admitted (`artifact_admission_tests.rs`).
- **Prior art** (rule 7): the old route's standards-parser admission (`d362338f:src/compiler.rs:4847-4876`) parsed terminal leaves as modules only and compared nothing. Closure `0da58e1` compares ASTs structurally with `Node.isEquivalentTo` only in its tests (`test/com/google/javascript/jscomp/CompilerTestCase.java:1447-1470`), and Oxc `591966d` checks print idempotency only in its minifier tests (`crates/oxc_minifier/tests/mod.rs:56-129`). None admits production output by structure.

**M2.5 evidence (binary `~/lilscript-work/bin/m2tools-m25`, SHA-256 `d984c6f3…`).**
- **Case runner:** 384 cases × 18 lanes, no failure outside the ledger, and every artifact of every lane byte-identical to `head-d1d48c4c`.
- **Generic ratchet:** passes against the baseline (642 items × 3 lanes; no size changed).
- **Reference ports** (`scripts/ports.mjs --patches none`): all seven green with every delivered file admitted: markedlil 29/29, zodlil 1,353/1,353, katexlil 1,258/1,258, jquerylil 7/7, posthoglil 21/21, motionlil 22/22, micromarklil 1,963/1,963.
- **Cost** (`LILSCRIPT_TIMING=1` buckets `admission_structure` and `admission_parse`; level 13 at the shipped configurations, medians of five runs at load 5–10): katexlil 525 ms of 23.9 s, 2.2% (the tree's digest 171 ms over 32 renders, the parse 354 ms over 32 artifacts of 261 KB); markedlil 62 ms of 1.56 s, 4.0%. Linear in the bytes of each admitted artifact, so M3.5's smaller exact budget shrinks it with the codec's share.

**M2.5 on the merged tree** (M3.3a, the site refresh and batch A1 merged; binary `~/lilscript-work/bin/m2tools-a1`, SHA-256 `7dd62337…`). A1's printer groups foreign imports by specifier and prints `=void 0` only at the reflected length; the tree side follows both. The case runner (395 cases × 18 lanes, `--jobs 1`) has no failure outside the ledger; the generic ratchet's 1,926 lanes are byte-identical to `head-a1`; the seven reference ports are green with every delivered file admitted (markedlil 29/29, zodlil 1,353/1,353, katexlil 1,258/1,258, jquerylil 7/7, posthoglil 21/21, motionlil 22/22, micromarklil 1,963/1,963).

**M2.14, the frozen baselines.** The seven reference ports' sources on 2026-09-27 (`git archive` of each HEAD, motionlil at origin/main `33826f6`, with their configurations: `~/lilscript-work/baselines/2026-09-27/`) compiled by the pre-M1 binary, frozen once because later sources will not compile on it. Median of five runs alternated with the head binary `d1d48c4c`, after one warm-up; the package build rotated with head's and with upstream's own build at its pinned commit; Terser 5.51.2 on upstream's bundled graph. Measured in a quiet window after the host's reboot (load 0.5–4.5, median 2.6, no steal); a first pass under load 5–23 is kept as `receipt-under-load.json` and superseded. Every 2026-09-27 source, katexlil's font metrics in LilScript included, compiles on the pre-M1 binary with no warning. Architecture §13.2 now cites these numbers; `tests/compile-time/frozen-2026-09-27.json` is the checked-in copy gates read.

| Port | Main entry, level 13: pre-M1 s (s per 100 KB) | head `d1d48c4c` s | Package build: pre-M1 / head s | Upstream's build s | Terser `-c -m` / `passes=3` s |
|---|---|---|---|---|---|
| markedlil | 0.82 (2.29) | 1.17 (1.43×) | 2.17 / 3.62 | 6.08 | 0.36 / 0.51 |
| zodlil (`scripts/zod.lil`; core `src/entry.lil` 0.67 / 1.77) | 0.82 (0.92) | 2.02 (2.48×) | 1.02 / 3.02 | 11.39 | 0.87 / 1.37 |
| posthoglil | 0.11 (0.67) | 0.41 (3.64×) | 0.72 / 2.07 | 299.74 | 0.26 / 0.31 |
| micromarklil | 0.67 (0.96) | 2.27 (3.41×) | 2.02 / 6.78 | 10.99 | 0.56 / 0.82 |
| katexlil | 4.62 (1.70) | 17.60 (3.81×) | 10.74 / 37.29 | 12.74 | 1.12 / 1.72 |
| jquerylil | 28.83 (38.81) | 30.63 (1.06×) | 23.51 / 22.01 | 3.12 | 1.12 / 1.57 |
| motionlil (`src/full.lil`) | 14.34 (12.91) | 16.50 (1.15×) | 19.76 / 22.87 | 32.79 | 1.02 / 1.67 |

- At each port's shipped level (15 where it is not 13), pre-M1 / head: markedlil 0.82 / 1.37, zodlil 0.77 / 2.77, posthoglil 0.11 / 0.41, micromarklil 0.62 / 2.27, jquerylil 23.87 / 22.36, motionlil 14.69 / 18.00 s.
- Against §13.2's targets: level 13 today is 1.06–3.81× the frozen value (3.81× on katexlil); katexlil's package build (37.29 s head) is 2.9× KaTeX's own (12.74 s); jquerylil's is 7.1× jQuery's own (3.12 s). With the frozen katexlil value at 4.62 s rather than the provisional 5.6 s, §13.2's single-threaded ceiling (1.3×, 6.0 s) is below its own projection for M3.5 (6.5–8.1 s); §13.2 now says so.
- posthoglil's upstream build ran 12 turbo tasks without cache (`--force`), about 490 CPU-seconds a run; the port's source-build record of 2026-09-24 has 170 s.

---

## 2026-09-28 Owner decisions: counted budgets, decide statically first, verify per batch

The owner's messages of 2026-09-27 (evening) and 2026-09-28 are in [finer/intent/2026-09-28.md](../../finer/intent/2026-09-28.md).

- **The documents.** Progress is a one-line-per-task list ([index.md](index.md)), the plan is [plan.md](plan.md) with no status, and the batch records are here.
- **Amendment AM1** (architecture, appendix A):
  - Budgets are counts: the prefix length, the number of exact judgements and the decision floor.
  - Dropped: the work-unit currency, its tariff, the exchange rate and the per-level work ceilings.
  - Deferred: lockstep objectives with checkpoint offers, the gzip shadow and the speculative parallel greedy. Each returns only by amendment, with its bytes per exact judgement measured on the calibration corpus.
  - M3.5 is slimmed and no longer waits for M2.15, and step 6 is re-estimated at 1–2 batches.
- **Law L22, decide statically first.** The language is typed and closed-world by design. Rules decide whatever types, the checker and program facts can decide, and the codec judges only representations whose value depends on the codec.
- **Verification rhythm** (plan rules 3 and 8): each change gets a build and the unit tests of the modules it touches, and the full ladder runs once per batch.
- **Implement the whole plan**, one heavy job at a time. The end result must be smaller under gzip, Brotli and raw.

The evidence behind AM1:
- exact Brotli-11 is 55–74% of every production compile;
- batches T and D bought −88..+7 bytes per port for 2.7× compile time;
- the search winner is byte-identical at levels 8, 13 and 15 on five of six ports.

## 2026-09-28 Batch F1: the floor slice, first part (edit kernel, DCE, exact values, folding)

**Pre-registration** (written before the first build of the batch; base `222ce5cb`, baseline binary `~/lilscript-work/bin/head-edf870ae`, which differs from the base by documentation only).

What the batch builds:
- **M5.1, the edit kernel and DCE.** Program rules run once per build on the owned program, after conversion and before any target, so JavaScript, native and every search candidate start from the optimized program (P*). They run at every level. The kernel removes and splices operations and regions, substitutes values, compacts a unit's arenas, and edits the cells table. DCE removes unused operations whose evaluation is not required, dead stores to write-only locals, and named functions nothing reads; the native plan skips units nothing creates.
- **M6.4a, exact values.** Constants; loads of cells that are initialized once, never reassigned and read after initialization; parameters joined over complete call sets; and call results. Root cells count only under module sealing.
- **M7.8a, literal and branch folding.** A removable operation with an exact result becomes a constant. `If`, `?:`, short circuits and loops with exact conditions keep only the code that runs.

Predicted, per lane:
- `comparison/cases`:
  - `control/dead-branch` from 54 bytes to about 15 raw;
  - the loss count against the smallest competitor falls (439 on Brotli today);
  - no case grows beyond the seeded noise band.
- `comparison/apps`:
  - `optimizer-pressure` from 357 raw bytes to about 250;
  - the apps total falls from 945 Brotli.
- Reference ports: 0 to −300 Brotli each (constant conditions are rare in ports), no port larger beyond noise; gzip and raw move the same way.
- Compile time: the rule phase adds 1–5% at level 13.
- Runtime: neutral or better, since the programs execute fewer operations.

Not in this batch: M7.3 and M7.5a (F2). `drop_unreferenced_functions` stays as a transitional rule, because the tree inliners still create unreferenced functions until M7.5a and the inline-or-share choice replace them.

**What landed** (M5.1's first part, M6.4a's exact tier, M7.8a; branch `m5-floor` on `222ce5cb`).
- **The rule phase** (`src/program/rules/mod.rs`). Program rules run once per build, after conversion and before any target, at every level, to a fixed point:
  - The contract permits them through the tactics `constant-folding` and `dead-code-elimination`, and seals root storage only under module execution: a script's root bindings are globals.
  - The build receipt records them as `phases.rules`.
  - Reaching the round ceiling (64) is a compiler bug and fails the build.
  - The program is verified after the phase in every build, and after every round in debug builds.
  - The retained-bytes ledger is re-accounted around the phase (`publication::program_retained_bytes`).
- **The edit kernel** (`rules/edit.rs`). An `Editor` copies a shared unit once, stamps each committed unit with a fresh revision, and drops the derived views. Its edits:
  - `detach`, `make_constant`, `make_block`, `substitute` and `splice`;
  - `compact`, which rebuilds a unit's arenas from what its entry region owns; a cell whose region was removed moves to the nearest region that remains.
- **Exact values** (`rules/values.rs`). A lattice (⊥, exact, ⊤) over values, formals and results:
  - formals are joined over complete call sets (`CallGraph::complete_callers`);
  - a cell is settled when it is local, never reassigned, not synthetic, of primitive type and initialized once, and a root cell counts only when sealed; its loads count only where initialization order proves them past its initializer;
  - evaluation goes through the language's own `facts::exact`, with bounded work.
- **Folding** (`rules/fold.rs`, M7.8a):
  - An operation with an exact result, whose evaluation is not required, becomes that constant.
  - `if`, `?:`, short circuits and loops with exact conditions keep only the code that runs.
  - The guard (L3): the constant's text is at most a lower bound of the replaced expression's text in any output. A literal the program spells once counts at its text; anything else counts as one character, since the target may name a repeated literal, a load is a name, and a nested expression may be named.
  - The batch's first binary counted exact operands at their literal text. It grew `-2147483647-1` into `-2147483648` where the raw objective had named the repeated literal: `integer_multiplication` +7 raw and `typed_arrays` +9. This was fixed within the batch, with a test (`a_fold_counts_what_the_output_may_name_at_one_character`).
  - Integers take the shortest numeral, as the printer spells them.
- **Dead code** (`rules/dce.rs`):
  - operations nothing reads, whose evaluation is not required, go with their call preparations;
  - stores to unread locals go, past initialization;
  - named functions whose cells nothing reads leave the module prefix, and bodies nothing creates are emptied, captures included;
  - visible cells are exports, `import()` namespaces, class values and published methods, and a script's root bindings.
- **Native plans only the code that exists** (`native_plan.rs`, `native.rs`, `native_ownership.rs`):
  - uncreated units are neither declared nor written;
  - the fall-through check covers created units only;
  - an unused local callable cell no longer pulls in the closure runtime.
- **Wiring** (`src/build.rs`): `Frontend::rules()`, on both the source and the path front end.
- **Tests:** 11 rule tests. Each program is converted, optimized, verified and run in Node against the interpreter's output. The formation test that inspects an unfolded `3+4` turns the rules off through a test switch.

**Evidence (binary `~/lilscript-work/bin/f1-6`, SHA-256 `4f9d0ecd76e446f6…`, against `head-edf870ae` and the A1 reports of `~/lilscript-work/out/m2.5`).**
- Unit tests: 1,564 pass, 9 ignored.
- Case runner, 395 cases × 18 lanes: no failure outside the ledger.
  - The formation-only JavaScript lanes are byte-identical, since the rules are off there.
  - Production:

    | Lane | Brotli | gzip | raw |
    |---|---:|---:|---:|
    | module | 38,731 → 38,155 | 47,019 → 46,468 | 68,757 → 67,881 |
    | script | 39,710 → 39,617 | 47,665 → 47,576 | 69,237 → 69,045 |

  - C: production 2,215,237 → 2,140,652 bytes (−74,585); formation-only 2,173,655 (−41,582, from native planning alone).
  - No case grows in raw or gzip. Two grow in production Brotli module (+7 and +6), from the search's tie-break (findings).
- Ratchet: pass, with 230 improvements.
  - `comparison/cases` Brotli 49,170 → 48,746. Losses to the competitor: Brotli 439 → 423, gzip 400 → 378, raw 399 → 375.
  - Apps Brotli 945 → 926; raw losses to the competitor 1 → 0.
  - Algorithms Brotli 3,250 → 3,235; raw losses to Closure 11 → 10.
  - `control/dead-branch` 54 → 15 raw (bar 14). `wins/optimizer-pressure` 357 → 322 raw as a script, and 296 → 235 as a module.
- Reference ports: all seven green with identical tests.
  - Over all artifacts: raw −15,908, gzip −1,664, Brotli −2,084.
  - Per port (raw/gzip/Brotli): katexlil −13,620/−1,174/−1,132; posthoglil −1,659/−376/−257; micromarklil −311/−99/−172; motionlil −318/−15/−523.
  - markedlil, zodlil and jquerylil are byte-identical.
- CPU time at level 13, median of three alternating pairs after a warm-up (`cpu-pairs.py`, M2.14's frozen entries): markedlil ×1.025, zodlil ×1.027, posthoglil ×1.016, micromarklil ×1.024, katexlil ×1.000, jquerylil ×1.006, and motionlil ×0.658 (15.69 → 10.32 s): the search starts from less code.
- Against the prediction:
  - `dead-branch` reached the predicted 15.
  - The loss count fell.
  - `optimizer-pressure` reached 322, not about 250, because a script's roots stay global (it is 235 as a module).
  - The apps total fell.
  - katexlil exceeded the per-port prediction.
  - No port grew.
  - The rule phase costs what was predicted.

**Findings.**
- **The search scores a text it does not deliver.** The terminal stage re-spells loops after selection. The scoped and global naming plans tied at 111 Brotli and ended at 95 and 107, depending on which one the tie-break kept; `Style`'s derived order prefers `Global` on a tie. Owner: M9.1 and M9.5. The choice procedure judges delivered text, and a tie keeps the seed.
- **The script lanes cannot seal their roots.** `comparison/cases` compile as scripts, whose root bindings are globals, while every competitor recipe treats top-level bindings as private (`toplevel: true`, or an IIFE). `functions/nested-local` folds to the bar, `console.log(16)`, as a module, but stays at 47 raw as a script. This is Y5.
- **motionlil's `dist/internal/graph` parts** are esbuild's unminified reprints of the compiler's output. A shorter nested form (`if(!c){…}` for `if(c)return;…`) prints longer there, so the parts move with the compiler's choices, not with its bytes.

**Deviations.**
- M5.1 lands its first part: remove, splice, substitute, compact and cell-table edits. Clone, delete and merge units, change signature, retype and the incremental `UseIndex` come with the rules that need them (F2 grafts and changes signatures). Units are emptied, not deleted, and `drop_unreferenced_functions` stays until M7.5a.
- M6.4a lands its exact tier. The finite-set tier and deleting `simplify::known`'s constant cases stay open.
- The search-era rewrites `fold_literal_int_binary` and `drop_dead_value` stay for their lineage tests; production reaches them only for what the rules left.

**Open.** M7.3 and M7.5a (F2); M6.4a's finite-set tier and `simplify::known`; the two findings (M9.1 and M9.5; Y5); folding what tree inlining exposes (`1+1|0`), which is M7.8's.


## 2026-09-28 Batch F2: the floor slice, second part (graft, removal-only inlining, typed defaults)

**Pre-registration** (written before the first build of the batch; base `e5bed423`, baseline binary `~/lilscript-work/bin/f1-6`).

What the batch builds:
- **M5.1, the kernel's graft.** One unit's body is copied into another: its regions, operations, values, places, calls and instantiations. The cells it declares are cloned into the receiving unit as synthetic cells, owned where the copy stands. The captures the copy needs are added along the receiver's creation chain.
- **M7.5a, removal-only inlining.** A body whose call set is complete is inlined at every call and then retired, when the program's operation count does not grow. Each round inlines an independent set of bodies, callees first.
  - Legality (v1): not recursive, not suspending, not a constructor; one exit (its only `return` ends its entry region, or it has none); no function created inside; no ambient `this` or `arguments`; value parameters only; no generic instantiation; caller and body in the same module.
  - A body that is an expression may go anywhere. A body with statements goes only into a statement region, where no pending value crosses the call.
  - A parameter the body never writes reads its argument directly; the others become local cells.
- **M7.3, first rule: typed defaults.** When every caller is known and a parameter's type excludes `undefined`, its default never runs: `IsUndefined` of that parameter is false, and the default's branch folds.
- **Storage facts.** The rules read whether a cell is written from the current program (`CellStorage`), not from conversion's `reassigned` flag, which goes stale as rules remove stores.

Predicted:
- `comparison/cases` module lanes: `functions/*` cases shrink, and the loss count against the competitor falls further. The script lanes barely move, since their root functions are globals (Y5).
- Reference ports: 0 to −500 Brotli each. Single-call helpers and immediately invoked arrows leave, but formation's tree inliners already remove many of them, so the gain is smaller than the rule's reach. No port grows beyond noise; gzip and raw move the same way.
- `comparison/apps` and `algorithms` (script lanes): nearly unchanged (Y5).
- C: smaller, with fewer functions and closures.
- Compile time: neutral or faster, since the search sees fewer units. Runtime: neutral or better, with fewer calls.

Not in this batch: M7.3's signature edits (dropped and constant parameters, unused results) and deleting the tree twins, which is batch F3.

**What landed** (M5.1's graft, M7.5a's first version, M7.3's first rule; branch `m5-floor` on `c69ad2f0`).
- **The kernel's graft** (`rules/edit.rs`). One unit's body is copied into another:
  - `ownership`, the walk compaction uses, decides what is copied;
  - `graft_regions` places the copied regions, so cloned cells know their regions before the copy is made;
  - `Remaps` maps cells, forwarded parameters and allocation sites (renumbered past the receiver's own, since a site's identity is local to its unit);
  - `Editor::add_cell` and `Editor::handle` let a body be read while another unit is edited.
- **Removal-only inlining** (`rules/inline.rs`, M7.5a). Legality:
  - every call is known, direct and in the same module, with value arguments and no generic instantiation;
  - the body is not recursive, does not suspend, is no constructor, creates no function and reads no ambient `this` or `arguments`, and has one exit.

  How much it copies:
  - A body with one call moves.
  - A body with several calls is copied only where each copy is free: an expression that reads each argument at most once, in no more operations than the call. A larger duplicate is the inline-or-share choice (M9.1).

  Where a copy may stand:
  - An expression stands anywhere when it reads its arguments in their evaluation order with nothing observable before the last read. This is the tree inliner's condition, widened to typed arithmetic when every argument is a constant or a cell's value.
  - A body with statements stands only in a statement region, outside other calls' arguments and with no value waiting across the call.

  Parameters and scope:
  - A parameter nothing writes, whose argument has its exact type, reads the argument. Every other cell is cloned.
  - A copy that declares storage of an owned type keeps its own block: native releases owned storage at scope end. The call's argument evaluation moves into that block. Such a copy cannot yield a value.
  - Each round inlines an independent set of bodies, callees first. A copy's captures are added along the receiver's creation chain.
- **Typed defaults** (M7.3's first rule, `rules/values.rs`). When every caller is known and passes an argument in the parameter's position, and the type excludes `undefined`, `IsUndefined` of the parameter is false, and its default's branch folds. Callers omit only trailing arrow defaults, which the callee builds.
- **Storage facts.** Whether a cell is written is read from the current program (`CellStorage`), not from conversion's `reassigned` flag.
- **Tests and receipt:**
  - The round ceiling is 128, and the receipt counts `inlined_calls` and `inlined_bodies`.
  - 14 new rule tests.
  - Four tests adjusted:
    - the budget fixture's helper now has two calls;
    - the `JS.call` test checks that the receiver's effect survives: its helper is inlined, and the call becomes plain;
    - formation's helper-inlining test sets a test-only `SKIP_PROGRAM_INLINING`, since its fixture's helpers are what that machinery (retired by M9.1) qualifies;
    - F1's effects test runs F1's rules alone.

**What the ladder caught** (ten binaries, `f2-1` to `f2-10`; each fixed within the batch):
- **A callee-built default.** Callers omit a trailing arrow default and the callee builds it. The typed-default fold must therefore require the argument (`callable_defaults`: a wrong program in `f2-1`).
- **Exact types.** `CopyValue` needs identical types, so forwarding and substituted results need exact types (two cases refused).
- **Statement sites.** Formation cannot place statements inside another call's arguments or across a waiting value, and a parameter's cell is a statement (`35_nullable`).
- **Duplicates.** A copy that reads an argument twice needs a temporary per copy (`ir_inlining_variant` +68 raw), so several calls take free copies only.
- **Evaluation order.** An expression that evaluates something observable before its last argument read makes the target hold the arguments in temporaries (`dynamic_js_coercion` +18).
- **Scope.** Copying a body's locals into the module scope kept native storage alive to the end of the program; the `factories` native test counted three live owners. Hence scoped copies. Scalars and host values own nothing a scope releases.

**Evidence (binary `~/lilscript-work/bin/f2-10`, SHA-256 `4a7d57ee4b87bfaf…`, against F1's `f1-6`).**
- Unit tests: 1,578 pass, 9 ignored.
- Case runner, 395 cases × 18 lanes: no failure outside the ledger. Production, against F1 (in brackets, against the A1 baseline, both batches together):

  | Lane | Brotli | gzip | raw |
  |---|---:|---:|---:|
  | module | −598 (−1,174) | −592 (−1,143) | −1,150 (−2,026) |
  | script | −223 (−316) | −218 (−307) | −355 (−547) |

  - C: production 2,140,652 → 2,103,391 bytes (−37,261; −111,846 since A1).
  - Five cases grow in raw module (+21 in total), all from struct copies (findings).
- Ratchet: pass, with 271 improvements.
  - `comparison/cases` Brotli 48,746 → 48,725. Losses to the competitor: raw 375 → 368, gzip 378 → 372.
  - Apps and algorithms are unchanged: they compile as scripts, whose root functions are globals (Y5).
- Reference ports: all seven green, with identical tests.
  - Against F1: Brotli −696, gzip −316, raw +10,820. Against A1: Brotli −2,540, gzip −1,749, raw −4,139.
  - Per port against F1 (raw/gzip/Brotli): markedlil −1,245/−228/−140; micromarklil −107/−55/−133; katexlil +4,625/+187/−270; jquerylil +5,950/+91/−234; zodlil +1,522/+168/+12; posthoglil +46/+16/+9; motionlil +29/−495/+60.
  - The raw growth is in Brotli-objective builds: jquerylil's (all its artifacts use `cost_model = "brotli"`), katexlil's `.closed/.esm/.mjs` and zodlil's `index.cjs`. After inlining, the search prefers `if(c)e` to `c&&e` there, and Brotli falls (jquerylil's `jquery.raw.js` +1,190 raw, −23 Brotli). Builds made for raw shrink (marked's, katex's `.min.js`).
- CPU time at level 13 against F1 (`cpu-pairs.py`, the frozen entries): markedlil ×0.696, zodlil ×0.975, posthoglil ×1.021, micromarklil ×0.990, katexlil ×0.997, motionlil ×0.989, and jquerylil ×1.098, where the search lands on a different result (its frozen entry 76,673 → 76,478 raw).

**Findings.**
- **M7.9's residual growth.** A parameter written through a field is a cell, which formation prints as `{let e=d;…}` where the tree inliner read the argument (+9 raw). A struct copy made for an argument makes formation hold the waiting operand (`interprocedural_values` +6).
- **Tail returns** need a result declared without a value, which the IR has no form for. So formation's block inliner (`src/js/blocks.rs`), which spells `let x;…x=v`, stays until M7.5's rest.

**Deviations.**
- M7.5a is a first version: one exit, no function created inside, one module.
- The tree inliners' removing half, `drop_unreferenced_functions` and the limit 6 are not deleted. The program rule does not yet cover tail returns or the helpers formation creates, so deleting them is batch F3's measured decision, or M7.5's rest.
- M7.3 lands only its typed-default rule. Its signature edits and deleting `drop_typed_default_checks`, `drop_default_arguments` and `native_default_lengths` are batch F3.

**Open.** Batch F3 (M7.3's signature edits and the tree twins' deletion, measured); M7.9's two growths; M7.5's rest (tail returns, closures inside bodies, across modules).

## 2026-09-28 Batch F3: the floor slice, third part (parameters and returns, the tree twins)

**Pre-registration** (written before the first build of the batch; base `996713a6`, baseline binary `~/lilscript-work/bin/f2-10`).

What the batch builds:
- **M7.3, parameters and returns**, on bodies whose calls are all known and direct:
  - a parameter the body never reads leaves the signature and every call, and the call still evaluates its argument;
  - a parameter every call passes the same exact value becomes that constant, where the literal costs no more than the arguments it removes, and then leaves;
  - a result no call uses leaves: the body returns nothing, and its calls yield nothing.
  - Exclusions: ambient `arguments`, reference parameters, generic instantiations, defaults that name another parameter, and a callee value that reaches its calls through a copy.
- **M5.1, signature edits:** a function type added to the program's type table (interned by equality); parameter cells renumbered; the holding cell, the callee values and the call contracts retyped; argument lists rebuilt.
- **The tree twins, measured.** A binary with `drop_typed_default_checks`, `drop_default_arguments`, `native_default_lengths` and the removing half of the tree inliners turned off is compared byte for byte with one that runs them, on the case corpus and the reference ports. What changes nothing is deleted. What still acts stays, with its owner and its measured effect.

Predicted:
- Module lanes: −100 to −400 Brotli from unused parameters and results. Script lanes and apps barely move (Y5).
- Reference ports: 0 to −300 Brotli each. No port grows beyond noise.
- The twins: `drop_typed_default_checks` changes nothing after F2's typed defaults. The others may still act where callers omit a callee-built default.
- Compile time: neutral.

**What landed** (M7.3's signature edits, with M5.1's; branch `m5-floor` on `ce260fde`).
- **Parameters and returns** (`rules/params.rs`), on bodies whose calls are all known and direct:
  - a parameter the body never reads leaves the signature and every call, and the call still evaluates its argument;
  - a parameter every call passes one exact value becomes that constant where it is read, when the literal costs no more than the arguments it removes;
  - a result no call uses leaves: the body returns nothing, and its calls' results are `void`.
- **A signature is a coupling class** (architecture §9.5). The functions that share one print one shape, which a codec compresses. The batch's first binary dropped parameters one function at a time: markedlil's handlers of one signature, where only some read their first parameter, measured +300 Brotli, and posthoglil +106. Now:
  - a position leaves only when no function of the signature reads it;
  - a result leaves only when no function's calls use it;
  - a constant stands for a parameter only where one function has the signature to itself.
- **The kernel:**
  - `Editor::intern_type` adds a function type to the program's table, interned by equality;
  - signatures, parameter cells, the holding cell, callee values and call contracts are retyped, and argument lists rebuilt;
  - compaction now also drops operand and argument slots an edit orphaned.
- **The tree twins stay** (measured). A binary with `drop_typed_default_checks`, `drop_default_arguments` and `native_default_lengths` off, against one with them on, over the case corpus:
  - script lanes: 9 cases grow, +113 Brotli, +162 gzip and +341 raw;
  - module lanes: 5 cases grow, +32 Brotli.

  They still act where the program rules do not reach: a script's global roots, and defaults the callee builds. They go with Y5 and M7.3's remaining cases.
- **Tests:** 5 new rule tests.

**Evidence (binary `~/lilscript-work/bin/f3-2`, against F2's `f2-10`).**
- Unit tests: 1,582 pass, 9 ignored, after a comment named a port (NO3; the check caught it).
- Case runner: no failure outside the ledger. Production, against F2 (in brackets, against the A1 baseline, all three batches):

  | Lane | Brotli | gzip | raw |
  |---|---:|---:|---:|
  | module | −96 (−1,270) | −137 (−1,280) | −158 (−2,184) |
  | script | 0 (−316) | 0 (−307) | 0 (−547) |

  - C: −2,748 bytes.
  - No case grows in raw.
- Ratchet: pass, with the same totals as F2. Its cases are scripts, whose root functions are globals (Y5).
- Reference ports: all seven green.
  - Against F2: raw −3,487, gzip −367, Brotli −522 (zodlil −190 Brotli, motionlil −332); markedlil, katexlil, jquerylil, posthoglil and micromarklil are byte-identical.
  - **Against A1, the three batches together: raw −7,858, gzip −2,059, Brotli −3,050**, and every port is smaller in Brotli.
- CPU time at level 13 against F2: markedlil ×1.011, zodlil ×0.949, posthoglil ×1.018, micromarklil ×1.018, katexlil ×1.009, jquerylil ×1.001, motionlil ×1.046.

**Deviations.** Constant parameters apply only to functions whose signature is their own. Class-wide constants, where every member passes the same value, are M9's coupled choices. The tree twins stay, as measured above.

**Open.** Step 5 is done except its carried items: the twins, which go with Y5 and M7.3's remaining cases; M7.9's two growths; M7.5's rest.

## 2026-09-28 Batch B1: the counted walk (M3.5, first part)

**Pre-registration** (written before the first build of the batch; base `e0f03613`, baseline binary `~/lilscript-work/bin/f3-2`).

Measured before the batch, at level 13 on M2.14's frozen entries:
- **katexlil** takes 17.8 s of CPU, of which 32 exact Brotli-11 scores take 12.9 s. Its terminal stage judges 26 moves exactly and buys −1,210; five of the rejected moves lose by more than 1,000 bytes.
- **The structural beam** buys 0 bytes on markedlil, −47 Brotli on katexlil for 3.2 s, and −13 Brotli on jquerylil for 22.6 s. On jquerylil it also costs +543 raw.

What the batch builds (architecture §9.6, §13.3–§13.4, B1–B4, B9):
- **The proxy judge.** Brotli at quality 5 on the whole candidate, and gzip and raw as their own proxies. A move whose proxy delta exceeds the margin M is pruned without an exact judgement. The proxy never keeps.
- **The effort schedule, version 1.** Each level sets the prefix p(L), the exact budget e(L) and whether the beam is reached:
  - level 0: no moves;
  - levels 1–4 and 5–9: short prefixes;
  - levels 10–13: every unreserved move, with e(13) calibrated so no reference port or case loses bytes;
  - levels 14–16: unbounded, with the beam.

  The seven ladders stop deciding anything. The port-level search keys (`candidate_*`, `terminal_codec_probe_limit`) warn that they have no effect, except `candidate_search = "off"`, which keeps its meaning: no walk and no beam.
- **One walk from A0.** Each objective's list, in a fixed order that does not depend on the level: the choice sites of A0 by stake, the declared challengers, then the naming seeds (global, source) as whole-artifact moves. The walk examines at most p(L) positions and judges at most e(L) exactly, keeping a move only on a strict exact win.
- **The beam as one reserved late move.** From level 14, structural exploration runs at its level-13 schedule, forming each recipe with the incumbent's spelling, choices and naming. A beam artifact replaces the incumbent only on a strict exact win, so level 14 passes through level 13's result.
- **Receipts.** For each objective: positions examined, exact judgements, prunings, and each move's proxy and exact deltas. `--print-policy` prints p, e, M and the beam tier.

Predicted:
- Level 13 CPU: katexlil 17.8 s to 8–12 s, jquerylil 24.6 s to 2–4 s, markedlil 0.83 s to about 0.5 s.
- Level-13 bytes: equal to F3's on the reference ports within noise. jquerylil and katexlil lose the beam's −13 and −47, within the ±100 band.
- Monotone: size(L+1) ≤ size(L) across the tier boundaries on the case corpus and the ports, checked by a new test script.
- Levels 14–15 at or below level 13 by construction.

Not in this batch: level 0 without a codec, codec settings as configuration, parallel scoring (B2); schema v3 and the public API (B3).

**Landed** (binary `~/lilscript-work/bin/b1-6`, SHA-256 `d57286084191a7eb…`):
- **The proxy judge.** `compression::measure_proxy_admitted` runs Brotli at quality 5, once per artifact (`CachedSizes.brotli_proxy`), timed in its own bucket (`proxy_brotli`); gzip and raw are their own proxies, and `ArtifactArena::proxy_is_exact` says when a proxy measurement is an exact probe.
- **The effort schedule, version 1** (`compilation_policy::WalkSchedule`, in the policy's fingerprint and receipt; an unbounded count prints as null):

  | Levels | p(L) | e(L) | passes | tail |
  |---|---|---|---|---|
  | 0 | 0 | 0 | 0 | no |
  | 1–4 | 8 | 2 | 1 | no |
  | 5–9 | 24 | 4 | 1 | no |
  | 10, 11, 12, 13 | all | 6, 8, 12, 16 | 1 | no |
  | 14–16 | all | all | to the fixed point | yes |

  The margin M is 150 bytes of proxy for Brotli and 0 for gzip and raw. The seven ladders and their fields are gone; the five search keys warn (`RETIRED_KEYS`), and `candidate_search = "off"` keeps its meaning.
- **The walk** (`program/search_terminal.rs`, a `Walker` over one list):
  - The list is the choice sites by stake, the declared challengers, then the joint moves (the other literal spelling, the naming seeds). A pass walks it once; a pass that keeps a move is followed by another, up to the level's number.
  - The walk holds its incumbent itself: it retains and qualifies each kept artifact, discards the one it replaces, and promotes the last one into the portfolio once.
  - The tail (from level 14) restarts the walk from A0 under each other naming seed. A restart's start is pruned by the proxy against A0, and its result replaces the best only on a strict exact win. Then the structural beam runs as the last move, seeded with the incumbent's tactics and naming (`BeamSeed`), for one objective (a build with several skips it, with the reason in the receipt).
  - `search_javascript` forms A0 only. The machinery tests reach the old exploring flow through the test-only `search_javascript_explored`.
- **Receipts.** Per objective: the schedule, positions examined, exact judgements, prunings, passes, and each move's pass, proxy delta and exact delta; restarts with their start and result. `--explain human` prints a `walk` line and a `beam` line, and `--print-policy` the schedule.
- **Tools.** `scripts/monotone.mjs` (the case runner at each tier boundary, failing on any growth from one level to the next) and `scripts/cases.mjs --level N`; [testing.md](../testing.md#the-effort-schedules-monotonicity-m35).
- **Tests.** 1,582 unit tests pass. The terminal build tests check the walk's arithmetic per pass and per restart, the schedule per level, and monotonicity from level 1 to 15. The beam's own shape keys stay readable by direct deserialization for the beam's unit tests only; they go with the beam (M9.1).
- **NO3.** `src/config.rs`'s two entries (acorn, jquery; owner M3.5) are gone with the ladders.

**Calibration** (`~/lilscript-work/tools/walk-calibrate.py` on M2.14's frozen entries; the unbounded one-pass walk against the exact budget):

| Port | A0 | walk | final kept by judgement | F3 level 13 |
|---|---:|---:|---:|---:|
| markedlil | 9,201 | 9,197 | 1st | 9,197 |
| zodlil | 26,886 | 26,840 | 1st | 26,840 |
| posthoglil | 5,340 | 5,244 | 4th | 5,244 |
| micromarklil | 21,633 | 21,614 | 3rd | 21,614 |
| katexlil | 61,346 | 60,166 | 14th | 60,119 |
| jquerylil | 25,499 | 25,416 | 7th | 25,403 |
| motionlil | 34,140 | 33,984 | 6th | 34,070 |

- e(13) = 16 holds every port's unbounded result.
- With no pruning at all (a calibration build), every port reaches the same sizes. M = 150 prunes 4 to 6 moves per port and loses nothing.
- katexlil needs 14 judgements; at e = 8 it would stop at 60,302.

**What the ladder caught.**
- **One greedy order is path-dependent.** With the joint moves last and one pass (`b1-2`), five ratchet items lost 1 to 6 bytes (nested structs, `host-hasown`, `edge-loop-control`, `host-callable-as-value`): F3's search explored the naming seeds before its challengers, and a naming seed that ties or loses alone can win with the families it enables. The joint moves first (`b1-3`) fixed those and broke ten others (Brotli −69 against −111 for the corpus). Passes to the fixed point (`b1-4`) fixed one more. The tail's restarts (`b1-5`) fixed the rest: the ratchet passes, and no item grows.
- **Test configurations with retired keys.** Direct deserialization rejected them once the fields went; they are readable again for the beam's tests only.

**Evidence** (`b1-6` against F3's `f3-2`):
- **Case runner**, 395 cases × 18 lanes: no failure outside the ledger. At level 13 against F3:

  | Lane | Brotli | gzip | raw |
  |---|---:|---:|---:|
  | module | +122 | +183 | +176 |
  | script | +5 | +117 | +128 |

  These are the structural recipes the beam found on small programs, which level 13 no longer reaches: scalar replacement of records and structs (`irjs-record_literals…`, the `folds_redundant_record_miss…` pair, `17_struct`), inlining in `16_templates` and `modules/main`. The data tables gain (−555 Brotli). C is unchanged.
- **Monotone** (`scripts/monotone.mjs`, levels 0, 1, 5, 10–14, production module lanes): no growth at any step. Brotli 38,703 (level 0) → 37,819 (1) → 37,775 (5) → 37,455 (10) → 37,454 (11–13) → 37,194 (14); gzip and raw fall the same way.
- **Ratchet:** pass, with 439 improvements; the baseline is tightened to this binary (`tests/ratchet/baseline.json`). Against the baseline before the batch (at level 15 configurations): cases Brotli 48,725 → 48,573, gzip 58,550 → 58,522; apps Brotli 926 → 920; algorithms Brotli 3,235 → 3,233.
- **Reference ports** (`scripts/ports.mjs --patches none`): all seven green. Delivered files against F3: raw −2,342, gzip −62, Brotli +675.
  - markedlil and zodlil are identical.
  - jquerylil: raw −2,890, Brotli −7. micromarklil −22 Brotli. posthoglil +2.
  - katexlil +406 Brotli over its 26 files. Its compiler-built artifacts (`.esm.js`, `.mjs`, `.closed.js`) grow 8 each (60,085); the esbuild re-bundles of them (`.cjs` +130, `.min.js` and `.umd.js` +47) and the test builds (+107, +47) grow more.
  - motionlil +296 over its 621 graph files, whose part numbering shifted.
- **CPU time** at level 13 (`cpu-pairs.py`, median of three alternating pairs): markedlil ×0.789 (0.87 → 0.68 s), zodlil ×1.043, posthoglil ×0.821, micromarklil ×0.766, katexlil ×0.577 (18.09 → 10.44 s), jquerylil ×0.081 (24.82 → 2.01 s), motionlil ×0.492 (10.80 → 5.31 s). jquerylil and motionlil are now below their frozen pre-M1 times; katexlil is at 2.26× its 4.62 s.

**Deviations.**
- The tail comes from level 14, as pre-registered, not at 16 as the architecture's starting table has it; the restarts are the tail's new first moves.
- The beam serves one objective; a build with several objectives skips it (the product always builds one).
- The beam's shape keys stay readable by direct deserialization for its unit tests.

**Known cost, ledgered** (M3.5's size exit allows it with an owner; superseded the same day by batch B1b under amendment AM2, which runs the structural search at level 13 again): level 13 no longer reaches the beam's structural recipes. The case corpus pays +122 Brotli in the module lane, owned by M7.9 (scalar replacement) and M7.5's rest (inlining), and katexlil +8 on its compiler-built artifacts, owned by M9.1's rest. Level 14 recovers all of it.

**Open.**
- The time exit: katexlil at level 13 is 10.4 s against the 6.0 s ceiling. Formation is most of the rest after the exact judgements (M5.7), and the list order could put katexlil's late winners earlier.
- The replay check of §9.6 (the incumbent at each lower level's stopping point against a build at that level).
- An auditing lane that measures what pruning misses.
- The per-batch count gates in the runners.
- Level 0 without a codec, codec settings as configuration, parallel scoring (B2).

## 2026-09-28 Batch B1b: the upper levels try more (M3.5, AM2)

**Pre-registration** (written before the first build of the batch; base `e4a44a7b`, baseline binaries `~/lilscript-work/bin/f3-2` for the owner's comparison and `bin/b1-6` for the ratchet's).

The owner's ruling of the same day (amendment AM2, [intent](../../finer/intent/2026-09-28.md)): size first, compile time second, the level trades them. Level 13 and above must not deliver more bytes than before a change, and from 13 up each level tries more. B1 broke the first part: level 13 no longer reached the structural search, and grew by +122 Brotli on the case corpus's module lane and +675 over the reference ports' files (katexlil +406, motionlil +296); most ports ship at level 15, where their retired search keys had asked for wider searches than B1's constants.

What the batch builds:
- **Schedule version 2.** Levels 0–12 keep version 1: one pass from A0, e = 2, 4, 6, 8, 12. From level 13, every walk is unbounded and runs in passes to its fixed point. The structural search runs from level 13, with a budget (proposals, retained candidates and bytes, beam width, codec probes) that grows with the level:
  - 13: (1,024, 1,024, 768 KB, 10, 1,536);
  - 14: (1,024, 1,024, 896 KB, 11, 1,536);
  - 15: (1,536, 1,536, 16 MB, 24, 1,536);
  - 16: (4,096, 4,096, 64 MB, 32, 4,096).

  At each level this is at least what F3 searched there for the reference ports, with their now-retired keys: katexlil's level-13 `always` tier, and jquerylil's and posthoglil's level-15 widths and byte budgets.
- **Several starts from level 13.** The search forms A0 and runs the structural search from the policy's default tactics, as F3 did; A0 stays pinned. The starts, in order:
  1. the search's winner W, when it is not A0;
  2. A0;
  3. A0 under each other naming seed (the restarts, pruned by the proxy against A0);
  4. from level 14, the beam from the incumbent's tactics (B1's tail move), whose winner is walked when it wins.

  Each start is walked in passes to its fixed point. A walk's result replaces the objective's winner only on a strict exact win.
- **The judged-assignment memo.** Within one start, the walk records each assignment it judged (spelling, choices, naming, literals), with its exact or proxy size and verdict. A repeated assignment is not formed again: a measured one is rejected when it is not smaller than the incumbent, a pruned one is pruned while its recorded proxy exceeds the margin, and a refused or identical one repeats its verdict. Repeats occur only across passes, where no budget binds, so no decision changes.
- **Receipts.** Every start with its name, first pass, start size, result and verdict; the schedule with the structural budget.

Predicted:
- **Level 13 and above at or below F3.** On the case corpus (every production lane), the ratchet (level 15, against `b1-6`'s baseline) and each reference port's delivered files in their own objective, apart from rare deviations of a byte or two. Level 13 holds F3's result as a candidate: the search's winner walked with F3's list order and no exact budget passes through F3's terminal stage. The proxy's pruning is the one exception, and it lost nothing in B1's calibration.
- **Monotone.** From 0 to 13 by construction: level 13 walks A0 past level 12's stopping point. From 13 to 16 on the ports and all but a few cases.
- **CPU time, reported (AM2).** Level 13 roughly doubles against F3 on katexlil (about 30–40 s against 18.1 s), because it walks both W and A0. Levels 1–12 keep B1's times.

**Landed** (binary `~/lilscript-work/bin/b1b-1`, SHA-256 `81b8dd261c3e505a…`):
- **Schedule version 2.** `WalkSchedule` sets p, e, M and the passes, plus `starts`: several starts from level 13. `StructuralSchedule` sets the structural search's budget for each level. Levels 1–12 are unchanged; from 13 every walk is unbounded and runs to its fixed point.
- **Several starts.** From the default level the search runs its structural exploration again, from the policy's own tactics. The level-0 artifact stays pinned in the portfolio (`Portfolio::pin_selected`, `unpin`) until the walks end. Each objective walks, in order:
  1. the search's winner, when it is not the level-0 artifact;
  2. the level-0 artifact;
  3. the level-0 artifact under each other naming seed.

  Each start is walked in formations of its own and settled against the objective's winner: a strict exact win is promoted, with its own source state (`promote_terminal` now takes it), and anything else is discarded.
- **The memo.** Within a start's formations, each judged assignment (spelling, choices, naming, literals) keeps its verdict. A repeat is `recalled` and not formed again.
- **B1's beam from the incumbent's tactics is gone.** It has no budget left once the structural search runs first, and the wider search replaces it above 13. `BeamSeed` and `BeamReport` go with it, and so does `challenge`'s observer argument.
- **Receipts.** `searched` (the search winner's size); `starts`, each with its name, first pass, start size, result and verdict; `recalled` moves. `--explain human` prints the starts kept.
- **Docs.** Amendment AM2 is in the architecture (§13.2, §13.4, appendix A) and in M3.5's exit in the plan. `scripts/monotone.mjs` checks levels 0 to 16.

**Evidence** (against F3's `f3-2` for the owner's comparison, and against B1's `b1-6` for the ratchet):
- **Unit tests:** 1,582 pass.
- **Case runner**, level 13 against F3. No case grows in any production lane:

  | Lane | Brotli | gzip | raw | cases smaller |
  |---|---:|---:|---:|---:|
  | module | −152 | −30 | −114 | 44 / 17 / 4 |
  | script | −179 | −40 | −106 | 48 / 24 / 4 |

  B1 had been +122 to +183 in the same lanes.
- **Monotone**, levels 0, 1, 5 and 10–16: no growth anywhere. Module-lane Brotli: 38,703 → 37,819 → 37,775 → 37,455 → 37,454 (11–12) → 37,180 (13–16). The level-13 total is below B1's level 14 (37,194). The small programs gain nothing from the wider searches of 14–16.
- **Ratchet:** pass, with no item growing against `b1-6`'s tightened baseline.
- **Frozen level-13 entries** against F3 (raw / gzip / Brotli):
  - markedlil, zodlil, posthoglil, micromarklil and katexlil are byte-identical (katexlil at 60,119, where B1 had 60,166);
  - jquerylil −630 / −50 / −30;
  - motionlil −319 / −63 / −87.
- **Reference ports:** all seven green. Delivered files against F3: raw −4,845, gzip −608, Brotli +133.
  - katexlil −2,482 / −398 / −136, where B1 had +406 Brotli. jquerylil −2,890 / −176 / −7. micromarklil −22 Brotli. marked and zod are identical, posthoglil +2.
  - The growths are in files the compiler does not judge. motionlil +296 over its 587 graph files, which `scripts/shared-graph.mjs` splits from the compiled `full` module with Terser's parser and reprints with esbuild; the split now makes one more file. The compiled module shrank (`full.bundle.js` −21, `motionlil.global.js` −31). The esbuild re-bundles of compiled output: katexlil's `.umd.js`, `.min.js` (+78 each) and `.cjs` (+40), jquerylil's `.umd.js` (+46) and motionlil's `full.cjs` (+49). posthoglil's `posthog.gzip.js` (+28 Brotli) is a gzip-objective build.
- **CPU time** at level 13 against F3 (reported, AM2): markedlil ×1.28, zodlil ×1.31, posthoglil ×1.17, micromarklil ×1.04, katexlil ×2.30 (18.1 → 41.6 s), jquerylil ×1.14, motionlil ×1.69.

**Findings.**
- On katexlil the walk from the search's winner reproduces F3's level 13 exactly. The level-0 start, the passes and the restarts find nothing more for their 23 s. They are the price of the construction: level 13 passes through level 12's result, and any start can win.
- The ports' derived files (esbuild re-bundles, motionlil's graph split) move with every change of the compiled module. M3.3e, M12.2 (compiler-written delivery) and M3.8a are where they become the compiler's own.

**Open.** B1's open items carry over: the replay check, the audit lane, the count gates, level 0 without a codec, and deterministic `-j` scoring. `-j` scoring waits for M5.6's resource accounting, because the codec's scratch memory is still ledger-charged, and a thread count must not change a memory refusal.

## 2026-09-28 Batch B2: level 0 without a codec, the replay check, the audit lane, the counts (M3.5)

**Pre-registration** (written before the first build of the batch; base `e5800392`, baseline binary `~/lilscript-work/bin/b1b-1`).

What the batch builds (architecture §9.6, §13.3–§13.4; plan M3.5):
- **Level 0 measures no codec.** When the walk is off (level 0, or `candidate_search = "off"` in development mode), the level-0 artifact is admitted on its raw bytes, and its gzip and Brotli sizes stay unmeasured. The receipt says so and shows no walk. The CLI measures them only when it writes a manifest, which needs exact sizes.
- **The replay check** (§9.6). The level-0 start's walk records the incumbent's size and SHA-256 wherever a lower one-pass level would have stopped (levels 0–12), in the receipt's `stops`. `scripts/monotone.mjs` then checks, byte for byte, that each lower level's own build delivers the recorded stop. That is the monotone construction, tested directly.
- **The audit lane.** `LILSCRIPT_WALK_AUDIT=1`, a diagnostic that changes no output, also measures each pruned move exactly and records its exact delta. A pruned move whose exact delta is negative is a miss of the proxy.
- **Counts per batch** (§13.7 (a)). `scripts/cases.mjs` records each production artifact's walk counts: positions examined, exact judgements, prunings and starts. `--compare` prints them per lane against the previous report, with the bytes they bought (B9). `~/lilscript-work/tools/cpu-pairs.py` records them for the frozen entries.

Not in this batch:
- Deterministic `-j` scoring waits for M5.6. The codec's scratch memory is still charged to the ledger, and a thread count must not change a memory refusal.
- Codec settings as objective configuration go with M3.1's first slice (B3).

Predicted:
- No output byte changes at any level.
- Level 0 saves one Brotli-11 and one gzip encode: about 0.45 s of katexlil's ~1.9 s.
- The replay check passes on every case at every recorded stop.
- The audit lane finds no miss on the reference ports at level 13, as B1's no-prune calibration found.

**Landed** (binary `~/lilscript-work/bin/b2-2`, SHA-256 `59279e91cf4fbf94…`):
- **Level 0 measures no codec.** Without a walk (level 0, or `candidate_search = "off"`), the portfolio scores the level-0 artifact without a codec, and admission qualifies it on its raw bytes (`ArtifactArena::qualify_unmeasured`; `QualifiedArtifact::exact` is false). The gzip and Brotli sizes stay `None`: the receipt has no walk, `--explain` prints `-`, and the manifest reports `null`, as manifest v3 already does for a codec the objective never scored. On katexlil at level 0 with the Brotli objective the output is identical, and time falls from 1.52–1.56 s to 1.24–1.31 s.
- **The replay check.** The walk from the level-0 artifact records, in the receipt's `stops`, the size and SHA-256 of the incumbent where each lower one-pass level (0–12) would stop. It records them at the first position that level's prefix or exact budget would not examine, and otherwise at the end of the first pass. The case runner keeps each production build's counts and stops from `--explain json`. `scripts/monotone.mjs` checks every stop against its level's own build, and the unit test checks levels 0–15 of the terminal fixture. The walk's render bound is now the same at every level (`RENDER_BOUND`, 768 KB or the level-0 artifact's capacity); B1b had tied it to the structural budget.
- **The effort gates are gone.** The replay check's first run (`b2-1`) failed at level 5 on 77 case-lanes, 539 stops in all. `naming-search` needed effort 8 and `call-specialization` effort 11, so level 5 walked a list without the naming seeds that the levels above replay. Both gates are removed; effort is a work budget and grants no permission (§13.4, law B5). Level 16's startup-risk grant stays, by decision D5.
- **The audit lane.** `LILSCRIPT_WALK_AUDIT=1` also measures each pruned move exactly and records the delta in `audit`; the output does not change. `walk-calibrate.py` counts misses.
- **Counts per batch.** `--compare` prints each lane's exact judgements and examined positions against the previous report (B9), and `cpu-pairs.py` prints them for the frozen entries.

**Evidence:**
- Unit tests: 1,582 pass. The runner tests pass, with a new test for the explain parser.
- Case runner, `b2-1` and `b2-2` against `b1b-1`: no artifact changes in any of the 18 lanes. Level 13 is untouched.
- Monotone and replay (`b2-2`, levels 0, 1, 5 and 10–16): no growth at any step, and all 45,162 recorded stops replay byte for byte. Module-lane Brotli: 38,703 (level 0) → 37,819 (1) → 37,501 (5; 37,775 before the gates went) → 37,455 (10) → 37,454 (11–12) → 37,180 (13–16).
- Ratchet (`b2-1`): pass, no change.
- Reference ports (`b2-1`): all seven green.
- Frozen level-13 entries: bytes and exact-judgement counts are identical, and CPU is ×0.97–×1.02 against `b1b-1`. `b2-2` delivers katexlil's, markedlil's and jquerylil's level-13 entries byte-identical to `b2-1`. Level-13 exact judgements: markedlil 9, zodlil 9, posthoglil 11, micromarklil 11, katexlil 66, jquerylil 29, motionlil 31.
- Audit lane (level 14, every pruned move measured exactly): markedlil 4 audited, zodlil 6, posthoglil 4, micromarklil 8, katexlil 19, jquerylil 11, motionlil 19. There are **0 misses**, so the 150-byte margin has never pruned a winning move on the reference ports.

**Deviations.**
- The effort gates' removal changes levels 1–7: the naming seeds can now be walked there. It was not predicted, and the replay check found it.
- Deterministic `-j` waits for M5.6.
- The codec settings go with M3.1's first slice (B3).

**Open.**
- katexlil's level 13 makes 66 exact judgements across its starts. The memo recalls repeats within a start only, because W's and A0's candidates differ.
- M3.1's first slice and the codec settings (B3); M3.4.

## 2026-09-28 Batch B3: the objective and effort axes (M3.1 first slice, M3.5 codec settings)

**Pre-registration** (written before the first build of the batch; base `bd3ddbd0`, baseline binary `~/lilscript-work/bin/b2-2`).

What the batch builds (architecture §14.1–§14.2, law B2; plan M3.1's first slice, M3.5's codec settings):
- **`[objective]`**: `codecs = ["brotli"]` (any of raw, gzip, brotli; a single string is accepted too), `[objective.brotli] quality = 11, window = 22, mode = "generic"` (or `text` or `font`), and `[objective.gzip] level = 9, window = 15`. Ranges are validated. The settings are fingerprinted and printed in the receipt. For now `codecs` names one codec: the CLI delivers one winner, and several come with M3.4.
- **`[effort] level = 13`** (0–16).
- **The translator.** `[javascript] cost_model` is replaced by `[objective] codecs`, and `[javascript] optimization_level` by `[effort] level`, each with a rename warning; when both old and new are set, the new one wins with a warning. The repository's configurations and test configurations move to the new keys in the same batch, so the case configurations still load without warnings (BC12). The ports' configurations warn until M12.2 rewrites them.
- **The judge is the configured codec** (law B2). Every exact measurement of the build uses the objective's settings, and the proxy is Brotli at min(quality, 5) with the objective's window and mode. `lilscript-codec` and the benchmark contract stay canonical (quality 11, window 22; gzip level 9, window 15).

Deferred, each with its owner:
- `[resources]` to M5.6: memory and deadline, once the work ledgers retire.
- `[performance]` to M2.12's perf runner.
- `[target.javascript] checks` to M10.9's development-check lane.

None of them can mean anything before its owner lands, and an accepted key must not be inert.

Predicted:
- Default settings change no byte at any level.
- A configuration with `quality = 9` compiles faster and is judged, reported and fingerprinted at quality 9.
- Every existing configuration keeps building, with the warnings above.

**Landed** (binary `~/lilscript-work/bin/b3-1`, SHA-256 `0ea18239ce89f1e8…`):
- **Configuration.** `ProjectConfig` has `objective: ObjectiveConfig { codecs, brotli, gzip }` and `effort: EffortConfig { level }`. `codecs` takes a string or an array. Validation covers the level (0–16), exactly one codec until M3.4, and each codec's ranges. `resolve_policy` reads the level, the codec and the settings from the new tables. The retired keys are translator entries: `javascript.cost_model` and `javascript.optimization_level` are renamed with a warning, and the beam keys have no effect and warn (`EFFORT_SCHEDULE`).
- **Codec settings** (`compression.rs`): `CodecSettings { brotli: BrotliSettings { quality, window, mode }, gzip: GzipSettings { level, window } }`, with `CANONICAL` and `validate`. `measure_admitted_with` and `measure_proxy_admitted` take the settings.
- **The judge is the configured codec.** The artifact arena is bound to the objective's settings once per compilation, in `prepare_javascript_contract`. Every record's measurement, proxy and cached size uses them, and the staging arena inherits them. The receipt prints `codec_settings`, and the settings are part of the policy fingerprint. `lilscript-codec` and the benchmark contract stay canonical.
- **The repository moves to the new keys.** 94 configurations (benchmarks, comparison configurations, `tests/config`, the fixture) and about 60 test strings were rewritten. The case runner writes `[objective] codecs` and `[effort] level`, and `scripts/ports.mjs` rewrites both spellings. `docs/configuration.md` is rewritten, and `docs/knowledge/config/schema.md` is regenerated from `finer/tools/config-schema.mjs`.

**Evidence:**
- **Unit tests:** 1,584 pass, 2 of them new: the old keys are renamed into the tables, and the objective's codec settings judge, report and fingerprint the build.
- **Case runner** against `b2-2`: no artifact changes in any of the 18 lanes. No case configuration warns (BC12).
- **Monotone and replay** (levels 0, 1, 5 and 10–16): no growth, and all 45,162 recorded stops replay.
- **Ratchet:** pass, no change.
- **Reference ports:** all seven green. All 681 delivered files are byte-identical to `b2-1` (by SHA-256). The ports' configurations warn about the renamed keys, as designed.
- **Frozen level-13 entries:** bytes and exact-judgement counts are identical; CPU is ×0.97–×1.03.
- **Quality 9** (katexlil's frozen level-13 entry with `[objective.brotli] quality = 9`): 24.1 s against 43.0 s at quality 11. Its output is 262,047 / 72,065 / 60,024 (raw / gzip / Brotli-11), against 262,880 / 60,119 at quality 11. The plan search is chaotically sensitive to its judge, so one entry does not make quality 9 a better judge. It does make it the effort axis's cheapest lever above level 12, to be measured on the calibration corpus (M2.15).

**Deviations.** None.

**Open.**
- `[resources]` → M5.6, `[performance]` → M2.12, `[target.javascript] checks` → M10.9.
- Several codecs in one objective, with the public API: M3.4. It is off the critical path and follows language slice 1's first batch (S1).
- `-j` scoring waits for M5.6.

## 2026-09-28 Batch S1: the dynamic type's syntax (M4.2, M10.2 first part)

**Pre-registration** (written before the first build of the batch; base `f2efdaef`, baseline binary `~/lilscript-work/bin/b3-1`).

What the batch builds (language.md R12 and §14; plan M4.2, M10.2):
- **The dynamic type is a type.** `Type::Dynamic` replaces `TypeParameter("$js")` at every site of the checker, the native plan and the effects (M4.2's representation half).
- **Ordinary syntax on `JsValue`.** Each form lowers to the IR operation its `JS.*` spelling lowers to, so every dynamic operation has one IR form:

  | Syntax (an operand is a `JsValue`) | Operation |
  |---|---|
  | `v.k`, `v[k]` | `JS.get` |
  | `v.k = x`, `v[k] = x`; `+=` with a primitive key | `JS.set`; `JS.set` of `JS.add` of `JS.get` |
  | `v.m(a)`, `v[k](a)` | `JS.invoke` |
  | `v.call(t, a)`, `v.apply(t, a)` | `JS.call`, `JS.apply` |
  | `f(a)` | `JS.call(f, undefined, a)` |
  | `new C(a)` on a binding, a member chain or a parenthesized expression | `JS.construct` |
  | `a + b` (neither a `string`), `%`, `<`, `<=`, `>`, `>=` | `JS.add`, `JS.mod`, the comparisons |
  | `===`, `!==` (new tokens) | `JS.strictEqual`, `JS.strictNotEqual` |
  | `a \|\| b`, `a && b` | `JS.or`, `JS.and` |
  | `typeof v`, `k in v`, `delete v.k` | `JS.typeOf`, `JS.in`, `JS.delete` |
  | `v instanceof C` | a new builtin, emitted `v instanceof C` |
  | `v as T` | `JS.assume` |
  | `string(v)`, `number(v)`, `float(v)` | `JS.string`, `JS.number` |
  | `undefined` | `JS.undefined()` |
  | `object { k: v }`; an array literal whose expected type is `JsValue` | `JS.object`; `JS.array` |

  `==` and `!=` stay loose, as M1.9 made them.
- **One IR.** The spellings that had another IR before this batch move to the builtins: `v[k]` as a place, `f(x)` as a value call, `v["m"](x)` as a reference call, `object {…}` as an allocation, and array literals in a `JsValue` context.
- **The typed members keep their meaning in this batch.** `length`, `message`, `specifier`, `truthy()`, `isArray()` and `isObject()` on a `JsValue` are unchanged. The fix spells those names with brackets.
- **The fix-it** (language.md §14, batch 1 of two). A lint rule, `migration/js-builtin`, warns at every `JS.*` call that has a syntax spelling. Its machine-applicable fix rewrites the call, innermost call first, parenthesizing by the parser's precedences. `lilscript-lint --fix` is run to a fixed point.
  - Calls with no spelling yet stay: `JS.methodN`, `JS.methodRest` and `JS.staticRest` (M10.4); the array and string helpers such as `JS.push` and `JS.stringSlice` (the catalog, M10.17); `JS.box`, `JS.has` and `JS.encodeURI*`; and `JS.get`, `JS.set` and `JS.invoke` on a receiver that is not a `JsValue`.
- **Port patches** in `~/lilscript-work/portwork/s1/`: each reference port is rewritten by the fix. Where a port has its own helper that only returns `JS.undefined()`, its calls are replaced by `JS.undefined()` first. Each port is built before and after the rewrite with the batch's binary, and its suite runs.

Not in this batch (S2 and the rest of slice 1):
- the refusal of the rewritten spellings, and the typed members' retirement;
- `?.`, `?.()` and `??` on a `JsValue`; `-`, `*`, `/`, unary `-` and `!`, and truthiness;
- `as?` and `unknown`;
- spread in calls (M10.4), dynamic `for…of` (M10.16), and `is` on classes (M10.7).

Predicted:
- **Programs that do not use the new syntax:** the only IR changes are the spellings that move to the builtins. Case and port bytes change only where those appear, within noise, with no growth at level 13 and above.
- **Rewritten ports:** each builds byte-identical to its unrewritten source under the batch's binary, and its suite passes.
- **Census:** the `JS.*` mentions in the seven reference ports fall from 16,041 to about 1,000 (the method adapters, the helpers and the non-`JsValue` receivers).
- **No bytes from new capabilities:** the new capabilities (`instanceof`, the `new` forms) yield bytes only when a port uses them, in their own patches.

**Landed** (binary `~/lilscript-work/bin/s1-1`, SHA-256 `4fb1870b726a1bd4…`; the fix-it as rebuilt for the port rounds, `s1-3/lilscript-lint`).

**Changes** (listed after the fact; the rule of one commit per change came during this batch and applies from S2):
- **C1. The dynamic type is a type (M4.2's representation half).** `Type::Dynamic` replaces `TypeParameter("$js")` in the checker, the native plan, the effects and the product families; `JsValue` prints as itself.
- **C2. Syntax.** Tokens `===` and `!==`. AST nodes `Cast` (`v as T`), `Convert` (`string(v)`, `float(v)`, `number(v)`), `Construct` (`new a.b.C(x)`, `new (f())(x)`), `DynamicBinary` (`===`, `!==`, `in`, `instanceof`) and `DynamicUnary` (`typeof`, `delete`). Those operators get their own enums (`DynamicBinaryOp`, `DynamicUnaryOp`), because the IR shares `ast::BinaryOp` and never sees them. `typeof`, `delete` and `instanceof` are contextual. The parser's precedences become one table (`ast::precedence`, `BinaryOp::precedence`), which the fix-it shares. Every AST walker learns the new nodes.
- **C3. The checker.**
  - Members, indexes, method calls and calls of a `JsValue` type as JavaScript's, with any property name; the six typed members keep their meaning.
  - Operators with a `JsValue` operand resolve to `ExpressionResolution::Dynamic(builtin)`: `+` (unless a `string` operand makes it concatenation), `%`, the comparisons, `||` and `&&`, `===`, `!==`, `in`, `instanceof`, `typeof`, `delete`, `v as T`, the conversions, `new` of a `JsValue` binding or value, `v.call(t, a)`, `v.apply(t, a)` and `undefined`.
  - `v += x` on a `JsValue` place is the dynamic add.
  - An array literal where a `JsValue` is expected holds `JsValue`s, so `[]` is legal there.
  - `undefined` is a parameter default like `JS.undefined()`.
  - Reading a binding moves into `analyze_binding_read`, which `new C(a)` shares.
- **C4. The lowering.** A `Dynamic(builtin)` node lowers through `dynamic_expression`: one call site, its `PrepareCall`, then the node's operands in order, exactly as `JS.name(operands)` lowers. `prepare_call_with_receiver` and `prepare_call_values` share `open_call` and `close_call`. Member access, indexes, method calls, calls and object and array literals keep their natural IR (places, reference and value calls, allocations).
- **C5. `instanceof`.** `BuiltinCall::JsInstanceOf`, emitted `v instanceof C`, an operator over its operands (`operands_first`).
- **C6. The fix-it.** `migration/js-builtin` (`src/lint/js_builtin.rs`) reports every `JS.*` call with a spelling and fixes the innermost ones. It parenthesizes by the shared precedences and offers no fix where the syntax would mean another operation. `lilscript-lint --fix` now applies identical edits once and skips overlapping ones, since shared modules are linted once per entry graph.
- **C7. Tools and docs.** `scripts/ports.mjs --patches DIR`; `~/lilscript-work/tools/s1-port-rewrite.sh`; configuration and lint docs; `types-not-glue.md`.

**Found by the unit tests, and the design change it made.** The first build lowered all syntax onto the `JS.*` builtins, so that rewritten ports would be byte-identical. Five unit tests failed. Two of them run programs that observe evaluation order: a getter on the method, and an argument that redefines it. `JS.invoke`'s builtin form evaluates the method lookup after the arguments, so the emitter hoists effectful arguments ahead of the lookup: `(c=value,e=argument(),c).go(e)` where JavaScript reads `value.go` first. That is wrong when an argument changes the method, and longer besides. The natural IR, a reference call whose preparation is the lookup, is correct, and object literals as allocations carry facts the builtin lacks. So syntax that has a natural IR form keeps it, and only the operators use the builtins. The `JS.*` spellings keep their lowering until the refusal batch deletes them, and each port's rewrite carries its IR change, measured per port.

**Evidence:**
- **Unit tests:** 1,588 pass at `s1-1`, and 1,589 at the next checkpoint, which adds the fix's regressions below to its fixed-point test.
- **Case runner** against `b3-1`: no artifact changes in any of the 18 lanes.
- **Monotone and replay:** pass, with all 45,162 stops replayed.
- **Ratchet:** pass, no change.
- **Reference ports, unpatched:** all seven green, and all 681 delivered files byte-identical to B3's. The batch changes nothing for a program that does not use the new syntax.
- **CPU pairs** against `b3-1`: ×1.00–×1.02, with identical bytes and judgement counts.
- **The rewrite** (`migration/js-builtin`, run to a fixed point per port; patches in `~/lilscript-work/portwork/s1/fix/`). All seven suites pass on the rewritten sources. `JS.*` mentions in the seven ports' `src/`:

  | Port | Before | After |
  |---|---:|---:|
  | markedlil | 30 | 13 |
  | zodlil | 726 | 108 |
  | posthoglil | 698 | 127 |
  | micromarklil | 3,459 | 428 |
  | katexlil | 5,793 | 506 |
  | jquerylil | 2,028 | 498 |
  | motionlil | 1,601 | 476 |
  | total | 14,335 | 2,156 (−85%) |

  What remains is mostly the method adapters (M10.4), the array and string helpers (the catalog), `JS.isNullish`, `JS.isUndefined` and `JS.isFalse`, `JS.has` and `JS.box`, and calls whose receiver is not a `JsValue`.
- **Bytes of the rewrite** against the unpatched ports (the S1 binary on both):
  - jquerylil, markedlil and posthoglil: byte-identical;
  - motionlil: −68 Brotli (+1,097 raw), 571 of 617 files (its graph split);
  - micromarklil: +37 Brotli over 8 files;
  - katexlil: +646 Brotli over 10 files, one program seen ten times (+65 per file). At level 0 the difference is +16 Brotli, so most of it is the level-13 search's sensitivity. The rest is the forwarding gap below;
  - zodlil: +331 Brotli over 2 files. At level 0 it is +105, all of it the forwarding gap.
- **The first port use of `instanceof`:** katexlil's `instanceOf(value, ctor)` helper calls, spelled `value instanceof ctor` as upstream KaTeX writes them (31 sites), cost −184 Brotli and +8,423 raw over its 10 files, on top of the rewrite.

**Findings.**
- **The forwarding gap.** A port's wrapper that forwards its parameters into one host call (`invoke1(o, m, a)` returning `JS.invoke(o, m, a)`) compiled to its call sites because formation replaces a forwarding wrapper of a *builtin* with the builtin. Rewritten as `o[m](a)`, it is a natural reference call, and the wrapper stays: zodlil's level 0 prints `w(b,"concat",c.checks)` where it printed `b.concat(c.checks)`. The rule must forward any single host operation, not only a builtin; batch S2 takes it as change C7.
- **A port's `undef()` helper is a size lever the compiler lacks.** Replacing `undef()` with `JS.undefined()` cost katexlil +92 to +192 Brotli per file (+2,362 raw), since `u()` is 3 bytes against `void 0`'s 6. Level 0 is identical, so the level-13 search keeps the helper as a short alias. An undefined alias (an unassigned compiler-owned binding, 1 byte per use) belongs to M9.3's spelling families; until it lands, ports keep their helpers, and the rewrite leaves them alone.
- **The fix-it's first rounds broke four ways**, each now a regression line in its fixed-point test:
  - the parser drops parentheses from spans, so an operand's text is widened to balanced parentheses from the tokens;
  - `Map` and the other builtin constructor names need `new (Map)(…)`;
  - an arrow's result type is inferred, so its results expect nothing;
  - a module that binds `undefined` (katexlil's and zodlil's `extern JsValue undefined`) keeps `JS.undefined()`. Spelling it `undefined` there read the host global, and the compiler lost that `undef()` returns undefined: +16,759 Brotli on katexlil.
  - Linting each port's graph roots one by one keeps a source the build never compiles from stopping the rest.
- **The owner's statement on runtime casts** answers Y1 (architecture §21.1): no conversion the source did not write. S1's forms follow it: `as` emits nothing, `instanceof` and `typeof` are the explicit tests, `string(v)` and `float(v)` the explicit conversions.

**Deviations.**
- **One IR per operation:** only the operators are one IR with their spellings. Member access, calls and literals keep their natural IR, and the `JS.*` spellings keep theirs until the refusal batch. So the rewrite is not byte-identical: its bytes are measured per port above.
- **Census:** it fell to 2,156, not about 1,000, because the adapters and the `JsValue` tests stay for S2 and later.
- **Loss gate:** the rewrite loses Brotli on katexlil (within the search's noise at level 13, +16 deterministic at level 0) and on zodlil (+331, the forwarding gap). The patches are kept in `portwork` and not counted as landed port changes until S2's C7 closes the gap.

**Open.**
- S2: M10.4's first part, with C7 (forwarding any single host operation).
- The refusal batch for the rewritten spellings, when every reference port's patches are loss-free and the test sources are rewritten.
- `?.`, `??`, arithmetic and truthiness on `JsValue`; `as?` and `unknown`; `JS.isNullish`, `JS.isUndefined` and `JS.isFalse` as syntax.
- M9.3: an undefined alias.
- M10.12 (Y1 answered yes).

## 2026-09-28 Owner decisions: compile time scales, test on a clock, track each change

The owner's message, during batch S1, is in [finer/intent/2026-09-28.md](../../finer/intent/2026-09-28.md).

- **Amendment AM3** (architecture §13.2, §13.7 and appendix A). Compile time has no target in seconds: a program that grows takes longer, so a number of seconds either blocks honest growth or says nothing about a new program. Withdrawn: level 12 at the frozen `-j 4` time and 1.3× it single-threaded, level 0 at 1.4 s on katexlil, katexlil's package build at 14.4 s, the package builds at upstream's time, and levels 1–4 at 2× Terser. In their place:
  - *scaling*: each phase's counted work at levels 0–12 is at most linear in the program's size (log-linear where it sorts), and a batch that adds a super-linear term fails;
  - *relative cost*: CPU per thousand IR operations against the previous binary on the same inputs; at levels 0–12 a rise the pre-registration did not predict fails, and at 13 and above it is reported (AM2);
  - *what time buys*: every exact judgement is reported with its bytes (B9).
  The frozen baseline (M2.14) stays as the record of where the time went. The plan's compile-time gate, M3.5's exit, step 6's exit, CP2 and M12.4 follow.
- **Test on a clock** (plan rule 8). Per change, a type-check only. Builds and unit tests run about every 20 minutes of work or right after a critical part, meaning a change every program passes through; the ladder runs per batch.
- **Track each change** (plan rule 5). A batch's pre-registration lists its changes as C1, C2, …; each lands as its own commit on the batch branch, and a surprise in the batch's verification is bisected over those commits. Batch S1, already built when the rule came, records its changes as a list; the rule applies from the next batch.

## 2026-09-28 Batch S2: receivers and rest parameters (M10.4, first part)

**Pre-registration** (written before the first build of the batch; base `02cb92a1`, baseline binary `~/lilscript-work/bin/s1-1`).

What the batch builds (language.md R7; plan M10.4):
- A lambda may name its receiver and gather its rest arguments: `(this JsValue self, JsValue a) => body` and `(this JsValue self, JsValue... argv) => body`, and without a receiver `(JsValue... argv) => body`. Such a lambda is the method its `JS.method<N>`, `JS.methodRest` or `JS.staticRest` spelling makes, with the same IR, so batch A1's formation emits it as the function itself. Its parameters and result are `JsValue`, and the value is a `JsValue`, as the adapters' are. Typed receivers (`fn(this: T, A) -> R` as a type), spread arguments and `extern JsValue this` retire in the second part.

Changes (one commit each):
- **C1.** AST and parser: `ast::ParamRole` (`Value`, `Receiver`, `Rest`); `this` before the first parameter's type; `...` after a parameter's type, last parameter only.
- **C2.** The checker: a lambda with a receiver or rest parameter resolves to `Dynamic(JsMethod<N> | JsMethodRest | JsStaticRest)`. The roles must be in their places, every parameter and the result are `JsValue`, and no parameter has a default. The node's recorded type is its callback's function type, and its value is `JsValue`.
- **C3.** The lowering: such a lambda's closure is formed with its callback type and passed to its adapter, exactly as `JS.method<N>(lambda)` lowers.
- **C4.** The fix-it: `JS.method<N>`, `JS.methodRest` and `JS.staticRest` of a lambda become receiver and rest lambdas. `JS.call(o.m, o, …)`, where both `o`s are one binding, becomes `o.m(…)`.
- **C5.** Tests: the parser's roles, the adapters' identity with their spellings, and the fix's fixed point.
- **C6.** Port patches regenerated with the new fix-it on top of S1's, measured per port.
- **C7** (added during the batch, after S1's record). Formation forwards a wrapper into any single host operation, not only a builtin. A reference call on `p0[p1]` with the remaining parameters as arguments is `JS.invoke`; a load of `p0[p1]` is `JS.get`; an empty object literal is `JS.object()`; an array literal of the parameters is `JS.array`. S1's rewrite lost zodlil +331 Brotli because its `invoke` wrappers, written as `o[m](a)`, stayed calls.

Predicted:
- Unmodified programs, cases and ports: byte-identical.
- The receiver and rest syntax compiles byte-identically to its adapter spelling.
- `o.m(…)` is the reference call where `JS.call` was: the output was already `o.m(…)` through `self_method_calls`, so bytes are within noise.
- Census: the seven ports' `JS.method*`, `JS.methodRest` and `JS.staticRest` mentions (about 800) fall to those with a function reference as the argument.

**Landed** (binary `~/lilscript-work/bin/s2-1` for the ladder, SHA-256 `a30385314c2db608…`; the port run with `s3-1`, which carries S2's fix-up).

Changes, each its own commit:
- **C1.** `ast::ParamRole` (`Value`, `Receiver`, `Rest`). The parser reads `this` before a lambda's first parameter type and `...` after its last. Declarations refuse both: a receiver or rest parameter belongs to a lambda in this part.
- **C2.** A lambda with a receiver or rest parameter resolves to its adapter (`JS.method<N>`, `JS.methodRest`, `JS.staticRest`). Its parameters and result are `JsValue`s, it takes no defaults, and the roles are in their places. The node's recorded type is the callback's function type; its value is the method, a `JsValue`.
- **C3.** Lowering: the lambda's closure is formed as the adapter call's operand, exactly where `JS.method<N>(lambda)` forms it. Closure formation moved into `Lower::closure`, which the ordinary arrow shares.
- **C4.** The fix writes receiver and rest lambdas, and `o.m(…)` for `JS.call(o.m, o, …)` when both `o`s are one binding.
- **C5.** Tests: the parser's roles and refusals, adapter identity with the spellings (a method lambda bound to a name included), and the fix's fixed point.
- **C7** (added during the batch). Formation forwards a wrapper into any single host operation. A reference call on `p0[p1]` with the rest as arguments is `JS.invoke`, a load of `p0[p1]` is `JS.get`, an empty object literal is `JS.object()`, and an array literal of the parameters is `JS.array`.
- **Fix-up**, found by the first port run: `infer_creation_id` assumed a lambda bound to a name has a closure as its value. A method lambda's value is its adapter's result, and like its spelling it takes no inferred name. The adapter-identity test now binds one to a name.
- A diagnostic knob, `LILSCRIPT_JS_FIX_ONLY`, lets the fix run one family of spellings at a time.

**Evidence** (ladder at `s2-1` against `s1-1`):
- Unit tests: 1,589 pass (1,590 with S3's).
- Case runner: no artifact changes in any of the 18 lanes. C7 finds no natural forwarding wrapper in the case corpus.
- Monotone and replay: pass, all 45,162 stops. Ratchet: pass, no change.
- Reference ports, unpatched: all green, and all 681 files byte-identical to S1's.
- CPU pairs: ×0.997–×1.006, identical bytes and judgement counts.
- **Ports rewritten with S1–S3's fixes together** (run at `s3-1`, below in S3's record): all seven suites pass, and the method adapters fall from about 800 mentions to 73, those whose argument is a function reference. With C7, S1's zodlil loss (+331 Brotli) becomes +15 and katexlil's +646 becomes −298.

**Deviations.**
- **C7 joined the batch** after S1's record found the forwarding gap.
- **The fix-up came from the port run**, which the unit tests had missed: they bound method lambdas to members, never to names.
- **One port run for two batches:** S2's own port run failed on the fix-up's panic, so S2's ports are verified in S3's combined run.

**Open.** M10.4's second part: typed receivers (`fn(this: T, A) -> R` as a type), spread arguments, declared functions' rest parameters, and `extern JsValue this`/`arguments` retiring (jquerylil's 22 files). `self_method_calls` is deleted once no port writes `x.m.call(x, …)`.

## 2026-09-28 Batch S3: the dynamic type's tests and conversions (M10.2, second part)

**Pre-registration** (written before the first build of the batch; base: S2's commits, baseline binary `~/lilscript-work/bin/s2-1`).

What the batch builds (language.md R12, and the owner's Y1 answer: the runtime carries no conversion the source did not write, and a test or conversion appears only where the source writes one).

Changes, one commit each and grouped by feature, so a surprise bisects to a feature:
- **C1** (`c3983f33`). `v ?? x` with a `JsValue` on the left: JavaScript's nullish coalescing, the natural short circuit, whose right side expects a `JsValue`.
- **C2** (`b253ebaf`). `bool(v)`: JavaScript's truthiness as an explicit conversion. It is the `truthy()` intrinsic's call, so the fix of `v.truthy()` is byte-identical.
- **C3** (`4d598ed2`). `-`, `*`, `/` and unary `-` on a `JsValue`, as operators over their operands (new builtins). `!v` on a `JsValue` is refused with `!bool(v)`, since conditions stay `bool`.
- **C4** (`1a7ca86c`). `v as? T` for `float`, `string` and `bool`: the `typeof` test `v is T` selects `v` viewed as `T`, or null.
- **C5** (`afdec67f`). The fix: `JS.isNullish(v)` → `v == null` (loose equality on a `JsValue`, M1.9); `JS.isUndefined(v)` → `v === undefined` where `undefined` is not bound; `JS.isFalse(v)` → `v === false`; `v.truthy()` → `bool(v)`.
- **C6** (`978d823c`). Tests: the forms' meaning run under Node, and the fix's fixed point.
- **C7.** Port patches regenerated (S1–S3's fixes), measured per port.

Not in this batch:
- `?.` and `?.()` on a `JsValue`: the JavaScript tree has no optional chain yet, which M9.3's optional-chaining family brings;
- `unknown`;
- `is` and `as?` on classes (M10.7).

Predicted:
- Unmodified programs: byte-identical.
- Rewritten ports: `v == null` for `JS.isNullish(v)` trades a builtin for loose equality with the same text, and `bool(v)` is identical; bytes within noise.
- Census: about 340 fewer `JS.*` mentions in the seven ports, and the `.truthy()` calls gone.

**Landed** (binary `~/lilscript-work/bin/s3-1`, SHA-256 `5012d564ec167c61…`; the fix-it as rebuilt for micromarklil's rerun, `s3-2/lilscript-lint`).

Changes: C1–C6 as pre-registered, with three fix-ups found on the way:
- **C1 fix-up.** `JsValue ?? x` moved into the shared binary-typing rule, which the IR verifier also reads. The checker-only special case failed verification ("semantic operation type mismatch: ShortCircuit Nullish"). The frozen prior-rules oracle records the rule, as a deliberate change.
- **C2 fix-up.** `bool(v)` carries `truthy()`'s checked signature, `() -> bool`, which the checker builds by hand; the intrinsic table has none.
- **C5 fix-up.** A fix's edit covers the call's balanced text. micromarklil's `(if … { … }).truthy()` started its span inside the parentheses, and the fix left an unclosed one.

**Evidence** (ladder at `s3-1` against `s2-1`):
- Unit tests: 1,590 pass.
- Case runner: no artifact changes in any of the 18 lanes.
- Monotone and replay: pass, all 45,162 stops. Ratchet: pass, no change.
- Reference ports, unpatched: all green, and all 681 files byte-identical to S2's.
- CPU pairs: ×0.98–×1.00, identical bytes and judgement counts.
- **Ports rewritten with S1–S3's fixes**, from each port's own sources. All seven suites pass. Bytes against the unpatched ports at `s3-1`:

  | Port | Files changed | Raw | gzip | Brotli |
  |---|---:|---:|---:|---:|
  | jquerylil, markedlil, posthoglil | 0 | 0 | 0 | 0 |
  | katexlil | 10 of 26 | +1,381 | −232 | −298 |
  | zodlil | 2 of 7 | +85 | −56 | +15 |
  | motionlil | 21 of 621 | +925 | +154 | +88 |
  | micromarklil | 8 of 9 | +177 | +119 | +37 |

  motionlil and micromarklil are within noise per file (+4 and +5 Brotli), and katexlil is a win.
- **Census**, the seven ports' `JS.*` mentions in `src/`:

  | Port | Before | After |
  |---|---:|---:|
  | markedlil | 30 | 4 |
  | zodlil | 726 | 10 |
  | posthoglil | 698 | 106 |
  | micromarklil | 3,459 | 25 |
  | katexlil | 5,793 | 361 |
  | jquerylil | 2,028 | 303 |
  | motionlil | 1,601 | 245 |
  | total | 14,335 | 1,054 (−93%) |

  What remains:
  - `JS.add` with a typed `string` operand (296), where `+` would be typed concatenation;
  - `JS.invoke` (151) and `JS.assume` (126) on typed values;
  - `JS.call` (47);
  - adapters of a function reference;
  - the catalog helpers (`regexTest`, `isArray`, `stringSlice`, …).
- **katexlil's `instanceof` idiom** is noise: +177 Brotli on this run against −184 on S1's.

**Findings.** Three fix-ups, each found by a port run, not by unit tests: the verifier's rule, the intrinsic's signature, and a span inside parentheses. The port runs are this slice's real test of the fix-it and the new syntax, which is why each batch runs them.

**Deviations.**
- **`!v` on a `JsValue` is refused**, spelled `!bool(v)`, where the pre-registration had it mean `!bool(v)` implicitly: the owner's Y1 answer wants truthiness explicit.
- **One port run covers S2 and S3.**

**Open.**
- `?.`, `?.()` and `unknown`.
- `is` and `as?` on classes (M10.7). Classes are observed today only when published or host-derived, and marking runs before bodies are checked, so a tested class needs a second marking step.
- The typed-`string` `+` rewrite: its result type becomes `string`.
- The refusal batch.

## 2026-09-28 Batch S4: iteration and spread (M10.16, M10.4 second part)

**Pre-registration** (written before the first build of the batch; base `ec9e45b2`, baseline binary `~/lilscript-work/bin/s3-1`).

What the batch builds (language.md R14 and R7):
- **Iteration.** `for (T x of set)` over a `Set<T>`, and `for (K k, V v of map)` over a `Map<K, V>`, each in insertion order. `for (JsValue x of v)` iterates a `JsValue` by JavaScript's iterator protocol. Each runs the IR's `for…of`, which generators already use. Arrays keep their index loop until M9.12's codec-judged choice (C5). A map's entry `[k, v]` is read into its two bindings, viewed as `K` and `V` with no code (Y1).
- **Spread arguments.** `f(...xs)`, `o.m(...xs)`, `new C(...xs)` and `f.call(t, ...xs)` spread an array or a `JsValue` iterable into a JavaScript call, and so do `JS.*` calls with a `JsValue` rest. A call to a LilScript function takes no spread, since its parameters are typed and declared rest parameters come later. The IR gains `CallArgument::Spread`, legal only in a host call and printed `...xs`.

Changes (one commit each):
- **C1** (`f1263c51`). `for…of` over a `Set<T>` and over a `JsValue`.
- **C2** (`bf094894`). `for (K k, V v of map)`: the parser's value binding, the checker's map rule, the lowering through a synthetic entry, the verifier's map element.
- **C3** (`52a3f20d`). A test of set, map and `JsValue` iteration, with `break`.
- **C4.** The parser's spread argument and `Argument::spread`.
- **C5.** The checker: a spread argument only in a host call, over an array or a `JsValue`.
- **C6.** The IR's `CallArgument::Spread`: the lowering, the passes (a spread is a use of its value and is never inlined into a LilScript callee), the verifier (host calls only) and the JavaScript printer (`...value`).
- **C7.** Tests: spread's meaning, run.

Predicted:
- Unmodified programs, cases and ports: byte-identical.
- No census change: `JS.apply` sites could become spreads, but `apply` passes an array-like where a spread iterates, so the fix-it leaves them.

**Status (2026-09-28, evening).** Pushed before its ladder at the owner's request ("commit and push everything"). The unit suite passes: 1,593 tests, 9 ignored. Its first runs found four things, each fixed in its own commit: a test constructed `Argument` without `spread`; the arity error hid the spread refusal; the demand walk had no arm for a spread argument; and the JavaScript tree's verifier admitted a spread only inside an array literal. The ladder ran with S5's (below): every unmodified program, case and port is byte-identical through S4.

**Deviation found while writing the owner's report.** The pre-registration said a map entry's bindings are "viewed as `K` and `V` with no code (Y1)". That holds for the key, but an `int` value is still normalized on load: `for (string name, int count of counts)` prints `let e=d[0],f=d[1],c=f|0`. This is the load normalization that M10.12 deletes ("loads never normalize", step 8), so its owner is M10.12, not S4.

---

## 2026-09-28 Batch S5: the fix-it on typed operands (M10.2, M4.2)

**Pre-registration** (written before the first build of the batch; base `3bed426f`, baseline binary `~/lilscript-work/bin/s3-1`). S4's ladder runs with this batch's, at the owner's direction to test several changes together; each change is its own commit, so a surprise bisects to it.

What the batch builds (language.md R12; M10.14 names the explicit view):
- **`x as JsValue`**: the explicit view of a typed value as the dynamic type. It emits no code: it resolves to the trusted view `JS.assume` resolves to, over a typed operand, which is what `JS.assume(x)` with a typed `x` does today.
- **The fix-it on the spellings it left** (the 1,054 at S3):
  - `JS.add` with a typed `string` operand becomes `+`. `+` with a `string` operand is typed concatenation, and it prints the same JavaScript `+` (checked: `JS.add("color:", v)` and `"color:" + v` both print `"color:"+a`). Its type becomes `string`, so it is not offered where the sum is a receiver or the operand of a unary operator.
  - `JS.get`, `JS.set`, `JS.delete`, `JS.invoke`, `JS.call`, `JS.apply` and `JS.construct` on a typed receiver or callee are spelled on `(x as JsValue)`, which keeps the dynamic operation.
  - `JS.assume(x)` on a typed `x` becomes `x` when `x` already has the target type, `x as JsValue` for a `JsValue` target, and `x as JsValue as T` otherwise.

Changes (one commit each):
- **C1.** Checker: `as JsValue` from any value type.
- **C2.** Fix-it: typed-`string` `JS.add`.
- **C3.** Fix-it: typed receivers and callees.
- **C4.** Fix-it: `JS.assume` on typed values.
- **C5.** Tests: the checker's `as JsValue`, and the fix-it's new rewrites at a fixed point.
- **C6.** The seven reference ports rewritten from their own sources, suites green, census.

Predicted:
- Unmodified programs, cases and ports: byte-identical to `s3-1` through S4 and S5.
- Census: 1,054 `JS.*` mentions fall below 400. What stays is the catalog helpers, function-reference adapters, `JS.undefined()` where a module binds `undefined`, and calls whose operand keeps a `JS.*` spelling.
- Rewritten ports: the same operations, so bytes within noise.


**Landed** (binary `~/lilscript-work/bin/s5-1`, SHA-256 `99468b74817edc7d`). The ladder covers S4 and S5 together.

Changes, each its own commit:
- **C1.** The checker: `as JsValue` from any value type, resolved to the trusted view `JS.assume` resolves to.
- **C2–C4** (one commit; `LILSCRIPT_JS_FIX_ONLY` separates them by name). The fix-it on typed operands:
  - typed-`string` `JS.add` becomes `+`, but not as a receiver or a unary operand;
  - `JS.get`, `set`, `delete`, `invoke`, `call` and `apply` on a typed or nullable receiver or callee are spelled on `(x as JsValue)`;
  - `JS.assume` on a typed value becomes the value itself, `x as JsValue`, or `x as JsValue as T`.
- **C5.** Tests.
- **Fix-ups:**
  - a same-type `JS.assume` keeps its operand's text;
  - the test's `show` prints strings quoted.

**Evidence** (ladder at `s5-1` against `s3-1`):
- Unit tests: 1,595 pass, 9 ignored.
- Case runner, 395 cases × 18 lanes: 0 artifacts changed in every lane.
- Monotone and replay: pass, all 45,162 stops; the level-16 totals are unchanged.
- Ratchet: pass, no change.
- Reference ports, unpatched: all green, 0 of 681 files changed.
- CPU pairs: ×0.995–×1.053 (posthoglil 0.47 → 0.50 s), with identical bytes and judgement counts. A type-check of K1 overlapped one pair; the pairs are medians of three.
- **Ports rewritten by S1–S5's fixes**, from their own sources: all seven suites green.
- **Census:** the seven ports' `JS.*` mentions fall from 14,335 to **452 (−96.8%)**:

  | Port | Mentions |
  |---|---:|
  | markedlil | 4 |
  | zodlil | 7 |
  | micromarklil | 9 |
  | katexlil | 48 |
  | posthoglil | 81 |
  | motionlil | 127 |
  | jquerylil | 176 |

  What remains:
  - `JS.assume` on types the fix-it cannot spell (61);
  - the method adapters over a function reference (83), which S7's forwarding fix takes;
  - the catalog helpers (`regexTest`, `isArray`, `stringSlice`, …), for M4.6;
  - `JS.or`, `JS.invoke` and `JS.add` in the contexts the fix-it declines;
  - `JS.undefined()` where a module binds `undefined`;
  - `JS.array` in `auto` contexts.

**Finding: a katexlil regression under the rewrite.** Bytes against the unpatched ports:

| Port | Files changed | Raw | gzip | Brotli |
|---|---:|---:|---:|---:|
| zodlil | 2 of 7 | +85 | −56 | +15 |
| motionlil | 21 of 621 | +925 | +154 | +88 |
| micromarklil | 8 of 9 | +177 | +119 | +37 |
| katexlil | 10 of 26 | −73,567 | +19,572 | **+12,770** |

- Every katexlil bundle is about 6.5K smaller raw, but about 2K larger in gzip and about 1.3K larger in Brotli (+2.2%). katexlil's whole build went from 144 s to 1,477 s.
- S3's rewrite was a Brotli win on katexlil (−298), so S5's new rewrites are the cause. The prime suspect is the typed-`string` `+` (227 of katexlil's sites). As a typed concatenation, `+` exposes its literal operands to string pooling and the data choices, which `JS.add` kept opaque.
- The patches are not applied anywhere, since port repositories change only at release. The diagnosis runs in S7's window, and its owner is the fix-it or the choice it exposes.


---

## 2026-09-28 Batch S6: identity tests, declared rest parameters, typed receivers (M10.7, M10.4)

**Pre-registration** (written before the first build of the batch; base: S5's last commit, baseline binary `~/lilscript-work/bin/s5-1`). Its tests and ladder run with S7's (the refusal), at the owner's direction to test several changes together.

What the batch builds (language.md R13 and R7):
- **`v is C` and `v as? C` on classes.**
  - Allowed where `v` is a `JsValue`, or has a member type that `C` is or extends; also for extern classes.
  - Refused on a generic class and on an `object`.
  - The test is `instanceof`. An internal class an identity test names keeps its identity: after every body is checked, it, its internal ancestors and every class extending it are marked observed, the second marking step S3 found missing.
  - Lowering: a kept class's test reads the class's constructor, as a second operand of the IR's type test. A host class is named by its extern declaration when printed.
  - Native refuses a class test for now (a class-id word comes with M11).
- **Declared rest parameters.**
  - `T... name`, last in a function, method or extern declaration. It is `T[]` in the body.
  - A call to a LilScript function packs the trailing arguments into a fresh array, with a spread argument as a spread element, so the IR keeps one argument per parameter. Printing the packed call as JavaScript's rest syntax is a spelling for M9.3.
  - A call to an extern passes the trailing arguments one by one.
  - Parameters before a rest parameter take no defaults.
- **Typed receivers and rest lambdas.** `(this T self, …) =>` and `(T... rest) =>` take any types, not only `JsValue`: the values the adapter passes are trusted views (Y1).

Changes (one commit each):
- **C1.** `is` and `as?` on classes: checker, second marking, lowering, printer, verifier.
- **C2.** Declared rest parameters: parser, checker, lowering of calls.
- **C3.** Typed receivers and rest lambdas.
- **C4.** Tests.

Not in this batch:
- `unknown` (R12): about 150 sites match on the dynamic type, so it gets its own batch.
- `?.` on a `JsValue`: M9.3's optional-chaining family, as S3 recorded.

Predicted: unmodified programs byte-identical (no program uses the new forms); the new forms' outputs as written in C4's tests.


**Landed** with S7 (binary `~/lilscript-work/bin/s7-1`, SHA-256 in `s7-1.sha256`).

Changes:
- **C1.** `is` and `as?` on classes (R13): `instanceof`, with the second marking step.
- **C2.** Declared rest parameters (R7).
- **C3.** Typed receivers and parameters in method lambdas.
- **C4.** Tests.

Fix-ups, found by the unit run (1,599 pass, 3 failed):
- the parser's "required parameters cannot follow defaulted parameters" hid the checker's reason that a function with a rest parameter takes no defaults;
- the verifier took an extern's rest arguments against the rest's array type, not its element;
- the identity test built an `Error` from LilScript, where an extern class cannot be constructed. It now builds it through `globalThis`.

The fix-ups' tests run with K1–K3's unit suite, on the core branch that carries them.

Evidence: S7's (below). Every unmodified case and port is byte-identical.


---

## 2026-09-28 Batch S7: `unknown`, and the fix-it's last family (M4.2, M10.2, M10.4)

**Pre-registration** (written before the first build of the batch; base: S6's last commit, baseline binary `~/lilscript-work/bin/s5-1`). Its tests and ladder run with S6's.

What the batch builds (language.md R12, R7):
- **`unknown`**: a dynamic type on which only the tests and the ways out apply until it is narrowed:
  - allowed: `==`, `!=`, `===`, `!==`, `typeof`, `instanceof`, `is`, `as T`, `as? T`, and the conversions `string(u)`, `float(u)`, `bool(u)`;
  - every value is assignable to `unknown`; `unknown` is assignable to nothing but itself, so leaving it, even to `JsValue`, is written;
  - it lowers as `JsValue` does, and a module that mentions it is JavaScript-only.
- **Function-reference adapters.** The fix-it writes `JS.method<N>(f)`, `JS.methodRest(f)` and `JS.staticRest(f)` over a function reference as the lambda that forwards to it, `(this JsValue self, JsValue a) => f(self, a)`, which is what the adapter calls. These are the 73 adapter mentions S2 left.

**Order change (recorded here, plan rule for re-ordering):** the refusal of the `JS.*` spellings, R12's second batch, lands with each port's next release (M12.4). Port repositories change only at release, and until then the unpatched ports must build. Step 7 closes with the fix-it complete and the patches in `~/lilscript-work/portwork/`.

Changes (one commit each):
- **C1.** `unknown`: the type, its relations and operations, lowering as the dynamic type.
- **C2.** The fix-it: function-reference adapters.
- **C3.** Tests.

Predicted: unmodified programs byte-identical; the census's adapter mentions fall to the ones a function reference cannot be forwarded from (none expected).


**Landed** (binary `~/lilscript-work/bin/s7-1`). The ladder covers S6 and S7 together, against `s5-1`.

Changes:
- **C1.** `unknown` (R12).
- **C2.** The fix-it writes a function-reference adapter as the lambda that forwards to it, where the binding is never reassigned.
- **C3.** Tests.

**Evidence** (ladder at `s7-1` against `s5-1`):
- Unit tests: S6's three failures are above; every other test passes (1,599 in total).
- Case runner, 395 cases × 18 lanes: 0 artifacts changed.
- Monotone and replay: pass, all 45,162 stops. Ratchet: pass, no change.
- Reference ports, unpatched: all green, 0 of 681 files changed.
- CPU pairs: ×0.932–×1.013, with identical bytes and judgement counts.
- **Census** after S1–S7's fixes: 14,335 → **355 (−97.5%)**; the method adapters are gone.

  | Port | Mentions |
  |---|---:|
  | markedlil | 2 |
  | zodlil | 7 |
  | posthoglil | 7 |
  | micromarklil | 9 |
  | katexlil | 48 |
  | motionlil | 109 |
  | jquerylil | 173 |

  What remains:
  - `JS.assume` on types the fix-it cannot spell (61);
  - `JS.or` and `JS.invoke` where their operands keep them (31, 30);
  - the catalog helpers (`regexTest` 24, `isArray` 17, `stringSlice` 15, `regexExec` 15, `stringReplace` 14, …);
  - typed-string `JS.add` in declined positions (22);
  - `JS.undefined()` where a module binds `undefined` (18);
  - `JS.array` in `auto` contexts (17).
- **Fix-patched ports** (S1–S7's rewrite applied, `s7-1`): all seven green, 4,653 of 4,653 tests. Against S5's fix-patched run (so the delta mixes S6–S7's patches with S6–S7's compiler):

  | Port | Files changed | Raw | Gzip | Brotli |
  |---|---:|---:|---:|---:|
  | markedlil | 7 of 7 | −272 | −78 | −97 |
  | jquerylil | 5 of 5 | +90 | −163 | +15 |
  | motionlil | 567 of 617 | +1,636 | +590 | +46 |
  | the other four | 0 | 0 | 0 | 0 |
  | **total** | 579 of 677 | +1,454 | +349 | **−36** |

  motionlil's 567 files are the delivery graph's parts: the partition moved code between parts (part-288 −10,639 raw, part-292 +10,588), and the sum over its 112 kB moved +46 Brotli. The patches are not in the port repositories yet (M12.4); their costs are recorded here for that release.
- **Step 7 closes** with the language slice built and the fix-it complete: R12, R7, R13 and R14 are in force on the one compiler.
  - The refusal of the spellings lands with each port's release (M12.4).
  - M4.2's remaining items move to step 9 with M4.4's node ids: type parameters by id, and interned types without source lifetimes.
  - `?.` stays with M9.3's optional-chaining family.


---

## 2026-09-28 Batch K1: trusted crossings (M10.12, R1)

**Pre-registration** (written before the first build of the batch; base: S7's last commit, baseline binary `~/lilscript-work/bin/s7-1`). Step 8, the core, starts here: M10.12 needs only Y1, answered yes on 2026-09-28 ("runtime shouldn't have explicit type casting behaviours").

What the batch builds (language.md R1): inside a program every value inhabits its static type; host values enter at declared crossings, trusted, with no code in production.
- **Loads and calls never normalize.** A typed `int` load (a field, a class field, a member, an element) and an `int` call result (a builtin or a call through a reference) are int32 by type: the `|0` the JavaScript lowering adds to them goes. The reads R2 and R11 own stay: `?? null` past an array's end or on a missing record key, and `?? ""` past a string's end (M10.9).
- **The int32 proof collapses.** An `int` value is an int32 by type. The exception is a load of a classic script's top-level cell, which another script may write until Y5 is answered. Parameters no longer need their callers' arguments checked. The counting loop's range reasoning, which drops `|0` on `c+1`, stays.
- **The effect obligations go.** A summary no longer records parameters assumed primitive, int32 or well-formed, or reliance on untraced typed data (`untrusted`). Under R1 no typed value reaches a conversion hook, so a call is removable on its effects alone.

Changes (one commit each):
- **C1.** Loads and call results never normalize.
- **C2.** The int32 proof by type.
- **C3.** The effect obligations deleted.
- **C4.** Tests: expectations that carried the normalizations; new tests of typed loads without `|0` and of a call removable with typed arguments.

Predicted:
- Byte changes wherever typed `int` loads were normalized: fewer `|0`, smaller in every codec on typed programs. Level 13 and above must not grow (AM2); every growth is named.
- Ports: small wins on typed code (markedlil, parts of zodlil and posthoglil); JsValue-heavy ports barely move.
- No behavior change on the suites. R1 changes meaning only for ill-typed crossings, which the development-check lane (M10.9) will catch.

---

## 2026-09-28 Batch K2: the reflected set (M10.14, checker half)

**Pre-registration** (written before the first build of the batch; base: K1's last commit). Its tests run with K1's.

What the batch builds (language.md R6): the checker computes the **reflected set** once for the whole program. These are the nominals whose property names, key order and identity a crossing shows the host.
- Seeds:
  - a value widened to `JsValue` or `unknown`, implicitly or with `as JsValue`;
  - a thrown value;
  - a host binding's parameters, result or value;
  - a root module's exports;
  - published classes and classes with a host ancestor.
- Closure: a reflected class's base and field types, a struct's field types, and the type arguments and function signatures a seed names.
- Published on the checked view as `is_reflected`.

Consumers follow their own tasks: field facts (M6.7, step 10) and property names (M9.6, step 11). Until then the set changes no output.

Changes:
- **C1.** The set: seeds, closure, the view's query, and a test (a crossing reflects its nominal and what its fields reach; a thrown class is reflected; a class that never crosses is not).

Predicted: no artifact changes.

---

## 2026-09-28 Batch B4: the walk's reserve (M3.5, AM2)

**Pre-registration** (written before the first build of the batch; base: K2's last commit).

**Found by S5's katexlil diagnosis.** A structural search with many optional alternatives spent the whole optional work budget before the level walk ran.
- With S5's rewrite, katexlil's main entry made 665 proposals, each one a render and a full-file codec probe (223 exact string definitions × 3 naming styles). The unpatched main entry makes 5.
- The search stopped at `WorkExhausted(Optional)`. The walk ran 0 passes (unpatched: 4 passes, 61,226 → 59,937 Brotli). The entry ended at 61,213, and took 524 s against 51 s.
- The rewrite itself costs +78 at the walk's start (61,304 against 61,226). The rest is the starved walk.

What the batch builds: the structural search spends at most half the optional work (search schedule 26), then stops as it does at its alternatives limit. The level walk, which finds most of level 13's bytes, always keeps the other half. This answers AM2 (13 and above never grow because of a change) for programs with many alternatives, and bounds the search's share of compile time (AM3).

Changes:
- **C1.** `BudgetLedger::work_limit`; `SearchLimit::WalkReserve`, checked before each optional proposal; schedule version 26.

Predicted:
- Programs whose search stays under half the optional work are byte-identical. Every current case and port is expected to, except possibly jquerylil, whose search makes 125 probes.
- katexlil rewritten by S5's fixes: its walk runs again, so the regression falls from about +1.3K to about +78 per file, which the string family's own cost then owns.

---

## 2026-09-28 Batch K3: the operation catalog (M4.6)

**Pre-registration** (written before the first build of the batch; base: B4's last commit). Its tests and ladder run with K1, K2 and B4.

What the batch builds: `src/catalog.rs`, the one place an operation's attributes are declared. The effect analysis, the call graph, the checker and the JavaScript target read them there.

Changes, each its own commit:
- **C1.** The intrinsics' tables, moved from the effect analysis, the call graph and the JavaScript target:
  - the effect class;
  - the reliance on a replaceable host builtin;
  - the callback, typed-array constructor and typed-array member classifications;
  - the JavaScript recipe: form, arity and the int32 facts of the result.
- **C2.** The builtins' host status, contracts and effect class (fresh, none, output or host), moved from `primitive.rs` and the effect analysis.
- **C3.** Host nodes by identity. The catalog classifies a host name once, where the JavaScript tree creates the node: a standard global by its identity (`Global`, 29), `eval`, `arguments`, or a declared host name. `Expr::Host` holds `Host { name, kind }`, and every test reads the kind: no host-name test remains on the tree. The standard builtins' known result types stay spelling-keyed, gated on the kind, until the platform catalog (M10.17).

Carried from M4.6:
- merging `CallTarget::Builtin` and `CallTarget::Intrinsic` into one operation identity in the IR, to step 9 (M5.2), which revises the IR's call representation;
- recognizing `Object.prototype.hasOwnProperty.call(o, k)` written through `JsValue`s as `Object.hasOwn`, and `new RegExp(valid literal)` as an effect-free construction in the IR, to the platform catalog (M10.17). The JavaScript tree already turns a literal `new RegExp` into a regex literal, and the typed `JS.hasOwn` and record `hasOwn` already print `Object.hasOwn` where the edition allows it.

Predicted: byte-identical everywhere (a move and an identity).

---

## 2026-09-28 Batch K4: definite assignment, first batch (M10.13, R3)

**Pre-registration** (written before the first build of the batch; base: K3's last commit).

What the batch builds (language.md R3):
- **Locals without an initializer.** `int x;` in a function body. Every read must be definitely assigned, which the checker proves over the statements:
  - an `if`'s branches start from the state before it, and join where they fall through;
  - a loop body, a lambda body, the catch and finally of a `try`, the right operand of `&&`, `||` and `??`, a conditional's arms and a match's arms may not run, so their assignments do not count after them;
  - a read of an unassigned local is refused with its span;
  - a compound assignment or an update reads its target first.

  A module's own bindings keep their initializers until the initialization order proves their reads (M6.5).
- **`let x;` in the IR.** `OperationKind::Declare(cell)`: the cell exists, with no value until its first store. JavaScript prints `let x;`; native declares the C local without a value.

Changes:
- **C1.** The checker's flow.
- **C2.** `Declare` through the IR, the targets and the passes.
- **C3.** Tests: the flow's acceptances and refusals, and programs with declared locals, run.

Not in this batch: a field's implicit default (R3's second half), which gets a warning with its fix-it and then the refusal, and `this` read before every field is assigned. Both are K5.

Predicted: unmodified programs are byte-identical, since none declares a local without an initializer.

## 2026-09-28 Batch K5: field initializers and the implicit-default warning (M10.13, R3)

**Pre-registration** (written before the first build of the batch; base: K4's last commit). Its tests and ladder run with the next batch's.

What the batch builds (language.md R3, first of its two batches for fields):
- **Field initializers.** `int count = 0;` in a class. The initializer is a value of the field's type, checked where `this` is not in scope. Each construction evaluates it before `init`, once per construction, and a derived class's construction evaluates its base's first. It is lowered at each construction under its class's module's facts, since the construction may be in another module.
  - An `extern class` field is the host's and takes none; a struct field takes its value from the construction literal. Both are refused with their spans.
- **`migration/implicit-default`.** A warning at each class field that has no initializer and that `init` does not assign on every path. Its fix writes the field's implicit default as its initializer, which is what every construction evaluates today:
  - `0`, `0.0`, `false`, `""`;
  - `[]`, `new Map()`, `new Set()`, `record {}`;
  - the enum's first variant, `null` for nullable and dynamic fields, and the buffers' zero-length constructions.

  A class, struct or function field that `init` leaves null has no default of its type, so it is reported without a fix.
- **The port rewrite** applies these fixes with `migration/js-builtin`'s, and counts them.

Changes:
- **C1.** Syntax: the initializer, and the struct refusal.
- **C2.** The checker: the initializer's type, and the extern refusal.
- **C3.** Lowering: each construction evaluates the initializer of the class that declares the slot, under that class's module's view.
- **C4.** The lint rule, its fix and its test; the docs.
- **C5.** Tests: constructions of generic, derived and kept classes, and the refusals; a case with the class in another module.

Not in this batch (R3's second batch for fields, with the port releases, M12.4): the refusal of implicit defaults, and of reading `this` before every field is assigned. Until the ports carry the fix, every field has a value when `init` starts.

Predicted: unmodified programs are byte-identical, since none writes a field initializer. A fixed port's output should match its unfixed output, since the fix writes the value the construction already stored.

## 2026-09-28 Batch K6: the `debug` class (M10.11 core, R15)

**Pre-registration** (written before the first build of the batch; base: K5's last commit). Its tests and ladder run with K5's.

What the batch builds (language.md R15, first batch):
- **`debug` declarations.** `debug void trace(string m) {…}` and `debug extern void invariant(bool ok, string m);`.
  - `debug` is a modifier only before `void` or `extern void`, where an identifier could not stand, so `debug` stays usable as a name.
  - Refused: with `pure` (the call has an effect), with `async` or `generator`, with a result, and on extern values and classes.
- **Stripping.** Under `strip_debug`, a call whose source callee names a `debug` declaration goes, and its arguments' evaluation stays.
  - Conversion decides it once, as a flag on the call, so a rewrite that exposes the callee (inlining a function that receives it as a value) never makes a call strippable.
  - The IR inliner keeps a `debug` body a body.
  - A call through a function value is not a direct call and runs.
- **Migration.** The name-keyed `debugLog` rule stays until the ports declare it. `migration/debug-class` writes `debug` on an `extern` named `debugLog` (jquerylil declares one; 24 port configs set `strip_console`, which is now `strip_debug`). The rule's second batch, with the ports' releases (M12.4), removes the name.

Changes:
- **C1.** Syntax and the language server's keyword help.
- **C2.** The IR: cells and calls carry `debug`; demand drops the calls; the inliner keeps the bodies.
- **C3.** `migration/debug-class`, and the docs.
- **C4.** Tests: parsing and refusals, the fix, and a case with a host prelude under `strip_debug`.

Predicted: unmodified programs byte-identical, since none writes `debug`; jquerylil's fixed port byte-identical to its unfixed build, since `debugLog` was already dropped by its name.

## 2026-09-28 Batch K7: typed intrinsics mean the originals (M10.15, R10)

**Pre-registration** (written before the first build of the batch; base: K6's last commit and the K1/K4 fix-ups). Its tests and ladder run with K5's and K6's.

What K1 already did: a typed call's `int` result carries no `|0` (R1). The one builtin that returns `int`, `Math.imul`, returns an int32 in its original, so that is R10's result half.

What the batch builds (language.md R10): every assumption typed code makes about the builtins stops depending on `assume_pristine_builtins`.
- **Integer intrinsics.** A typed length, position, size or `push` whose original returns an int32 prints without `|0`, and a length is bounded by 2^30 for the arithmetic next to it. Before this, both held only under pristine builtins.
- **Regex literals.** A typed `new Regex(p, f)` of literal strings in the proven subset is the literal `/p/f`, formed as such. The simplifier's gated rewrite stays for a `JsValue` construction.
- **The store fold.** A store to a key that a fresh literal already has updates an own data property, which shadows any inherited setter. It folds without pristine builtins; only a new key needs them. Initializer inlining, whose stores are to the instance literal's own keys, runs always.
- **Data tables.** The decoder is compiler-written code over compiler-made data: it takes `String` and `Array` methods to be the originals.
- **What stays gated.** `assume_pristine_builtins` now means the host's builtins are the originals where a `JsValue` operation reaches them:
  - `.call` receivers rewritten;
  - forwarding wrappers;
  - standard globals read as inert;
  - prototype stores grouped with `Object.assign`;
  - new keys folded into a literal.

Changes:
- **C1.** Integer intrinsics and lengths.
- **C2.** Regex literals.
- **C3.** The store fold and initializer inlining.
- **C4.** Data tables.
- **C5.** Tests and docs.

Predicted: the ports set `assume_pristine_builtins = true`, so they are byte-identical, with one exception. Where the fold used to push a new key, it still does, so the fold's output under pristine builtins is unchanged. Cases and ratchet items compiled without pristine builtins get smaller (regex literals, no `|0` on lengths, folded constructions).

## 2026-09-28 Batch K8: the development-check lane (M10.9, first part; R11, R1)

**Pre-registration** (written before the first build of the batch; base: K7's last commit). Its tests and ladder run with K5–K7's.

Why first: a rule whose meaning change is silent must first run every port suite in the development-check lane (language.md §14). That covers R11's index precondition, which M10.9's second batch makes production semantics, and R1's trusted crossings, which K1 already made production semantics without the lane. The lane checks index reads now; crossings and trusted views join it as their own batch.

What the batch builds:
- **The `checks` contract axis.** `javascript.checks = "production" | "development"`, default production. It is part of the contract and of the policy's fingerprint, and independent of effort.
- **Index reads under development.** A typed read `a[i]` of an array, a typed array or a string becomes `index_checked(a, i)`. That is one hoisted helper, formed once per module. It returns the element when `0 <= i < a.length` and throws a `RangeError` otherwise. The receiver and the index are evaluated once each, in the read's order. The absence recipes (`??""`, `??null`, `|0`) still apply to the element. A rest list spelled as formals has no array to check.
- **The lane.** `scripts/ports.mjs --checks development` sets the key in every configuration of a port's workspace, so every compile of its build checks.
- Not yet: native reads out of range still read 0, and trap only with batch 2. Crossing and view checks come later.

Changes:
- **C1.** The axis: contract, configuration, fingerprint.
- **C2.** The checked index read.
- **C3.** The port runner's lane.
- **C4.** Tests and docs.

Predicted: production builds are byte-identical (the policy JSON gains `"checks":"production"`). The lane's port runs show which suites read past an end.

## 2026-09-28 Batch K9: R11's first batch (M10.9)

**Pre-registration** (written before the first build of the batch; base: K8's last commit). Its tests and ladder run with K10's, or with the next batch's.

What the batch builds (language.md R11). The language additions come first, so that the fix-it's rewrite type-checks both before R11's second batch and after it:
- **Float `%`.** With a `float` operand, `%` is JavaScript's remainder, a `float`. Native uses `fmod`, and the interpreter the same.
- **Bitwise operators on floats.** A bitwise operator takes a `float` operand through ToInt32 and gives an `int` (`s.charCodeAt(i) | 0` stays valid once `charCodeAt` returns a number). Native uses `ls_to_i32`; the interpreter uses JavaScript's ToInt32.
- **`s.codeUnitAt(i) -> int`.** The code unit at an index in range by precondition. JavaScript spells it `charCodeAt` with no `|0`. Under development checks it reads `index_checked(s,i).charCodeAt(0)`.
- **The identity fold.** `x | 0`, `x ^ 0`, `x << 0`, `x >> 0`, `0 | x` and `0 ^ x` of an `int` are `x`, so the fix-it's `| 0` costs nothing while `charCodeAt` still prints its own `|0`.
- **`migration/char-code`.** A typed `s.charCodeAt(i)` gets one of two fixes:
  - `s.codeUnitAt(i)` inside `for (int i = k; i < s.length; …)`, with `k` a non-negative literal and neither `i` nor `s` assigned in the body;
  - `(s.charCodeAt(i) | 0)` elsewhere, today's meaning exactly.

  The ports use it 27 times at their current heads. The census above says 47, but that count predates the port rewrites.

Not in this batch: `charCodeAt` returning a number; `a.get(i)`, which lands when the development-check lane shows a port that needs it; and the precondition as production semantics. They form the second batch.

Changes:
- **C1.** Float `%`, ToInt32 bitwise operands and `codeUnitAt` through the checker, formation, native and the interpreter.
- **C2.** The identity fold.
- **C3.** The lint, its test and the scanner it reuses.
- **C4.** Tests, a case in every lane, and docs.

Predicted: unmodified programs are byte-identical except where they write `x | 0` of an `int`, which gets smaller. Fixed ports are byte-identical where the fix wrote `| 0`, and smaller by `|0` where it wrote `codeUnitAt`.

## 2026-09-28 Batch K10: R2's first batch (M10.9)

**Pre-registration** (written before the first build of the batch; base: K9's last commit). Its tests and ladder run with the next batch's.

What the batch builds (language.md R2):
- **`T??` is `T?`.** Wherever the checker builds a type: substituting `T?` with `T := U?`, and a destructured binding's optional element.
- **`migration/absence`.** A warning at each operation on a `T?` operand that could tell `null` from `undefined`, with today's meaning written out where it has one spelling:
  - `print(x)` and `string(x)` become `x ?? "null"`, since today absence prints as null;
  - typed `===` and `!==` become `==` and `!=`;
  - reported without a fix: a `JsValue` compared with `===`, `typeof`, a search for an absent value (`includes`, `indexOf`, `lastIndexOf`), and `sort` of a `T?[]`.

  A shared exhaustive expression walk (`lint/walk.rs`) serves the rule.
- **The port rewrite** applies the fixes with the others and counts the reports.

Not in this batch, R2's second batch:
- the refusals;
- the load normalizations' removal (`m.get(k)??null`, `a[i]??null`, `??""`), where the bytes are;
- a defaulted parameter's type `T` inside, with the default on absence. That refuses `x == null` inside, so it gets its own warning first;
- `T?` as a `Set` element or a `Map` key;
- reflected nominals' absent fields as missing keys.

Changes:
- **C1.** The collapse.
- **C2.** The rule, its walk and its test.
- **C3.** Docs.

Predicted: unmodified programs are byte-identical. The fixed ports' output changes only where `print` or `string` of an absent value would have printed "undefined".

## 2026-09-28 Batch K11: crossings in the development-check lane (M10.12's lane, R1)

**Pre-registration** (written before the first build of the batch; base: K10's last commit). Its tests and ladder run with the next batch's.

Why: K1 made R1's trusted crossings production semantics before the development-check lane existed. This batch puts the crossings in the lane, so every port suite can run with them checked (language.md §14).

What the batch builds, under `checks = "development"` only:
- **Crossing checks.** A typed value that crosses into the program calls a hoisted helper, formed once per shape. The helper throws a `TypeError` where the host breaks the declared type. The crossings:
  - an `extern` function's result;
  - a host method's result (a call through a place);
  - a trusted view's result (`v as T`, the `JS.assume` builtin);
  - each read of a typed host binding (`extern T x;`).

  The shapes and their tests:

  | Shape | Test |
  |---|---|
  | `int` | a number that `x \| 0` keeps |
  | `float` | a number |
  | `string` | a string |
  | `bool` | a boolean |
  | array | `Array.isArray` |
  | function | a function |

  Each is also offered with absence allowed, for `T?`. A class, struct, map or `JsValue` has no cheap test and is not checked.
- **Export parameters.** An exported function checks, at its entry, each parameter a caller must pass: no default (an omitted one is `undefined` until its default applies), passed by value, in plain storage, of a checkable shape.

Changes:
- **C1.** The helpers and the call and read hooks.
- **C2.** A test, and docs.
- **C3.** Export parameters, and their test.

Predicted: production builds byte-identical. The lane's port runs show which host values break their declared types.

## 2026-09-28 Batches K1–K4 and B4: the first run and its fix-ups

The five batches ran together (binary `k4-1`, ladder against `s7-1`). The run found bugs in K1 and K4 and tests that asserted the retired posture. The fix-ups are separate commits at the tip; the second run (below, with K5–K11) is the landing evidence.

**First run, what it found:**
- **Unit tests:** 1,573 passed, 33 failed.
  - K4 (2):
    - the rules' editor copied `Declare(cell)` with its cell unmapped, so an edited unit declared a stale binding and JavaScript refused the output;
    - a checker test read the old message.
  - K1, index reads: K1 removed the `|0` after every typed `int` load. For fields, class fields and members that is R1's. An element read is R11's business:
    - past the end it became `undefined`, where it reads 0 until the index precondition lands with M10.9's development-check lane;
    - a `Uint32Array` element stayed a uint32.
  - K1, narrowing (2): two facts tests exposed an unsound narrowing. A captured `T?` was narrowed inside a lambda that ran after the binding was set to null, and the facts, which now trust types, called its `.length` total.
  - K1, retired posture (25): the tests asserted "types are hints". Their host passed objects with conversion hooks, symbols or bigints through `int` and `string` crossings, or replaced a typed builtin.
- **Cases:** 3 changed state.
  - `definite_assignment` (new) was refused in production JavaScript (the editor) and rejected by the C compiler (native never declared the local).
  - `effects-raw_argument_conversion_in_a_discarded_call_still_runs` asserted the retired D2 posture.
  - `typed_arrays`: a `Uint32Array` read of `-1` gave `4294967295`.
- **Monotone and replay:** pass.
- **Ratchet:** verdict fail on AM2 growths only, +1 to +6 Brotli on struct, loop, nullish and min/max items. The 36 failures are S7's, unchanged. Totals improved:

  | Corpus | Brotli before | Brotli after |
  |---|---:|---:|
  | cases | 48,573 | 48,161 |
  | apps | 920 | 903 |
  | algorithms | 3,233 | 3,224 |

  The growth mechanism, seen on `minmax-scan-10-20-30-40`: with no `|0` on the index reads the text is 12 bytes shorter raw, and the statement spelling flips from `if` to `&&`, which costs 6 bytes of Brotli.
- **Unpatched ports:** all green. Against `s7-1`:

  | Port | Raw | Brotli |
  |---|---:|---:|
  | markedlil | | −129 |
  | zodlil | | −49 |
  | katexlil | −3,468 | +41 |
  | jquerylil | +3,160 | +138 |
  | posthoglil | +944 | +60 |
  | motionlil | | −125 |
  | **total** | +1,326 (gzip +377) | **−64** |
- **Fix-patched ports:** all green. katexlil's main artifacts moved raw +7.3 kB (+2.9%), gzip −1.9 kB and Brotli −1.2 kB each (60,151 for `katex.mjs`): B4's walk now runs and chooses for katexlil's Brotli objective. Totals: raw +73,951, gzip −19,346, Brotli −12,614.
- **katexlil's S5 snapshot (B4's evidence):** Brotli 61,213 → 60,072 (−1,141), in 281 s instead of 524 s.
  - The search stops at the walk's reserve.
  - The walk then runs 4 passes over 118 candidates, keeping conditional values −7, exit points −104 and start search −1,106.

  S5's katexlil regression is resolved.
- **CPU pairs:** my type checks overlapped them (load 3.7). They are not evidence; the second run's are.

**Fix-ups (commits at the tip):**
- **K4:** the rules' editor remaps a declared local; native declares it at its function's start; a declared local in an inlined body is a `let` there.
- **K1, index reads:** an `int` element read keeps its int32 conversion (`LoadResultRecipe::IndexInteger`), R11's until M10.9.
- **K1, narrowing is sound** (`check/assignments.rs`, after Kotlin's smart casts and TypeScript's `isSymbolAssigned`). A narrowing holds only where no code the flow does not see can assign the binding:
  - never for a binding a nested function assigns;
  - never inside a nested function for a binding its body assigns;
  - never for another module's binding;
  - for a host binding, until code runs: a call, a construction, an await or a yield ends it.
- **K1, retired tests:** where a property survives, the test keeps it with a well-typed host. Where none does, the test goes. The regression case became `effects-a_discarded_pure_call_keeps_its_arguments_host_call`.
- **K7 (found in the second run):** a regex literal leaves a `/` bare inside a character class, so its `source` is the constructor's (marked's `autolink`, `escapeRe` and `htmlPed`).

## 2026-09-28 Batch K12: R11's index precondition in production (M10.9, second batch, first part)

**Pre-registration** (written before the first build of the batch; base: K11's last commit and the fix-ups). It lands only if the development-check lane's port runs (K8, K11) are green: an index read past an end would throw there.

What the batch builds (language.md R11):
- **An element read is the plain read.** An `int` element (array or typed array) carries no `|0`, and a `string` element (a `string[]` or `s[i]`) no `??""`. In range by precondition, each is its type's value. A `Uint32Array` element keeps its conversion, since a uint32 is not an int32.
- **Absence stays for R2's second batch.** A record's missing key, a `T?` element and `Map.get` keep `??null`. Dropping those needs the crossing normalization R2 describes: a `T?` that reaches a `JsValue` or a print is pinned to one spelling. That in turn needs the use analysis through locals (the cell-SSA view, step 9). The draft of those refusals is kept in `~/lilscript-work/portwork/k12-r2-draft.patch`.
- **Tests stop reading past an end.** In production that is a precondition violation, whose result is unspecified.

Not in this batch:
- `charCodeAt` returning a number. That refuses `int c = s.charCodeAt(i)`, which the unpatched ports write, so it waits for their releases (M12.4).
- Native's trap past an end. Native returns a memory-safe default today.

Changes:
- **C1.** The load recipe.
- **C2.** Tests and docs.

Predicted: smaller in every codec wherever typed code reads an `int` or `string` element (the `|0` and `??""` go). The ratchet and the ports show the size. A spelling flip like the first run's minmax case is judged per case (AM2).

## 2026-09-28 Batch K13: int32 hints, a codec-judged family (AM2 for R1, R10 and R11)

**Pre-registration** (written before the first build of the batch; base: K12's last commit).

Why: the second run's ratchet blocked on 27 items that grew 1 to 5 Brotli bytes (AM2). Each is strictly smaller raw: the only change is a `|0` that R1, R10 or R11 made redundant. For example, `edge-map-set-1-1-2` drops `b.size|0` for `b.size`: raw −2, Brotli +4. On a 60–180 byte artifact the codec's repeat matching prefers the repeated `|0` (memory: repetition is load-bearing). Keeping those `|0`s everywhere would give back what the batches gained on the ports (markedlil −129, motionlil −125). So the codec judges it per artifact.

What the batch builds:
- **`int32_hints`, an output family.** It prints the `|0` the compiler printed before R1, R10 and R11, by the same rules:
  - after an `int` field, member or element read;
  - after an `int` result of a host method or builtin call;
  - after an integer method's result without pristine builtins.

  An int32 is its own ToInt32, so the spelling means the same program.
- **Decided at the head.** The `|0` nodes change what the head's passes do (operator simplification, forwarding), so the family is decided where the head is formed: `form_head` takes it. The terminal stage forms the head with the incumbent's value, and forms the other head, once, when the family's challenger first asks.
- **Seed and schedule.** Every objective seeds it off, the new spelling. Its challenger is first in the declared schedule, so a searching level keeps the previous spelling wherever the codec says the artifact is smaller. A level that does not search keeps the new one (AM2 binds level 13 and above).

Changes:
- **C1.** The family, its challenger and the head's decision.
- **C2.** A test, and the record.

Predicted:
- The ratchet's 27 growths close at level 13.
- The ports keep their gains where the codec says they are gains (a fleet total no larger than the second run's).
- CPU: one extra head formation per terminal stage that tries the family.

## 2026-09-29 Batches K5–K13 and step 8's close: the second and third runs, and their fix-ups

The second run (binary `k9-1`: K1–K9 with the first run's fix-ups) and the third (`k13-1`: K1–K13 with every fix-up before it) both ran against `s7-1`, the last landed binary. The fix-ups the third run found are commits at the tip, measured on `k13-2` (below).

**Second run (`k9-1`), what it found** (fixed before the third):
- **Unit tests:** 17 failures.
  - Counts and oracles that K1, K7 and K9 moved: the admission work counts (the scan's own unit), the frozen operator oracle (R11's operands), the binary64 and tables tests, a K7 test that asserted `length|0`.
  - Tests of replaced builtins and of unsound narrowings, gone with R10 and R1.
  - A regex literal's `source` (the K7 fix-up in the first record).
  - Narrowing of another module's binding.
- **motionlil refused** at `stagger.lil:41` and `consumer.lil:62`: the first fix-up's narrowing rules were stricter than soundness needs. Refined:
  - an inherited narrowing is masked inside a lambda only for a binding its declaring body assigns;
  - a narrowing the lambda makes itself holds until an await or a yield;
  - a host binding stays narrowed until code runs.
- **The `unwrap_barrier` case:** an extern global narrows until a call.
- **`migration/char-code`** re-reported a `charCodeAt` already under `| 0`, so the rewrite never reached a fixed point.
- **The fold round** did not follow a chain of substitutions made in the same round, and every fix-patched build failed with "unsupported source: program rules". The round now resolves each substitution through the chain.
- **CPU pairs:**

  | Port | Factor |
  |---|---:|
  | markedlil | ×1.675 (the walk judges 21 moves, 9 before: B4) |
  | posthoglil | ×1.366 |
  | zodlil | ×1.056 |
  | katexlil | ×0.976 |
  | jquerylil | ×0.986 |

**Third run (`k13-1`), what it found:**
- **Unit tests:** 1,606 passed, 6 failed.
  - Narrowing: `narrowed_type` returned nothing outside every body (an analyzer driven expression by expression), so a test's narrowing never held.
  - The substitution test's oracle predates R2's `T??` is `T?`.
  - Timing: K13's other head is a head formation the test did not count.
  - The terminal test asserted that raw and Brotli keep different families on its program. Since R10 and R11 they keep the same nine.
  - `declared_locals_run` and `field_initializers_run_at_construction`: dead-code elimination dropped a `Declare` (next item).
- **Cases:** six lanes of `definite_assignment` refused in production ("binding has no declaration"). The demand plan kept a declared cell's stores and reads but not its `Declare`, which fell to the default arm; `Declare` now waits on the cell's storage as `Initialize` does. Totals against `s7-1`, all lanes passing otherwise:

  | Lane | Script | Module |
  |---|---:|---:|
  | production Brotli | 39,160 → 38,284 (−876) | 37,268 → 36,520 (−748) |
  | production gzip | −899 | −764 |
  | production raw | −1,875 | −1,730 |
  | formation-only Brotli | −241 | −244 |
- **Monotone and replay:** pass at every listed level; 45,513 recorded stops replayed.
- **Ratchet** against `s7-1`:

  | Corpus | Raw | Gzip | Brotli |
  |---|---:|---:|---:|
  | cases | 64,807 → 62,330 | 58,522 → 57,497 | 48,573 → 47,249 |
  | apps | 1,359 → 1,246 | 1,078 → 1,026 | 920 → 861 |
  | algorithms | 6,047 → 5,798 | 3,542 → 3,505 | 3,233 → 3,197 |

  Brotli losses to the competitor bar: cases 422 → 368, apps 5 → 4. The apps now beat the competitor total (861 against 870). Sixteen rows blocked, on 7 items that grew 1 to 4 Brotli bytes: three nested-struct variants, `string-concat-typed-aggregate`, `minmax-scan`, `edge-string-utf16-accent` and `-empty`, and `algorithms/helper-sharing`. The mechanism is a pair of moves the walk tries one at a time:
  - `nested-struct-3-1-4-1-5-9`: int32 hints alone +3, block-scoped loop heads alone +2, both together is `s7-1`'s artifact, −1;
  - `edge-string-utf16-empty`: hints alone +0, a pooled `"lilscript"` alone +3, both −4. K7 made the second root-constant forwarding unconditional, and it forwards the literal into all five reads.
- **Unpatched ports:** all green. Against `s7-1`:

  | Port | Raw | Gzip | Brotli |
  |---|---:|---:|---:|
  | markedlil | +519 | −88 | −83 |
  | zodlil | +210 | −139 | −56 |
  | katexlil | +8,797 | +929 | +749 |
  | jquerylil | +155 | −55 | +17 |
  | posthoglil | +564 | −27 | −106 |
  | motionlil | −161 | −65 | −240 |
  | micromarklil | −176 | +43 | +108 |

  katexlil's +749 is walk path dependence. On its entry the level-0 artifact is 23 bytes larger (60 fewer `|0`, 84 more `return`s). The statement families `s7-1` kept (conditional values, exit points, conditional returns) are each rejected alone in the new walk, which ends at 60,022 against 59,937. On the frozen CPU snapshot of the same port the walk runs 9 passes and ends 94 bytes under `s7-1`. micromarklil's and jquerylil's are tens of bytes per artifact in both directions.
- **Fix-patched ports (production):** all green. Against `s7-1`'s fix-patched run: raw +82,720, gzip −19,483, Brotli −12,476. katexlil −11,727, motionlil −437, posthoglil −106, zodlil −93, jquerylil −75, markedlil −61, micromarklil +23. katexlil's patched build takes 871 s against 1,536 s.
- **Development-check lane** (K8's index checks, K11's crossing checks, on the fix-patched ports): five ports green. Two found port-source violations, as the lane is meant to:
  - micromarklil (1,796 failing tests, then 1,236): its HTML compiler reads the last chunk of an empty buffer, and the last flag of the tight stack outside any list. Upstream reads past the end and treats `undefined` as nothing.
  - motionlil (5): `parseFloat` was declared on `string` and given numbers through `asStr`; the numeric mixer claimed its second endpoint a `float` (`mix(0, "10")`); `noop` was a `float` identity while the frame loop and the public API pass it any value.

  Fixed in the ports' sources (local commits `micromarklil d2c2fd1`, `motionlil f6bc890`). Their output behaves as before, and both are green in production and in the lane. K12's gate holds: every port is green in the development lane. None of the findings was a read whose production meaning K12 changed.
- **CPU pairs:**

  | Port | Factor | Note |
  |---|---:|---|
  | markedlil | ×1.013 | |
  | zodlil | ×2.455 | judged 9 → 18; overlapped a type check |
  | posthoglil | ×1.728 | judged 11 → 23; overlapped a type check |
  | micromarklil | ×0.988 | |
  | katexlil | ×2.210 | frozen snapshot: 9 passes instead of 4, judged 66 → 156, 94 bytes smaller |
  | jquerylil | ×0.987 | |
  | motionlil | ×0.786 | |

  The port builds show no such growth (katexlil 140 s → 139 s unpatched). The int32-hints trial rendered identical bytes in each of katexlil's 9 passes: it formed a tail and rendered for nothing.
- **Rewrite census** (first pass, the K5–K10 fixes): implicit defaults 20 (markedlil 5, zodlil 2, motionlil 13); `char-code` 21; a `debug` declaration 1 (jquerylil); absences 216 reported, 0 of them in markedlil.

**Owner, 2026-09-29, on the growths:** "we cant always win, we must accept loss sometimes.. for pragmatistic approaching of the problem.. maybe this lose can mean better win else and its a step through global maximum .. be clever, and pragmatic.. overall general win might be a win for real." So a batch is judged by its totals (ratchet, cases, fleet), and a scattered growth of a few bytes on one item, or walk path dependence on one port, does not block when the overall wins. Extra search stays only if it pays for its compile time.

**Fix-ups after the third run** (commits at the tip):
- **Narrowing:** outside every body a narrowing holds as made.
- **Demand:** a `Declare` lives while its cell's storage does.
- **Tests:**
  - the substitution oracle follows R2;
  - the timing test counts the heads the stage reports, a new `heads` count;
  - the terminal test asserts that the three codecs do not all judge alike, and counts every start the portfolio does not hold as a restart;
  - K5's test keeps its derived class as JavaScript's through `export constructor`, the declared boundary. A plain `export class` failed in the single-source harness only: that path publishes a class's name as a value, where the module path publishes a type (a separate task).
- **The int32-hints family skips an inert head.** Formation counts the reads and call results the family would hint, whatever its value. A head with none, and with no integer intrinsic the printer would hint, marks the family inert, and the walk records the trial a duplicate without forming a tail. katexlil's walk formed and rendered one for nothing in each of its 9 passes.

**Fourth run (`k13-2`, the fix-ups with two restarts behind a temporary switch):**
- **Unit tests:** 1,606 passed, 6 failed: five terminal tests that recognized a restart by its `naming:` prefix, and K5's test (above). Both fixed.
- **Cases:** pass in every lane, `definite_assignment` included.
- **The restarts, on and off:** one from A0 with the other int32 hints, one under the other objective's family seed, each walked in passes of its own as the naming restarts are. They would reach the pairs above.

  | | Restarts on | Restarts off |
  |---|---:|---:|
  | Ratchet, cases Brotli | 47,242 | 47,249 |
  | Blocking rows | 9 | 16 |
  | katexlil (port entry, frozen entry) | 60,022, 60,025 | same |
  | zodlil, posthoglil, markedlil | same bytes | |
  | CPU: zodlil, posthoglil, markedlil | +11%, +22%, +13% | |
  | CPU: katexlil | +0.4% to +2% | |

  On the ports the seed restart is pruned at its start every time and the hints restart ends rejected. They buy 7 bytes on the ratchet for a tenth to a fifth more compile time on the typed ports. Under the owner's ruling they do not pay, so they are not landed. The 16 rows (7 items, 1 to 4 Brotli bytes each) stand as accepted losses against the totals above.

**Final check (`k13-3`, the landed code):** 1,612 unit tests pass, and the ratchet's 1,890 artifacts are byte-identical to `k13-2`'s restarts-off run.

Step 8 closes with this record. Carried:
- R2's second batch: normalize at the crossings instead of the producers, with the refusals (draft kept at `~/lilscript-work/portwork/k12-r2-draft.patch`);
- R11's `a.get(i)` returning `T?`, and `charCodeAt` returning a number, at the port releases (M12.4);
- the refusals of every warning K4–K10 added, each with its port's release (M12.4);
- the ports' source fixes the development lane found (micromarklil, motionlil), committed locally, published with the ports;
- the implicit default a field keeps when `init` assigns it (`this.later=0;this.later=…`), a store a later rule can drop;
- a generic function value instantiated at a function type (`noop<T>` where `func(float)->float` is expected), refused today: the checker's gap, found by motionlil.

## 2026-09-29 Batch N1: node ids (M4.4)

**Pre-registration** (written before the first build of the batch; base: step 8's merge, `2a647409`).

Why: a span is a location, not an identity. Two nodes can share one (generated syntax, an export of a declaration's own name), and a fact keyed by span makes every consumer carry spans to ask about a node. M5.2's annotation columns need node identities first (plan step 9: M4.4 before M5.2).

What the batch builds:
- **Identifiers carry node ids.** `Ident` gets `id: SourceNodeId`, allocated by the parser's node sequence like an expression's, and a function's and a constructor's `this` is an identifier of its own.
- **The checker's facts are keyed by node; the span-keyed maps are deleted.**
  - An identifier's symbol and a declaration's binding, by identifier.
  - A type test's target and an optional access's present type, by expression.
  - An enum variant's value, by the variant's identifier (in `E.V` and in a `match` pattern).
  - A dynamic import's module, by expression.
  - The module bindings declared ahead of their module's items, by declaring identifier.

  A symbol records its declaring identifier (`Symbol::node`), and an interface import its local identifier, so a consumer holding a symbol can ask about its declaration.
- **The consumers look up by node:** lowering, the interpreter, the lints and the language server. The lint's identifier walk moves to `ast_walk` with the expression walk and yields identifiers. The language server renames through them instead of re-lexing, and the checker's default-arrow capture tests read an arrow's identifiers instead of comparing spans.
- **Choice keys name a source identity** (`SiteId`): a data table's key is the source symbol of the binding it initializes, or the binding's tree ordinal where formation created the binding.

Declarations are identified by their declaring identifier's node. Statement ids are carried to M5.2: no fact about a statement exists yet, and its first column gives statements their ids.

Changes:
- **C1.** Identifiers carry node ids, the checker's facts are keyed by node, and the consumers follow, with the shared walk.
- **C2.** Choice keys name a source identity.

Predicted:
- Byte-identical output on the cases and the ratchet against `k13-3`, except where two data-table sites tie on stake. The key breaks such ties, and the new order lists symbols before formed bindings.
- CPU neutral.

**Landed** (binary `n1-1`):
- **Unit tests:** 1,609 passed, 3 failed. Two counted a program's nodes without its identifiers, and one built an expression whose identifier took the id before it. Fixed; the 3 pass.
- **Cases:** all lanes pass; 0 of 4,431 artifacts changed against `k13-3`, and the walk counts are identical.
- **Ratchet:** all 1,890 artifacts byte-identical to `k13-3`'s. No data-table tie changed order on these corpora.
- Carried: statement ids to M5.2 (above); type parameters by id and interned types (M4.2's rest), batch N3.

## 2026-09-29 Batch N2: checker facts transported (M4.3's rest)

**Pre-registration** (written before the first build of the batch; base: N1's last commit).

Why: lowering still decides two things from syntax and types that the checker already knows. It picks an int32 operation from an expression's result type, and it reads `pure` and `debug` off the declarations' syntax (M4.3: the checker records, elaboration reads).

What the batch builds:
- **`ResolvedOperator`.** The checker records how each arithmetic operator resolves:
  - `Int(IntBinary)` where the value is an `int` (or an enum's);
  - otherwise `Plain(BinaryOp)`.

  It decides for a binary expression by its result, and for a compound assignment or an update by its target. Lowering reads it, and `binary_kind`, its re-derivation from result types, goes. Arithmetic needs `int` or `float` operands, so a result type is always concrete and the decision is lowering's exactly.
- **Declaration attributes.** A symbol records `Attributes { pure, debug }`, and lowering reads them from the symbol (a method's `pure` from its checked info). An extern's attributes are part of its contract: a module that declares an extern with other attributes than another module's is refused, as a different type is. Today the last module's declaration wins. No reference port declares an extern two ways.
- **Ambient `this` and `arguments`:** nothing to build. The checker declares a method's and a constructor's `this` (an identifier since N1) and a receiver lambda's `this` parameter, and R7's rest parameters replaced `arguments`.

Carried: parameter defaults on declarations. Moving them out of function types means a function type carries optionality and the callee applies a default (R2's "defaults apply to omitted or absent arguments"), a language-level change. It is its own batch after N3.

Changes:
- **C1.** `ResolvedOperator`.
- **C2.** Declaration attributes.

Predicted: byte-identical output everywhere against `n1-1`.

**Landed** (binary `n2-1`): 1,613 unit tests pass (C3 adds a test that a module declaring an extern with other attributes is refused); every case lane passes with 0 artifacts changed against `n1-1`; the ratchet's 1,890 artifacts are byte-identical.

## 2026-09-29 Batch P1: the tree's first annotations (M5.2, first batch)

**Pre-registration** (written before the first build of the batch; base: N2's record).

Why: M5.2 puts facts on the JavaScript tree's nodes, renumbered by the arena itself, so that no caller remaps anything. Three facts still live beside the tree:
- `binding_classes` and `defined_parameters`, sparse lists on the module;
- `literal_alternatives`, the string literals the source observes only for truthiness or nullishness. Formation owns them, remaps them by hand after each renumbering (about twelve places), and copies them into a `protected` slice for each of five passes (M5.2: "the hand remaps and the `protected` protocol go").

What the batch builds:
- **Binding facts on the binding.** `Binding::class` (what formation knows it always holds) and `Binding::defined` (a parameter whose type excludes `undefined`) replace the two lists.
- **Observed literals on the module.** `Module::observed_literals` is renumbered inside `renumber`, with its expressions. A pass asks `Module::observed(id)` instead of taking a `protected` slice. Formation's copy, the slices and every hand remap go. A formed head and a tail carry just the module, and preparation reads the module's rows where it read the target's.

Only expressions are renumbered (bindings, regions and functions keep their ids), so a fact on a binding needs no transfer, and the observed literals' transfer is the renumbering map.

Changes:
- **C1.** Binding facts on the binding.
- **C2.** Observed literals on the module; the `protected` protocol goes.

Predicted: byte-identical output. Tests that sum a module's retained storage follow the moved facts.

Next (P2 onward): typed mutation helpers with the journal, then the evaluation-behaviour and binding-fact columns that let `quiet.rs` go (L20: only once they pass their debug verification).

**Landed** (binary `p1-1`, one commit: the module's storage sums change for both halves): 1,613 unit tests pass; every case lane passes with 0 artifacts changed against `n2-1`; the ratchet's 1,890 artifacts are byte-identical. `binding_classes`, `defined_parameters`, formation's literal list, its twelve hand remaps and the five passes' `protected` slices are gone.

## 2026-09-29 Batch P2: the evaluation-behaviour column on operation nodes (M5.2)

**Pre-registration** (written before the first build of the batch; base: P1's record).

Why: the tree's legality tests read syntax where the program has facts. `quiet.rs`'s walk treats every call, construction, integer operation and `|0` as running code. So `let v=V;f(x+y|0,v)` never forwards `V`, though R1 makes `x+y|0` on `int`s a primitive operation, and a call to a function the effect analysis proves pure is treated like a host call. The demand plan already holds each IR operation's `EvaluationBehavior` (reads, writes, throw, divergence, re-entry, suspension; `facts.rs`'s local behaviour, which the architecture's evaluation column replaces).

What the batch builds:
- **The column.** `Module::behaviours`: for a call, construction, integer operation, negation or `|0` that formation forms from an operation, that operation's behaviour, sorted by node.
- **Self-validating rows.** Until the journal lands, each row also holds the node it describes, and the arena renumbers both. A lookup answers only while the node at that id is still exactly that node, so a pass that rewrites a node in place drops its fact (a conservative top) without knowing the column exists. Inlining's template copies carry their nodes' rows, and a call site replaced by its template's root takes the root's row.
- **The readers.**
  - `quiet.rs`'s walk takes a node whose behaviour reads and writes no memory other code can change, cannot throw, and neither diverges, re-enters nor suspends, as it takes a literal: quiet when its operands are.
  - `runs_no_user_code` takes such a call as running no user code: the program's own code, and no host code.

Soundness: the behaviour is the operation's, from the effect analysis. The node is the operation's own, or a copy that evaluates the same callee on the same kind of arguments (inlining); a node rewritten in place loses its row.

Predicted: more forwarding on typed code (Brotli smaller or equal on the ratchet and the ports, a few byte changes elsewhere); CPU: one row per operation node formed, and a comparison per lookup.

**Landed** (binary `p2-1`):
- **Unit tests:** 1,613 pass, the storage-sum tests included (a row's copied arguments are charged).
- **Cases:** every lane passes. The production script lanes carry the change: Brotli −520 (31 artifacts), gzip −541, raw −788. Module lanes −5, −3, −4 (1 artifact); C unchanged.
- **Ratchet** against `p1-1`:

  | Corpus | Raw | Gzip | Brotli |
  |---|---:|---:|---:|
  | cases | 62,330 → 61,784 | 57,497 → 57,078 | 47,249 → 46,833 |
  | apps | 1,246 → 1,232 | 1,026 → 1,018 | 861 → 849 |
  | algorithms | 5,798 → 5,770 | 3,505 → 3,491 | 3,197 → 3,185 |

  Brotli losses to the competitor bar: cases 368 → 350, apps 4 → 3. No new growth: the 16 blocking rows are step 8's. Under the owner's ruling they are accepted into the baseline with the new `--accept-growth`, which accepts growth only when no corpus total grew; the baseline is now `p2-1`'s.
- **Unpatched ports** against `k13-1` (N1, N2 and P1 were byte-identical): all green.

  | Port | Raw | Gzip | Brotli |
  |---|---:|---:|---:|
  | katexlil | −8,204 | −1,328 | −829 |
  | motionlil | +394 | −4 | −414 |
  | micromarklil | +934 | +223 | +208 |
  | markedlil, zodlil, posthoglil, jquerylil | 0 | 0 | 0 |
  | **total** | −6,876 | −1,109 | −1,035 |

  katexlil's step 8 regression (+749) is more than reversed. micromarklil pays about 23 bytes per artifact for forwarding it does not compress as well: a local loss under the overall win.

## 2026-09-29 Batch P3: the raw spelling dissolved, the printer's statement rewrite gone (M8.3, first half)

**Pre-registration** (written before the first build of the batch; base: P2's record).

Why: M8.3 wants a pure printer. `Plan::raw_spelling` bundles five spellings the codec cannot judge separately:
- self-named functions (naming);
- the root's most read bindings taking the shortest names (naming);
- compound assignments (printer);
- the least-escaping quote (printer);
- logical statements, which it forces on.

Under it the printer also rewrites structure: `if(c)return a;return b` as `return c?a:b`, consuming the next statement, the same rewrite the tail's conditional-return and conditional-value families make on the tree.

What the batch builds:
- **Five members, each its own family.**
  - Naming: `self_named` and `read_order`, in the naming plan.
  - Printing: `compound_assignments` and `quotes`, output families that formation writes onto the module as print decisions, as the loop-head and logical-statement ones are. The printer reads the module, never the naming plan.
  - Logical statements: a family on its own, no longer forced by any other.

  Their challengers replace `raw-spelling` in the schedule, in its place: `self-named-functions`, `read-order`, `compound-assignments`, `quotes`. The two naming ones form nothing and only re-render. The raw objective seeds all five on, as the bundle was; the codecs seed them off.
- **The printer's conditional statement goes.** `return c?a:b` and `x=c?a:b` come from the tree's statement families only.

Loop heads and logical statements stay module-wide for now; per-site spelling attributes are M9.3's per-site choices.

Predicted:
- The raw seed prints what the bundle printed, except where the printer's rewrite caught a site the statement families left.
- Each objective's walk can keep a member alone. Brotli and gzip were measured to want some of them off inside the bundle: quotes +53 and +80 on two ports, read order +647 on zodlil.
- A codec objective can now try members one at a time (they seed off), and the raw objective can drop one it measures larger.
- CPU: three more challengers per pass, two of them render-only.

**Landed** (binary `p3-1`, and a fix-up of three tests and the NO3 ledger):
- **Unit tests:** 1,613 pass after the fix-up.
  - The veto test lets the naming members run without formation.
  - The shortest-forms test reads the raw seed's families at level 0.
  - The quote rule's two ledgered measurements moved with it to `families.rs`.
- **Cases:** every lane passes; raw is unchanged in every lane, so the raw seed prints what the bundle printed.

  | Lane | Script | Module |
  |---|---:|---:|
  | Brotli | −10 (9 artifacts) | −12 (8 artifacts) |
  | gzip | −4 | −3 |
  | raw | 0 | 0 |

  The walk examines about 18% more positions.
- **Ratchet:** passes against `p2-1`'s baseline; totals unchanged.
- **Unpatched ports** against `p2-1`: all green.

  | Port | Raw | Gzip | Brotli |
  |---|---:|---:|---:|
  | micromarklil | −1,427 | −245 | −309 |
  | katexlil | −1,047 | −203 | −205 |
  | jquerylil | −290 | −35 | −171 |
  | markedlil | −8 | −22 | −43 |
  | zodlil | 0 | 0 | 0 |
  | posthoglil | +270 | +144 | +25 |
  | motionlil | +501 | +107 | +320 |
  | **total** | −2,001 | −254 | −383 |

  micromarklil's P2 loss is more than reversed. motionlil's growth is spread over 205 of its 623 small artifacts.
- **CPU pairs** against `p2-1`:

  | Port | Factor | Judged |
  |---|---:|---:|
  | markedlil | ×1.17 | 9 → 13 |
  | zodlil | ×1.21 | 18 → 24 |
  | posthoglil | ×1.04 | 23 → 24 |
  | micromarklil | ×1.86 | 10 → 25 |
  | katexlil | ×1.13 | 102 → 118 |
  | jquerylil | ×1.04 | 33 → 42 |
  | motionlil | ×0.95 | 38 → 35 |

  The three added challengers are the cost. The members now judged one by one buy the ports' −383 at level 13, where size comes first (AM2 as ruled on 2026-09-29).

## 2026-09-29 Batch D1: the region-structured solver and the cell-SSA view (M5.5)

**Pre-registration** (written before the first build of the batch; base: P3's record).

Why: M7.4's reaching-definition forwarding and M6.4b's ranges need flow-sensitive facts on the Program IR. The IR is region-structured and its values are already SSA, but a cell (a mutable local) has no view of what each load reads (M5.5: "one region-structured dataflow solver; the cell-SSA view"). The call graph with its SCCs, M5.5's first part, is already `call_graph.rs` (M6.1, computed with the effects).

What the batch builds:
- **`dataflow.rs`, one forward solver** over a unit's region tree. An analysis gives a state, a join and a transfer; the solver gives the flow:
  - branches join;
  - loops iterate to a fixed point, with `break` and `continue` routed to their loop and a test that is the constant `true` never exiting;
  - a catch starts from the join of its body's states;
  - a finally region's result reaches every target it guards;
  - `return` and `throw` end a flow.

  A loop still unsettled after one iteration per operation (plus two) ends the solve as `Unsettled`, never with a partial answer.
- **`cell_ssa.rs`, the cell-SSA view.** For each whole-cell load of a cell only its owner defines (no other unit writes it, no call takes it by reference), the definitions that may reach it: the entry (a parameter), an initialization, a declaration, a store to the cell or to a field of it, a `for…in`/`for…of` binding, or a catch binding.

Changes:
- **C1.** The solver, with tests (reachability after abrupt exits, a lattice that never settles).
- **C2.** The view, with tests (a straight line, branches, a loop, a `break`, a cell a nested function writes).

Predicted: byte-identical output; its first reader is F1 (M7.4), verified with it.

## 2026-09-29 Batch J1: the edit journal and one rule scheduler (M5.2's journal, M5.3a)

(Its commits are titled `S1`; it is J1 here, to keep step 7's batch S1 distinct.)

**Pre-registration** (written before the first build of the batch; base: D1's pre-registration, whose verification this batch carries).

Why: the tree's rules still run as a hand-written chain (`src/program/javascript.rs`, 54 calls):
- the order and the rounds are fixed by hand: `for _ in 0..3`, `fold_stores`'s four, and "if the last pass changed something, run this one";
- nothing records what a pass changed.

M5.2 asks for typed mutation helpers that journal every edit, checked against the actual difference. M5.3a asks for one journal-driven scheduler: rule sets to their fixed point, a round ceiling that fails the build, the verifier after each round in debug builds, and the chain deleted as a chain.

What the batch builds:
- **C1. The journal** (`src/js/journal.rs`).
  - Every edit of a tree slot that existed when a rule started goes through a helper: `set_expression`, `expression_mut`, `statements_mut`, `set_statement`, `set_region_scope`, `binding_mut`, `function_mut`, `tables_mut`, and the root-row helpers.
  - Each helper records the region, node, binding, function or table it edits. `renumber` carries the node entries.
  - About 60 direct writes across 13 files are routed through the helpers.
  - Test and debug builds check a rule's journal against the actual difference. Every changed region still reachable, every binding, function and table must be recorded, compared by structural digest so a renumbering changes none. A rule that recorded an edit must have changed something.
- **C2. One scheduler** (`src/schedule.rs`) for the program rules and the JavaScript target rules.
  - The program rules' own loop moves onto it unchanged.
  - The JS passes become data (`src/js/rules.rs`), in the chain's relative order, each marked transitional with the task that deletes it (§8.2's table). The four fact-free rules are classified, and test builds assert that each of their applications strictly decreases the measure (reachable functions, then reachable nodes): `elide_undefined`, `drop_unreachable`, `drop_bare_blocks`, `drop_double_negations`.
  - Three rule sets:
    - the head (family-independent);
    - the tail (the artifact's families);
    - string pooling.

    Each runs to its fixed point: a round in which no rule recorded an edit.
  - A ceiling of 64 rounds fails the build.
  - Test and debug builds verify the tree after every round.
  - The chain's hand rounds, its conditionals on what changed, and `fold_stores` are deleted. The inliner's limit 6 stays as a parameter of its transitional rule, deleted with it (M7.5a).

Predicted:
- **Unit suite:** passes with the journal checks active; an unrecorded edit the checks find is routed in this batch.
- **Output:** changes, since every rule set now reaches its fixed point (a rule that exposes work for an earlier one gets it next round).
  - Expected: small net decreases from more forwarding and folding, with scattered growths from the new interleavings.
  - The fleet should not move by more than ±0.5%.
  - Judged by the totals (2026-09-29 ruling).
- **CPU:** up on formation. Each rule set takes at least two rounds (the last confirms). Formation is about 15% of markedlil's and 20% of katexlil's compile time (`LILSCRIPT_TIMING`, `p3-1`: 184 of 1,240 ms and 14.3 of 72.2 s), so expect +5% to +15%. The remedy is M5.7's dirty-unit scheduling.
- **D1:** the solver and the cell-SSA view have no reader yet; this batch's runs verify them.

**Landed** (binary `j1-1`, and a fix-up). This batch's first run wrote to `bin/s1-1` and `out/s1`, which overwrote step 7's binary (SHA-256 `4fb1870b726a1bd4…`) and four of its result files. O1's run did the same to step 8's `bin/b1-1`. Both old binaries rebuild from their batches' commits, and their numbers stand in their records.
- **Unit tests:** 1,626 pass, with the journal checked on every rule application in every formation the suite runs. The checks found no unrecorded edit. The first run found five failures, fixed in the fix-up:
  - `encode_tables` recorded its choice sites afresh on each run, so the round confirming the fixed point erased them. It now records each site once, and the tail clears them when it starts: a rule must be idempotent in what it records, not only in what it edits.
  - The per-round verification counted as a formation in the phase counters. It is now untimed.
  - Two syntax tests follow the fixed point:
    - the early namespace read is now inlined at its call, which runs after the literal, so an escaping closure keeps the case;
    - the token's fields are read as their values, and no literal is left.
  - The NO3 entry moves with the inline limit.
- **Cases** against `p3-1`: every lane passes.

  | Lane | Script | Module |
  |---|---:|---:|
  | Brotli | −420 (33 artifacts) | −181 (30 artifacts) |
  | gzip | −345 | −183 |
  | raw | −619 | −275 |

  Per lane, 1–5 artifacts grow (at most +13 bytes together) and 21–30 shrink.
- **Ratchet:** passes against `p2-1`'s baseline.
  - Cases: Brotli 46,833 → 46,798, gzip 57,078 → 57,031, raw 61,784 → 61,718.
  - Apps: 849 → 843 Brotli, and 870 is the competitor.
  - Loss rows: 2,862 → 2,856.
- **Unpatched ports** against `p3-1`: all green.

  | Port | Raw | Gzip | Brotli |
  |---|---:|---:|---:|
  | katexlil | −7,486 | −450 | −467 |
  | motionlil | +828 | −141 | −119 |
  | zodlil | +1,048 | +141 | −19 |
  | markedlil | −215 | +68 | +59 |
  | micromarklil | +948 | +92 | +169 |
  | jquerylil, posthoglil | 0 | 0 | 0 |
  | **total** | −4,877 | −290 | −377 |

  The raw growths on zodlil, micromarklil and motionlil are the transitional inliner at its fixed point. A body that other rules shrank below the limit is now copied to every call. That is the duplicating case M9.1 turns into a choice. Brotli absorbs most of it.
- **CPU pairs** against `p3-1`:

  | Port | Factor | Judged |
  |---|---:|---:|
  | markedlil | ×1.20 | 13 → 13 |
  | zodlil | ×1.53 | 24 → 45 |
  | posthoglil | ×1.15 | 24 → 24 |
  | micromarklil | ×0.91 | 25 → 19 |
  | katexlil | ×0.88 | 118 → 90 |
  | jquerylil | ×1.10 | 42 → 38 |
  | motionlil | ×1.27 | 35 → 36 |

  - Each rule set takes one more round than it needs to change something (the round that confirms), which is the cost where the walk judges the same count.
  - zodlil's walk judges almost twice as many candidates.
  - katexlil's and micromarklil's walks judge fewer, and run faster.
  - M5.7's dirty-unit scheduling is the remedy the plan names: a confirming round re-runs only the rules whose inputs changed.
- **D1** is verified with this batch: its solver and view have no reader, and every run above includes them.

The fixed point is worth more than any hand order the chain had: every codec gains on the cases, and the fleet gains on all three. The owner's ruling of 2026-09-29 applies to the scattered growths (markedlil +59 and micromarklil +169 Brotli) and to the CPU. M5.3a is partly done:
- landed: the chain is gone, the scheduler runs both rule kinds, and every transitional rule names its deleting task;
- open: the program rules' structural order over SCCs with a dirty worklist, and classifying the remaining JS target rules as their legality moves to the columns (M5.2).

## 2026-09-29 Batch O1: the program's initialization order on the tree (M6.5 through M5.2's columns)

(Its commits are titled `B1`; it is O1 here, to keep step 8's batch B1 distinct.)

**Pre-registration** (written before the first build of the batch; base: J1's record).

Why: the tree's rules ask one initialization question: does a root binding hold its value whenever a function runs? Root constants and single-use forwarding into closures depend on the answer. `quiet.rs` answers it from the tree alone: a function may run from the first root statement that could run any program code. The program's initialization owner (M6.5) knows more:
- the root point that settles each module cell;
- the first point during which each unit may run, over the call graph.

It computes both on every build, and nothing on the tree reads them. The 2026-09 record measured the gap on the fleet: roughly −90 to −160 Brotli on zodlil (41 enum kinds, 229 reads), −124 on motionlil, −41 to −55 on posthoglil, and a spread on katexlil.

What the batch builds:
- **C1. Two columns on the module**, indexed by id and charged as retained output.
  - By binding: the point settling the module cell that a formed binding stores.
  - By function: the first point during which a formed function's unit may run.

  Formation writes them from the demand plan's initialization owner. A binding or function that a rule creates has neither.
- **C2. `Order::initialized_in` reads the program's answer first**: the cell settles before the function's first point. The tree's own answer stays as the fallback for what rules created.

  Forwarding root constants and single-use values into closures, the two consumers, get the stronger answer unchanged.
- **C3. Root rows carry the program's root point** of the last operation a statement was formed from. A fused row keeps the later point, and an unknown part makes it unknown. `runs_after_root` asks the program too: does the function first run after the statement completes? That is the namespace flattening's question.
- **C4. The tail runs declaration fusion** (`merge_declarations`, M8.2 A2's first rule). The tail's statement spellings build stores like `c=x?a:b`, and nothing fuses them with their `let` afterwards. `j1-1`'s katexlil keeps 38 adjacent `let c;c=E` pairs, such as `(a,b)=>{let c;c=a===void 0?b:a;return c}`; the 2026-09 splice measured 64 such sites at −104 Brotli.

Not in this batch:
- The deletion of `quiet.rs`'s order waits for the rule-created bindings and functions to carry a transfer.

Predicted:
- **Output:** smaller where functions declared early read root constants (zodlil first, then motionlil and posthoglil), and on katexlil from C4.
- **katexlil:** may grow where a forwarded string literal repeats (as recorded), within the ±0.5% fleet band.
- **CPU:** unchanged; two small copies per `order()`.
- **Tests:** a new syntax test forwards a constant into a function the call graph runs after it, and runs it.

**Landed** (binary `o1-1`, and a fix-up of the new test's source order; the checker refuses a function that reads a constant declared after it):
- **Unit tests:** 1,627 pass, including the new syntax test: a root constant reaches a function the call graph runs after it, and the function runs.
- **Cases** against `j1-1`: every lane passes; four artifacts change, all smaller.

  | Lane | Script | Module |
  |---|---:|---:|
  | Brotli | −5 | −17 |
  | gzip | −7 | −15 |
  | raw | −19 | −33 |

  The corpus seldom has early functions that read root constants, or statement-built stores.
- **Ratchet:** passes; the totals are those of J1.
- **Unpatched ports** against `j1-1`: all green.

  | Port | Raw | Gzip | Brotli |
  |---|---:|---:|---:|
  | katexlil | −14,343 | −5,601 | −5,026 |
  | micromarklil | −298 | −781 | −953 |
  | zodlil | −1,746 | −493 | −285 |
  | posthoglil | +184 | −93 | −121 |
  | jquerylil | +2,560 | +95 | +17 |
  | markedlil | −20 | −31 | +28 |
  | motionlil | −1,063 | −156 | +184 |
  | **total** | −14,726 | −7,060 | −6,156 |

  - katexlil's total is its ten builds of one bundle, each about −500 Brotli (the main file 60,959 → 60,434, −0.9%).
  - micromarklil's builds are about −145 each.
  - motionlil's growth is spread over 193 of its 622 parts, most of them under 600 bytes, where Brotli moves by tens of bytes for a few raw ones.
  - jquerylil's raw growth is string constants that now reach more functions. Every literal forwards, and the codec keeps the repeats.
- **CPU pairs** against `j1-1`:

  | Port | Factor | Judged |
  |---|---:|---:|
  | markedlil | ×0.99 | 13 → 13 |
  | zodlil | ×0.57 | 45 → 22 |
  | posthoglil | ×0.73 | 24 → 16 |
  | micromarklil | ×0.97 | 19 → 18 |
  | katexlil | ×0.72 | 90 → 57 |
  | jquerylil | ×0.93 | 38 → 29 |
  | motionlil | ×1.01 | 36 → 36 |

  The walks judge fewer candidates on smaller trees. This undoes most of J1's slowdown.

The program's order is worth far more than the prediction: the fleet is −6,156 Brotli, against about −300 predicted. Most of it is katexlil, where both halves act on the same functions:
- a root constant forwarded into an early function;
- a statement-built store fused with its declaration and then forwarded.

M6.5's first consumer is now the tree: `settled` and `first_run` reach the rules through formation, so the initialization owner's third answer ("per access") is the only one the tree does not read yet. What remains open:
- `quiet.rs`'s own order stays as the fallback for what rules create;
- deleting it waits for transfers on rule-created bindings and functions (M5.2).

## 2026-09-29 Batch A2a: A2's small rules (the dead `let`, C9, C19)

(Its commits are titled `B2`; it is A2a here, to keep step 8's batch B2 distinct.)

**Pre-registration** (written before the first build of the batch; base: O1's record).

Why: batch A2 lists these as JS target rules on the tree's columns: `let x; x = E` fusion, a dead `let` whose initializer is discardable, C9 (known pure constructions) and C19 (`globalThis.<builtin>`). O1 brought the fusion to the tail; this batch takes the other three. `j1-1`'s outputs:
- zodlil reads `globalThis.RegExp` and constructs `new globalThis.RegExp("…")` with a literal pattern;
- the tree keeps a declaration no code reads when its value has effects (`let x=f()`) whole;
- a regular-expression literal counts as an effect.

What the batch builds:
- **C1. A dead `let` keeps only its effects.** `let x=E`, where no code references `x`, becomes `E;` when `E` is not inert. An inert value still goes whole. A class or function value keeps its declaration, since the binding may name it. The rule runs under the dead-code permission, like the drop it extends.
- **C2. A regular-expression literal is inert** (C9): it creates a fresh object and cannot throw, because its pattern was validated when it became a literal. An unread `new RegExp("…")` of literals, which `simplify_operators` already spells as a literal under unpatched builtins, then goes as dead code.
- **C3. `globalThis.X` is `X`** for an ECMAScript builtin `X` under unpatched builtins (C19, R10). `globalThis` may be the host's name, or a declared extern of that spelling that is neither an import nor assigned. Naming never gives a binding a host name the tree reads. A constructed literal pattern then becomes a literal.

Predicted:
- **Output:** smaller by a few bytes per site: zodlil first; other ports only where unread declarations with effects remain.
- **CPU:** unchanged.
- **Tests:** two JS-tree tests (an unreferenced declaration, the global object's builtin).

---

## Appendix: where milestones 001–014 went

| Old | Now |
|---|---|
| 001–007 | Foundations of the compiler; receipts refreshed in M12.5 |
| 008 | M3.3, M8 |
| 009 | M7, M9 |
| 010 | M5.4, M9 |
| 011 | M1, M3 |
| 012 | M3.5 (the budget model, with M3.6 and M9.4 merged into it), M9.10, and the compile-time gate (rule 3) |
| 013 and 013-T1..T7 | M4–M9 and M12. T7.1 → M6.2/M7.2; T7.2 → M9.3; T7.3 → M5.3; T7.4 → M6.5/M7.4; T7.5 → M7.3; T7.6 → M7.5; T7.7 → M6.6/M7.9/M9.7; T7.8 → M4.1/M6.7; T7.9 → M7.7; T7.10 → M7.8/M8.3; T7.11 → M6.4; T7.12 → M7.6; T7.13 → M9.6/M9.7; T7.14 → M9.9 |
| 014 | M1 (retirement), M12 (certification) |
