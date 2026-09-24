# Migration plan: one compiler

Revision 2026-09-23 (second revision, after an adversarial review of the first). **This is the only plan.** It does two things:
1. It takes the codebase from two compilers in one binary to the single compiler described in [future-architecture.md](../future-architecture.md).
2. It carries that compiler to winning every maintained library under every objective, in both the open and the closed world.

History and records:
- Milestones 001–013 (receipts, batch ledger, measurements, the Closure ADVANCED inventory 013-T7) are in [record-2026-09.md](record-2026-09.md).
- Owner briefs are in `finer/intent/`; today's is [2026-09-23](../../finer/intent/2026-09-23.md).
- The eleven area reports behind this plan are in `~/lilscript-work/out/arch/`.

---

## Where we are (2026-09-23)

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

## Rules for every phase

1. **One compiler.** Nothing new may depend on the old route. Its code is prior art: read it, never link or port it line by line.
   - The frozen reference binary is `~/lilscript-work/bin/reference-2026-09-23/lilscript`: binary b80, SHA-256 `df8595…`. It was built at 21:00 on 2026-09-23 from the pre-flip working tree of `d362338f`, so its default route is the old one. It is used only to measure "the first bar".
   - The pre-migration binary of the one compiler is `~/lilscript-work/bin/pre-m1/lilscript`, built from `0c17237e`.
2. **By design.** Every change is generic and owned, and a fact is computed once, by its owner.
   - A fact reaches its consumers through its publication channel (program tables, or tree annotations after M5). **The re-derivation it replaces is deleted in the same batch its consumers switch.**
   - Formation never emits a shape a later pass undoes.
   - No thresholds tuned on three ports: a choice goes to the codec, a structural bound comes from policy.
3. **Gates.**

   | Gate | Rule |
   |---|---|
   | Correctness | Unit tests, the case runner and the reference port suites. Byte identity is evidence, not a gate |
   | Size | No Brotli degradation at batch end on the case runner's corpora and the reference ports; no per-library loss at phase end |
   | Exact rules (removing operations) | Must not raise any port's Brotli. Until monotone selection exists (M5.4), judged on the formation-only lane plus a codec check of the final artifact |
   | Codec-dependent shapes | Ship as choices or terminal challengers |
   | Compile time | Level-13 wall time per reference port must not regress at phase end against the pre-M1 binary on this host (alternating pairs, one session). Runtime is a reported lane, not a gate (architecture §17) |

4. **Pass rule (D4).**
   - At or below the bar is the floor ("tie"). A **win** is at least `max(100 bytes, 1%)` below it.
   - Each library has a fixed bar list: its pinned comparable competitor builds under the 013-T5 comparability rules.
   - Both the open world (the developer-facing API preserved, mangling fairness contract) and the closed world must pass.
5. **Batches.** 4–8 changes per build, one verification pass per batch and one ledger row per batch. Builds and tests run on this host only.
6. **Evidence.** A result counts only when the delivered file is compiler-written, with no post-minifier. The receipt pins the binary, source, patch and dependency identities.
7. **Prior art.** Each batch's ledger row cites the competitor source read (`repo@commit file:line`, from `~/competitors`) and the old-route prior art read (owner, 2026-09-01).
8. **Verification ladder.** The fleet runs only at phase end: the owner's "don't build it all" (09-04) holds per change, and "no accepted losses" (09-22) holds per phase.

   | When | What runs |
   |---|---|
   | Per change | Unit tests, the case runner, probelil, the micro gates |
   | Per batch | The reference port suites (katexlil, markedlil, zodlil, jquerylil, posthoglil, motionlil, micromarklil and its family), in the background, not blocking the next change |
   | Per phase | The fleet |

   A change worth under about 400 fleet bytes is judged on micro gates or by the terminal slot, never by a fleet A/B.

---

## Phases

| Phase | Name | Depends on | State |
|---|---|---|---|
| M0 | Record and freeze | — | done 2026-09-23 |
| M1 | One compiler: the old route leaves the product | M0 | active |
| M2 | Verification ladder, baseline and interim release | M1.3 (runs alongside M1) | ready |
| M3 | Honest configuration, one public API, delivery contract | M1 | waiting |
| M4 | Checker identities and checker-owned facts | M1 | waiting |
| M5 | The machinery: edit kernel, annotations, scheduler, monotone selection | M1, M4.1 | waiting |
| M6 | The fact spine | M4, M5 | waiting |
| M7 | Program rules | M6 | waiting |
| M8 | Canonical formation and the pure printer | M5, M6 | waiting |
| M9 | Choices, naming, layouts and data | M5, M7, M8 | waiting |
| M10 | Language for size | M4 (runs alongside M6–M9) | waiting |
| M11 | Native and cross-target | M6, M7 | waiting |
| M12 | Qualification and publication | continuous; closes last | waiting |

### M0 Record and freeze: done 2026-09-23

- Batches 29–32 and the default flip committed (`d362338f`).
- The owner briefs recorded verbatim.
- The architecture written; `docs/compiler-design.md` became a pointer to it.
- The plan rewritten; the history moved to `record-2026-09.md`.
- The reference binary frozen with its SHA-256; the pre-M1 binary preserved.
- Migration work happens on branch `one-compiler`, in worktrees under `~/lilscript-work/wt/`, so parallel sessions on the main tree are not disturbed.

### M1 One compiler: the old route leaves the product

**Goal.** No route switch, no old-route code, no old-route tests. `--backend`, `[compiler] backend` and `CompilerBackend` are gone, and every tool compiles through the one compiler.

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
| M1.9 Correctness debts found by the harvest | Fixed as bugs (D3), with the harvested cases as their reproductions:<ul><li>`async int f(){return 1;}` is folded to `1`, breaking `.then`;</li><li>a field default (`new Map`) is evaluated before a constructor argument (D3.4 order);</li><li>`JsValue == 0` is lowered to `===`, while the language spec says dynamic equality coerces. The fix follows the spec until M10.9 changes it by owner ruling;</li><li>struct values passed to an `extern` are refused;</li><li>a wrapper's inferred name disagrees with the spec, in one of two sibling cases</li></ul> |

**What M1 does not restore.** The old route had these; each is owned.

| Capability | State after M1 | Owner |
|---|---|---|
| The `pure` contract check (a diagnostic) | Absent (already absent on the default route) | M6.3 |
| Removal of discarded pure calls | Absent | M7.2 |
| Native `Record<T>`/JSON, and the user-facing C extern ABI | Refused; the `scripts/verify.sh` extern-ABI step and the differential's native `Record` lane become ledgered expected failures | M11.3, M11.4 |
| Native stack and region storage | Absent | M11.5 |
| Name-keyed host helpers: extern names given built-in JS bodies, such as jQuery's `isWindowValue`, lil-solidjs's `DOM_RECONCILE`, `objectHasOwn` and `mathMax` | **Dropped by design.** Ports declare host modules or use `JS.*` and the catalog (M10.2); jquerylil, motionlil, monacolil and lil-solidjs are checked in M1.8 | — |
| Same-named private classes in two modules; `export constructor` | Refused | M4.1 |
| Unrolling of `inline for`; `@pool` | Ignored | M10.11 |
| Record spread construction | Refused | M10 decision (M10.8) |
| `public_aggregate_abi = "positional"`, `function_scope`, `idiom_directed_naming`, `[mangle] properties`, profile-guided optimization | Refused or no effect (warned) | Positional: refused (D2). Module wrapper: M3.1 `format`. Naming: M9.5. Typed property renaming: M9.6 |
| preserve-modules chunks and lazy `import()` chunks | Already broken on the one compiler (reproduced: `chunks: []`) | M3.3 |
| Source maps | Not supported (the parallel `codex/source-maps` branches are built on the old route) | M8.6 |

**Exit, checked by script.**
- A grep over `src/`, the CLI help, configuration and current docs finds no route-selecting `backend`, and no "legacy route" or "semantic route" outside history files.
- Every binary (`lilscript`, `-lsp`, `-lint`, `-fmt`, `-codec`, `-differential`, `-playground`, `lilpack`) builds and uses the one compiler.
- `cargo test` passes on all targets.
- M1.8's corpus is byte-identical to the pre-M1 binary, or every difference is explained.
- The expected-failure ledger lists every red gate with its owner.

### M2 Verification ladder, baseline and interim release

| Task | Content |
|---|---|
| M2.1 Green CI | <ul><li>Fix `cargo fmt` and `examples/semantic-integrated.rs`.</li><li>One Linux job under about 15 minutes, with explicit steps: fmt, `cargo test --lib`, the case runner's production lanes, the codec contract, and the micro gates as a *reported* step until M7.</li><li>Publishing steps (web catalog, VS Code packaging, Playwright, Closure) leave the gating path.</li><li>probelil is vendored into the repository, or run from a pinned copy.</li><li>Competitor artifacts for the micro gates are cached and committed.</li><li>Steps owned by later phases (extern ABI, differential native lane) stay out until their owners land</li></ul> |
| M2.2 Case runner | <ul><li>One runner over `tests/cases`, the harvested regressions and the D3 clause cases, moved out of `d3_clause_tests.rs` so they run on shipped output.</li><li>Lanes: `{formation-only, production} × {brotli, gzip, raw} × {script, module, C}`, with per-target feature masks from day one.</li><li>The harvest's case conventions: `.host.js` preludes, `.module-probe.mjs`, multi-module folders and merged `.toml` keys.</li><li>Family-veto lanes are added after M3.2.</li><li>It replaces the development-mode census and the knob configs in `tests/config/`</li></ul> |
| M2.3 Oracles | Interpreter-generated `.out` where the interpreter covers the program. Coverage is measured by running it, not estimated. Blessing is refused when the interpreter disagrees. `print` is never stripped (decision owned here; M10.11 adds the `debug` class) |
| M2.4 Interpreter extension | The reference interpreter gains structs, classes, enums, generics, Map/Set and a declared host model, feature by feature. Each feature lands before the M6/M7/M9 work that optimizes it. It stays independent of formation |
| M2.5 Admission parse | Every delivered JavaScript file is re-parsed by Oxc inside admission; its structural digest must match the printed tree (A5) |
| M2.6 Port runner and ledger | <ul><li>One versioned runner that records failing-test sets, diffs them against the **expected-failure ledger** (every entry has an owner task) and pins the compiler by digest.</li><li>It replaces `portgate.mjs`, `semantic-port-tests.mjs` and the unversioned `~/lilscript-work/tools` scripts.</li><li>It is checked into the repository</li></ul> |
| M2.7 Differential | The generator becomes type-directed with per-target masks. Its hand-pinned prologue shapes move to `tests/cases`. It enters the gating job only after this |
| M2.8 Baseline on one binary | <ul><li>Every maintained port's rewrite is committed to its own repository (M12.1 brought forward). The in-flight motion and zod rewrites in `~/lilscript-work/portwork/` land there too.</li><li>The fleet is built and its suites run on the post-M1 binary.</li><li>The scoreboard is frozen as the baseline that phase gates compare against, open and closed world</li></ul> |
| M2.9 Interim release | The owner's 2026-09-23 release request. Every port is rebuilt by the pinned post-M1 binary with no post-minifier; its Pages site is updated with sizes and compile times (`site/results.json`, `npm run check:site`); one report compares against the last release and against Terser, Oxc/Rolldown and esbuild. Ports that lose are published as losses, not hidden |

**Exit.**
- CI is green on the one compiler.
- The case runner, the micro gates and the port runner run each batch.
- The baseline scoreboard exists and the interim release is published.

### M3 Honest configuration, one public API, delivery contract

| Task | Content |
|---|---|
| M3.1 Schema v3 | The axes of architecture §14: contract (`[target.javascript]` with independent `execution`, `world` and `format`, where `format` replaces `function_scope`), objective, effort, resources, execution, generated families. A translator maps every old key to a new key, a warning or a refusal |
| M3.2 Family registry | Every program rule, JS rule and choice registers `{id, mandatory or optional, legality, risk}`. `TargetCompaction` splits into its real families. The five tactics without a producer are removed. The receipt lists the families that actually ran |
| M3.3 Delivery contract | preserve-modules keeps every source module a file; lazy `import()` gets its chunk. Deploy cost uses the objective's codec only. `verify-bundles.mjs` is split into contract assertions and plan assertions, and its fixtures stop depending on `strip_console` |
| M3.4 Public API | `build::{check, build, with_session}` with a typed `BuildReceipt` and `Delivered { files, manifest, sizes }`. The multi-objective CLI (`--objective raw,gzip,brotli`) gives one winner per objective |
| M3.5 Budgets in policy | Search budgets enter the policy, receipt and fingerprint. `compiler_service::search_request`'s hard caps and `LILSCRIPT_SEMANTIC_WORK` go. Level calibration waits for M9.10 |
| M3.6 Codec pool | Bounded threads for render and codec work, with deterministic batch order |
| M3.7 Environment variables | Only diagnostic variables remain |

**Exit.**
- No accepted key is silently inert.
- `--print-policy` equals the build's request.
- The bundle contract cases pass.
- A thread-count change never changes output bytes.

### M4 Checker identities and checker-owned facts

These are prerequisites for field identity, shapes and every fact the checker already proves.

| Task | Content |
|---|---|
| M4.1 Nominal identity | <ul><li>`NominalId` for classes, enums and extern classes, with per-module scopes; the 16 port class renames are reverted.</li><li>One module-graph checker entry with explicit phase products; `export constructor` in module mode.</li><li>Class fields become `FieldRef{nominal, slot}` places in the IR, and allocations carry their nominal.</li><li>`ClassDefinition`, native and formation look classes up by id, not by name</li></ul> |
| M4.2 The dynamic type | `Type::Dynamic` replaces `TypeParameter("$js")` at every site; type parameters by id; interned types without source lifetimes |
| M4.3 Checker facts transported | <ul><li>`ResolvedOperator` recorded by the checker; elaboration stops re-deriving `IntBinary` from result types.</li><li>`assigned` split into `reassigned` and `observable_before_initialization`, plus a per-occurrence `ReadInitialization`, which seeds M6.5.</li><li>Parameter defaults on declarations, not in function types.</li><li>Declaration attributes (`pure`, `debug`); ambient `this`/`arguments` as checker-resolved bindings</li></ul> |
| M4.4 Node ids | Ids on identifiers, declarations and statements; the span-keyed fact maps are deleted |
| M4.5 Contracts at check time | Frame (D3.9) and boundary (D2) refusals are diagnosed in the check phase, with source spans, against the resolved contract |
| M4.6 Operation catalog | `BuiltinCall` and `Intrinsic` merge into one declarative catalog: signature, defaults, effect class, fold, JS and C spelling, target capability. The checker diagnoses non-portable use against the requested targets. The "never rename" host surface is derived from `extern` declarations |

**Exit.**
- Tests show that two modules' private `class Node` compile, `export constructor` works, and no `"$js"` string test remains.
- A grep finds no name-keyed class lookup.
- The checker's diagnostics carry spans for every refusal.

### M5 The machinery

What every later phase needs: edits, the carriers that take facts to the tree, the scheduler, and selection that cannot regress.

| Task | Content |
|---|---|
| M5.1 Program edit kernel | Structural `EditBatch` on the existing transaction (insert, remove, splice, clone, delete unit, change signature, retype allocation); `UseIndex` updates incrementally, checked against a full rebuild. DCE, demand's liveness applied as an edit, is its first production rule, and native gets it |
| M5.2 Tree annotations and journal | <ul><li>Annotation columns on the JS tree (value domain, `FieldRef`, `AllocSite`, `UnitId` and function facts, callee, initialization order, `GlobalId`, observation, spelling), renumbered by the arena itself.</li><li>`literal_alternatives`, `binding_classes` and `defined_parameters` move onto nodes.</li><li>Typed mutation helpers journal every edit; a debug build checks the journal against the actual difference</li></ul> |
| M5.3 Scheduler | One scheduler for program rules and JS rules. It starts in **fixed-order mode**: today's chain, encoded as data (byte-identical), with the verifier after each rule set in debug builds. **Fixpoint mode** then runs as a terminal challenger against it, and replaces it once no port regresses. The hand-written chain in `javascript.rs:663-966` is deleted at that point |
| M5.4 Monotone selection and the terminal slot | Incumbents never worsen; lower-effort incumbents are replayed. A terminal challenger stage offers choice assignments on the final artifact under the requested codec. The mechanism lands before any new family |
| M5.5 Dataflow and views | The call graph with SCCs (generalized from `CallableInputs`); one region-structured dataflow solver; the cell-SSA view |
| M5.6 Resource accounting | Work budgets per phase and per rule; exact byte accounting only for retained candidate and artifact storage (architecture §17) |

**Exit.**
- Fixed-order scheduling is byte-identical to the chain it replaces.
- The terminal slot runs on every build.
- The edit kernel carries DCE for both targets.

### M6 The fact spine

Every fact is taken through the same four steps in one batch: **compute → publish → switch consumers → delete the re-derivation.**

| Task | Fact | Deleted when its consumers switch | First reproductions |
|---|---|---|---|
| M6.1 Call graph and function facts | Complete call sets, value calls resolved, address-taken, name and length observability | The call-only scans in `inline.rs`; the name-observability predicates in `javascript.rs:1265-1537` | — |
| M6.2 Effects | Per-operation and per-unit summaries, seeded by declared `pure` and trusted `pure extern`. Termination needs a proof (D3.6; architecture §7) | `facts.rs`'s unknown calls, `demand.rs`'s private model, `helper_family`'s composition, `quiet.rs`, `inline.rs:inert`/`runs_no_user_code`, `mod.rs:inert_value` | `scratch-program/ip/pure.lil`; motionlil's `warning`/`invariant` |
| M6.3 The `pure` contract | A check-phase diagnostic from the effect engine | — | The old route's diagnostic, verbatim |
| M6.4 Values | One lattice: exact, finite set, int32 range, primitive class. Held per value, formal, result and `(nominal, slot)`. `binding_classes` trusts declared types only for private units, or under the `typed_arguments` assumption; this fixes an unsound source today | `javascript_int32.rs`'s proofs, `NumberFacts` sources, the `binding_classes` derivation, `simplify::known`, `scalar_transfer.rs` | `scratch-target/a.lil` (`9+40\|0`); probe `f1`; a D2/D3.3 case with an ill-typed JS caller |
| M6.5 Initialization order | Settled root bindings; "not invoked before root statement S", seeded by M4.3 | `quiet.rs`'s order and factory shapes, `root_constants.rs`'s own proof, the duplication in `demand.rs:initialized` | zodlil's 41 enum constants; `scratch-target/b.lil` |
| M6.6 Escape | Per allocation site: local, typed or host. A compare with `null` is not an escape | `scalar_objects.rs`'s syntactic test; lint's `aggregate-escape` returns | snippet `s17`; probe `f1`'s per-iteration array |
| M6.7 Field facts | Per `(nominal, slot)`: read, written-value join, host-reachable, reflective, exported shape | Name-keyed field logic | probes `p2`, `p5`; motionlil's dead fields |

- M4.6: the lint rule `performance/aggregate-escape` (removed in M1.2) returns with the escape fact.
- Lint IR rules see the program before program rules; they move after the M5 rules when those land.

**Exit.**
- An owner table shows one owner per fact.
- A grep finds none of the deleted functions.
- Every reproduction is committed to `tests/cases/regressions`.

### M7 Program rules

These are exact, operation-removing and target-neutral. Where a rule's value depends on printed length, it is not a rule; it becomes a choice (M9).

| Task | Content |
|---|---|
| M7.1 Removal | Dead values, units and cells; unused `let`s whose initializer is removable; dead stores; common subexpressions and algebraic identities, where they remove operations |
| M7.2 Discarded effect-free calls | Needs M6.2 and the termination proof |
| M7.3 Parameters and returns | `Dropped` and `Constant` transports on complete call sets; unused results. Negative cases: effectful arguments stay in order; `arguments`, rest, exported and host-visible `length` |
| M7.4 Root constants, defines, flow-sensitive forwarding | Settled constants of at most a few tokens are substituted; longer literals and aliases are choices. Build-time `define`s. Reaching-definition forwarding (katexlil's 41 `x=E;return x` sites) |
| M7.5 Inlining | <ul><li>Direct and block inlining with the splice edit; structural bounds at program level, and codec-dependent cases as choices.</li><li>Runtime adapters, known-closure calls, constant-capture cloning, constructor chains.</li><li>Deleted: `helper_family`, `forwarding_builtin`, `undefined_call`, the semantic parts of the JS tree inliners, and the IIFEs from `place_single_calls`</li></ul> |
| M7.6 Namespaces and emulated methods | Constant namespaces collapse, and single-definition methods of compiler-owned objects are devirtualized, on allocation identity and initialization order |
| M7.7 Fields | Dead fields, constant fields, overwritten stores |
| M7.8 Folding | Constant and branch folding; path-sensitive constants; string-literal sums; array store collection (`let e=[];e.push(…)`); `\|0` elision from ranges |
| M7.9 Scalar replacement | Of classes, control-flow aggregates and loop-carried structs, on escape |

**Exit.**
- `comparison/cases`: no case loses to the reference binary's old route, and none loses to the smallest competitor without a ledgered owner.
- `comparison/algorithms` and the census totals are at or below the old route.
- Every M6 reproduction is fixed.

### M8 Canonical formation and the pure printer

| Task | Content |
|---|---|
| M8.1 Formation writes annotations | `FieldRef`, `AllocSite`, `UnitId`, callee, domain, initialization order, `GlobalId` |
| M8.2 Canonical forms | <ul><li>`JS.call` and `JS.methodN` are formed as method calls; constructions by the layout choice; defaults by the transport.</li><li>Deleted: `self_method_calls` (except for user-written `JS.call`), `dissolve_receiver_adapters`, `array_receiver_calls`, `inline_initializers`, `drop_redundant_init_stores`, `drop_default_arguments`, `native_default_lengths`, `drop_typed_default_checks`</li></ul> |
| M8.3 Pure printer | Structure rewrites leave the printer (`return c?a:b` consuming the next statement, logical statements, loop heads) and become spelling attributes. `\|0` and `++` come from facts |
| M8.4 Host modules | A typed Oxc visitor produces host units; the ESTree JSON walk is deleted |
| M8.5 Runtime helpers | The table decoder, reference helpers and adapters are written as LilScript prelude code, compiled through the pipeline and demand-pruned |
| M8.6 Source maps | Re-founded on tree origins as a delivery-plan feature. The old-route source-map branches are prior art |
| M8.7 Port-shaped rules | Every rule in the inventory below is either given a generic legality condition, turned into a choice, or deleted. Every contract assumption a port sets carries a recorded reason |

**Exit.**
- No pass recovers a shape formation emitted.
- No side table is remapped by hand.
- The printer holds no semantics.
- The port-shaped inventory is empty.

### M9 Choices, naming, layouts and data

| Task | Content |
|---|---|
| M9.1 Choice interface | `Choice` + `ChoiceMap`. The product, record and string families and function layout become implementations; `implementations.rs`'s per-family vectors are deleted |
| M9.2 Objective as a judge | `raw_structure` and `raw_spelling` become per-family seeds, and every family is available to every objective |
| M9.3 Spelling families | 013-T7.2's subset (measured −462 Brotli over six ports) through the terminal slot; `loop_head_declarations` and `logical_statements` become choices |
| M9.4 Estimator and finalists | A per-objective size estimator ranks; Brotli-11 runs only on finalists |
| M9.5 Naming | One allocator. Slot scheme, assignment order and alphabet are codec-judged seeds (architecture §10: Closure's scheme measured +86 to +1,171). The −353 walk-order headroom is re-measured on current outputs first |
| M9.6 Property names | Rename and ambiguate private `FieldRef`s (markedlil −252 measured), reporting the open and closed lanes |
| M9.7 Layouts | Per nominal or allocation: scalars, positional, named or class; struct parameters as fields; `JS.assume` view or decode |
| M9.8 Data | String tables as a choice with an estimator (no 64 / 0.85 thresholds); numeric and columnar tables; pooling at naming time |
| M9.9 Function folding | Identical, permuted and one-constant functions, as a choice; outlining of repeated regions as a choice |
| M9.10 Effort calibration | Each level's schedule is calibrated on real alternatives. Level 15 is at most level 13 on every reference port, strictly smaller in total beyond the noise floor, and inside its declared time budget |

**Exit.**
- No objective-conditioned rule remains.
- The effort levels meet M9.10's rule.

### M10 Language for size

The language law (architecture §12, L13): a typed form never costs more than its untyped equivalent. Each item lands with at least one port rewritten to use it, its suites green and no Brotli degradation.

| Task | Content | Evidence |
|---|---|---|
| M10.1 L1 declared object shapes | `data`/`accessor` fields, optional fields, construction literal, nesting at boundaries. micromark's 11 views migrate first. `assume_pure_property_reads`, `preserve_properties` and `internal_properties` retire for declared values | −6,359 Brotli on the markdown stack as a global flag |
| M10.2 L8 dynamic type | Member, call, `new` and operator syntax; the 63 `JS.*` builtins collapse into it and a typed host catalog. Host helpers that the old route matched by name get declared bindings | Deletes recovery glue |
| M10.3 L2 const data and tables | Deep-immutable `const` data; exported const objects; hex, exponent and leading-dot literals; bounded `const` evaluation; katex's font metrics move into LilScript | katex −2,529, micromark −1,054 |
| M10.4 L3 receivers, constructibility, rest | `fn(this: T, …)`, `fn` against `function`, methods in literals, `T... rest`. `JS.methodN` and `extern JsValue this/arguments` retire, and `assume_unconstructed_callbacks` becomes a type fact | katex −240 |
| M10.5 L4 sealed virtuals, interfaces, sum types | A static call, tag switch or prototype method per call site | motion ≈ −700 |
| M10.6 L5 ABI-valued enums | `enum T: string`, explicit values, `ordinal`/`from`, flag sets | micromark's 104 string types; zod's 41 int kinds |
| M10.7 L9 casts and operators | `as?`, unsafe views, non-null assertion, float `%`, `is` on classes and shapes | motion's identity-cast externs |
| M10.8 Record spread and records | Implement spread or refuse it with a diagnostic; decide `Record<T>`'s default prototype | `comparison/cases/collections/record-json` |
| M10.9 L6 and L7, **owner rulings first** | Immutable value structs with functional update (revises D1); nullish `T?` on JS; a non-overflowing index/count type; `charCodeAt`; dynamic equality (strict against literals, explicit `looseEquals`) | `ref` used 0 times; the lens runtime; `??null` and `\|0` costs |
| M10.10 `object` singletons | Deleted in favour of module namespaces and const records (0 uses), unless the owner keeps them | — |
| M10.11 L10 pins and defines | `inline for`, `@pool` and region-scoped policy as pinned choices; `define` build constants; a `debug` effect class | Owner, 2026-09-04 (regions) |

**Exit.**
- Each L-item has a port using it.
- The census of `JsValue` and `JS.*` per reference port falls from the 2026-09-23 counts, with no Brotli loss.

### M11 Native and cross-target

| Task | Content |
|---|---|
| M11.1 Toolchain owner | One owner for compiler discovery, flags, strictness and sanitizer profiles. Native diagnostics carry spans and render as text |
| M11.2 Plain arithmetic | Remove the per-operation `volatile` (4.5× on float loops), keeping the ABI guards |
| M11.3 Externs per target | `extern` binds a JS host name or a C link name; the header is generated. The `verify.sh` extern-ABI gate passes again |
| M11.4 Portable records | `Record<T>`, `Object.keys/values/assign` and JSON natively; the differential's native `Record` lane is unmasked |
| M11.5 Shared facts | Native consumes liveness, initialization order (no runtime guards), escape (stack storage, refcount elision), effects and specialization |
| M11.6 Portable subset | Exceptions (status propagation driven by effects), a string ABI that reclaims memory, regex, generators/async |
| M11.7 Runtime and symbols | Runtime helpers as C files with declared dependencies and a standalone `-Werror`/sanitizer build; one native symbol allocator |
| M11.8 Objective | A native objective (speed, size or balanced) and a cost measure; a C library ABI (exports plus header) |
| M11.9 Profiles | Cross triples and wasm32-wasi as toolchain profiles; the case runner runs one cross triple |

**Exit.**
- One maintained library's portable core compiles to C and passes JS == C.
- The extern-ABI gate is green.
- Every corpus case runs on one cross triple.

### M12 Qualification and publication

| Task | Content |
|---|---|
| M12.1 Ports own their sources | (Done in M2.8.) `finer/port-migrations/` is retired. Sibling-line knobs leave port configs |
| M12.2 No post-minifiers | Compiler-written files for every export condition (ESM, CJS, browser). This replaces micromarklil's "smaller of compiler or esbuild", motionlil's esbuild+Terser `full.js` and zodlil's esbuild re-bundle |
| M12.3 Every library wins | Per library, per objective, open and closed world, under the pass rule (rule 4). Work list: motionlil, zodlil, then the ports without a standing. SWC and Closure lanes are pinned where comparable |
| M12.4 Rebuild and publish | Every port's `dist/` and Pages site is rebuilt by the pinned compiler with compile times; one report against the last release and the competitors |
| M12.5 Receipts | Refreshed on the final binary; separate conclusions for architecture, correctness, cost and size |

**Exit.** The scoreboard has no losing cell.

---

## Old-route optimizations: disposition

Every pass of the old optimizer chain (`optimizer.rs:243-430`, `compress_passes.rs`) and its emitter planning, with what replaces it. "Measure" means: measure the old route's value with the reference binary before deciding.

| Old-route pass | Disposition |
|---|---|
| `promote_locals_to_ssa` (mem2reg) | Replaced by the cell-SSA view (M5.5) |
| `optimize_scalar_fixed_point`: `fold_and_propagate_control_flow`, `simplify_algebraic_expressions`, `eliminate_redundant_phis` | M7.8 folding, M7.1 algebraic identities that remove operations; phis have no counterpart (regions) |
| `eliminate_common_subexpressions` | M7.1, where it removes operations; otherwise a choice (repetition is load-bearing) |
| `fold_owned_plain_object_reads` | M7.7 / M9.7 on field facts |
| `elide_single_use_stringify` | M7.8 on values |
| `propagate_single_assignment_globals`, `forward_single_assignment_global_aliases`, `internalize_entry_globals`, `eliminate_unread_globals` | M7.4, M7.1 |
| `devirtualize_methods` | Free by construction |
| `devirtualize_known_closure_calls`, `clone_constant_capture_signatures` | M7.5 |
| `specialize_constant_parameters` | M7.3 |
| `specialize_profiled_call_sites` | Dropped: profile-guided optimization is removed (architecture §17) |
| `optimize_unused_parameters`, `optimize_unused_returns` | M7.3 |
| `validate_declared_purity` | M6.3 |
| `optimize_inlining_fixed_point` (`inline_small_functions`, `inline_single_use_control_flow_function`, `eliminate_dead_functions`) | M7.5, M7.1 |
| `subsume_private_functions`, `merge_permuted_private_functions`, `merge_single_operand_private_functions`, `fold_identical_private_functions` | M9.9 (choices) |
| `analyze_escapes` | M6.6 |
| `scalar_replace_linear_classes`, `scalar_replace_control_flow_aggregates`, `scalar_replace_loop_carried_structs` | M7.9 |
| `eliminate_overwritten_field_stores` | M7.7 |
| `propagate_path_sensitive_constants` (SCCP) | M7.8 |
| `superoptimize_pure_expressions` | Dropped unless measured: it was off by default |
| `sink_partial_escape_allocations` | M11.5 (native storage); JS: dropped, it was off by default |
| `fuse_array_pipelines` | Dropped: 0 occurrences of `.map(…).map(` in the six ports; revisit with evidence |
| `outline_repeated_regions` | M9.9 as a choice (it wins raw and loses Brotli) |
| `collapse_single_use_byte_array_buffers` | Measure; else dropped |
| `call_array_methods_directly` | M8.2: formation emits receiver calls |
| `normalize_ambient_regex_constructions` | M4.6 catalog (regex literals are a spelling choice, M9.3) |
| `strip_console_output`, `lower_known_js_host_calls` (name-keyed) | The `debug` effect class (M10.11); name-keyed host helpers are dropped by design |
| Emitter planning (`IrJsEmitter::prepare`: 21 passes) and the 48 scored emission families | Each family is checked for whether it ever changed a winner on the last old-route fleet run (`--explain json` on the reference binary). Families that did become M9 choices; the rest are dropped with the measurement recorded |
| The text peephole (`js_peephole`, about 190 folds) | Dropped. Its insights (structurally identical functions spell identically, same-length names in source order) become naming seeds (M9.5) |

## Port-shaped rules and assumptions: inventory (owned by M8.7)

| Rule or setting | Where | Disposition |
|---|---|---|
| `self_method_calls`, `dissolve_receiver_adapters`, `array_receiver_calls` | `structured_js/calls.rs`, `typed.rs` | Deleted in M8.2 (formation) and M10.2 (dynamic type) |
| `group_prototype_stores` ("only katexlil declares both assumptions") | `declarations.rs:77` | Generic legality stated or deleted |
| Per-site namespace flattening for katex's `let _c;…;_c=$c` | `inline.rs` | Replaced by M7.6 |
| `fold_logical_assignments` / `fold_logical_returns` (transliteration temporaries) | `statements.rs` | Kept as canonical if a generic legality holds; otherwise a choice |
| Nullish narrowing for a port's `isNull` | `simplify.rs` | Replaced by value facts (M6.4) |
| The `undef()` helper-call fold | 008 batch 5 | Replaced by M7.5 inlining |
| Pinned foreign import spelling "for katex's build" | `javascript.rs:2540-2542` | Import identity by `(source, imported)` (M8.1) |
| `loop_head_declarations` (two contradictory measurements) | `javascript.rs:504`, `mod.rs:1043` | Choice (M9.3) |
| `assume_unconstructed_callbacks` (added to patch an unsound rule) | contract | Type fact (M10.4) |
| Every port's `assume_*` settings | port configs | Each gets a recorded reason, or is removed (M12.1) |

---

## Where the old milestones went

| Old | Now |
|---|---|
| 001–007 | Foundations of the compiler; receipts refreshed in M12.5 |
| 008 | M3.3, M8 |
| 009 | M7, M9 |
| 010 | M5.4, M9 |
| 011 | M1, M3 |
| 012 | M3.5, M3.6, M9.10, and the compile-time gate (rule 3) |
| 013 and 013-T1..T7 | M4–M9 and M12. T7.1 → M6.2/M7.2; T7.2 → M9.3; T7.3 → M5.3; T7.4 → M6.5/M7.4; T7.5 → M7.3; T7.6 → M7.5; T7.7 → M6.6/M7.9/M9.7; T7.8 → M4.1/M6.7; T7.9 → M7.7; T7.10 → M7.8/M8.3; T7.11 → M6.4; T7.12 → M7.6; T7.13 → M9.6/M9.7; T7.14 → M9.9 |
| 014 | M1 (retirement), M12 (certification) |

---

## Next action

M1 is active on branch `one-compiler` (worktree `~/lilscript-work/wt/one-compiler`):
- **Done:** M1.5 harvest, 288 cases.
- **In flight:** M1.1, M1.3 and M1.4 (the core batch), and M1.2 (tools, on branch `one-compiler-tools`).
- **Next:** M1.8's pre-deletion fleet gate; the M1.9 correctness debts; then M1.6 and M1.7.
- **Alongside:** M2.1 and M2.2.
