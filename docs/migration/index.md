# Migration plan: one compiler

Revision 2026-09-23. **This is the only plan.** It takes the codebase from two compilers in one binary to the single compiler described in [future-architecture.md](../future-architecture.md). It then carries that compiler to winning every maintained library under every objective.

The history of milestones 001–013 (receipts, batch ledger, measurements, the Closure ADVANCED inventory 013-T7) is kept in [record-2026-09.md](record-2026-09.md). Owner briefs are in `finer/intent/`; today's is [2026-09-23](../../finer/intent/2026-09-23.md).

---

## Where we are (2026-09-23)

**Two compilers share one binary.** `[compiler] backend` / `--backend` selects between them.

- **The old route (`compiler.rs` → `lower.rs` → `ir.rs` → `optimizer.rs` → `codegen_ir_js.rs` → `js_peephole/`, plus `codegen_native.rs`).**
  - About 148K lines with tests (42% of `src/`), and about 1,555 of the 3,092 unit tests.
  - It re-emits the whole program 267–381 times per build and reasons over its own printed text.
  - Its typed interprocedural optimizations are still ahead on small closed programs:

    | Corpus | Old route (Brotli) | Remaining compiler (Brotli) |
    |---|---|---|
    | 72 census cases | 5,866 | 8,576 |
    | `comparison/cases` (54) | 53 wins | 9 wins / 38 losses |
    | `comparison/algorithms` | 2,305 | 3,345 |

- **The remaining compiler (`compiler_service` → `semantic_program` → `structured_js`, plus `semantic_program/native*`).**
  - It is 3–12× faster per candidate, has zero census miscompiles, and has native C that is clean under ASan/UBSan/LSan.
  - It is ahead on the maintained libraries. katexlil, markedlil, posthoglil and jquerylil beat their bars on both objectives. The react-markdown family beats all seven bars; react-markdownlil's browser build is 27,248 Brotli against 31,082.
  - Open losses: motionlil (+8,612 Brotli) and zodlil's package (+12,211).
  - Its bytes so far come from about 40 hand-ordered rewrites of the finished JavaScript tree. Typed facts barely reach them: every user call is an unknown effect, class field identity is erased at elaboration, and native gets none of them.
- **The default was flipped to the remaining compiler on 2026-09-23** (commit `d362338f`). The tools still run the old route regardless:
  - the LSP;
  - the playground;
  - lint (on the old IR);
  - `--write-lock` effect summaries;
  - every `lib.rs` compile function.
- **No port pins a route.** Every port's committed `dist/` and Pages site is still old-route output from 2026-09-03 to 09-10. The current wins exist in scratch builds with `finer/port-migrations/*.patch` applied.
- **About half of the roughly 136 configuration keys do nothing** on the remaining compiler, and nothing says so.
- **CI has not been green since 2026-08-25.**
- **0 of 14 old milestones are verified.**

---

## Rules for every phase

1. **One compiler.** Nothing new may depend on the old route. Old-route code is read as prior art, never linked or ported line by line. The frozen reference binary is `~/lilscript-work/bin/reference-2026-09-23/lilscript`, built from `d362338f` with both routes. It is used only to measure "the first bar" (§M4–M7 reproductions).
2. **By design.** Every change is generic and owned. A fact is computed once, by its owner. **When a fact lands, the re-derivation it replaces is deleted in the same batch.** Formation never emits a shape a later pass undoes. No thresholds tuned on three ports: a choice goes to the codec, a structural bound comes from policy.
3. **Gates.**
   - **Correctness:** unit tests, the case runner, and the reference port suites.
   - **Size:** no Brotli degradation at batch end, and no per-library loss at phase end.
   - Byte identity is evidence, not a gate.
   - **Exact** rules (removing operations) must not raise any port's Brotli. Shapes whose sign varies ship as codec-judged **choices** or **terminal challengers**.
4. **Batches.** 4–8 changes per build, with one verification pass per batch and one ledger row. Builds and tests run on this host only.
5. **Evidence.** A result counts only when the delivered file is written by the compiler, with no post-minifier, and the receipt pins the binary, source, patch and dependency identities.
6. **Verification ladder.**

   | When | What runs |
   |---|---|
   | Per change | Unit tests and the case runner |
   | Per batch | probelil, `comparison/cases` per codec, and the reference port suites (katexlil, markedlil, zodlil, jquerylil, posthoglil, motionlil, micromarklil and its family) |
   | Per phase | The fleet |

   A change worth under about 400 fleet bytes is judged on micro gates or by the terminal slot, never by a fleet A/B.

---

## Phases

Each phase lists its tasks, what it depends on, and its exit criteria.

| Phase | Name | Depends on | State |
|---|---|---|---|
| M0 | Record and freeze | — | done 2026-09-23 |
| M1 | One compiler: the old route leaves the product | M0 | ready |
| M2 | Verification ladder and a green CI | M0 (runs alongside M1) | ready |
| M3 | Honest configuration, one public API, effort that buys something | M1 | waiting |
| M4 | The fact spine | M1 | waiting |
| M5 | Edit kernel, rule scheduler and program rules | M4 | waiting |
| M6 | Canonical formation and the annotated target tree | M4, M5 | waiting |
| M7 | Choices, challengers, naming, layouts and data | M5, M6 | waiting |
| M8 | Language for size | M1 (runs alongside M4–M7) | waiting |
| M9 | Native and cross-target | M4, M5 | waiting |
| M10 | Qualification and publication | continuous; closes last | waiting |

### M0 Record and freeze — done 2026-09-23

- Batches 29–32 and the default flip committed (`d362338f`).
- Owner briefs recorded verbatim: `finer/intent/2026-09-23.md`.
- Architecture written: `docs/future-architecture.md`, which supersedes `docs/compiler-design.md`.
- Plan rewritten (this page); history moved to `record-2026-09.md`.
- Reference binary frozen at `~/lilscript-work/bin/reference-2026-09-23/`, with its SHA-256 recorded beside it.

### M1 One compiler: the old route leaves the product

**Goal.** After M1 the product has no route switch, no old-route code and no old-route tests. `--backend`, `[compiler] backend` and `CompilerBackend` are gone. Every tool compiles through the one compiler.

| Task | Content | Notes |
|---|---|---|
| M1.1 Move shared pieces out of old modules | `FunctionSpelling` → `compilation_contract.rs`; `render_*_diagnostic` → `diagnostics.rs` with a route-neutral error type; the multi-file bundle manifest (`compiler.rs:1230-1385`) → the JS delivery module, with per-file sizes from the artifact record (no second Brotli encode); `measure_javascript_transfer_sizes` and the codec re-exports → `compression.rs`; `typed_array.rs` imports `primitive::Intrinsic`; tactic defaults move into the `TacticId` specs, so `resolve_policy` no longer goes through `optimizer::OptimizationOptions`; a single `JavaScriptCompilationContract` builder (`resolve_policy`) | Output byte-identical |
| M1.2 Tools on the one compiler | LSP: a check-only session (`with_checked_path`/`with_checked_source`), so the editor reports the compiler's own diagnostics. Playground: `compile_source_semantic`. Lint: AST rules on the checked module graph; IR rules on the Program IR (allocation, closure and indirect call in a loop; materialized array chains; the `web/eager-host-access` provider on the module initializer). `LintRuleContext` exposes the program instead of legacy IR (breaking change, accepted). `performance/aggregate-escape` returns with the escape fact (M4.6). `--write-lock` stops writing effect summaries: nothing reads them | lint tests and the LSP script pass |
| M1.3 CLI | Delete `--backend`, the old dispatch branch, `--profile-template`, the store census and the legacy `--explain` metrics. `--explain human` prints the compiler's report readably. One `request_for(target)` serves both `--print-policy` and the build. `--target all` writes every chunk. Native C uses one driver with strict numerics | — |
| M1.4 Configuration | Remove `[compiler] backend`; a config that sets it gets an error naming the one compiler. Every old-route-only key (about 55) produces a printed "no effect in this compiler" warning, from one per-key consumer table. `priority ≠ size-first` and `[policy.constraints]` are refused at load with an actionable message. Delete the old option derivations (`js_options`, `*_enabled`, `native_options`, `compress_pass_options`, the profile loader), the second contract builder and `ProjectConfig::javascript_compilation_contract`. Regenerate `docs/knowledge/config/schema.md` | Ports keep building; warnings list what to remove from their configs |
| M1.5 Tests | Harvest the executing old-route tests: about 271 compile LilScript and check output. Their programs and expected stdout become `tests/cases/regressions/*.lil` + `.out`, and they run on every case-runner lane. Delete tests that assert old-route IR or text. Re-point the cross-route tests (`structured_js/tests.rs`, `semantic/narrowing_input_tests.rs`, `semantic.rs`, `compilation_facts_tests.rs`) to the one compiler or to the interpreter oracle. Rewrite `comparison/cases/.../host/math-max`, which depended on the old route's name-keyed extern table, to use a declared host binding | — |
| M1.6 Delete | `compiler.rs` (what remains after M1.1), `lower.rs`, `ir.rs`, `optimizer.rs`, `value_analysis.rs`, `compress_passes.rs`, `codegen_ir_js.rs`, `codegen_js.rs`, `codegen_native.rs`, `js_peephole/`, `decision_registry.rs`, `artifact_memo.rs`, `profile.rs`, `js_externs.rs`, `for_of_family.rs`, `compiler_rest_capture_tests.rs`. Also: the module linker (`link_modules`, `ModuleCloner`, `locate_linked_span`, `$m` renaming) and the AST's linker fields; the old half of `compilation_contract.rs` (`abi_manifest` over the old IR); package effect summaries; `timing.rs`'s old buckets; the test-only annotated-tree experiment in `structured_js` (`lower.rs`, `lower/`, `analysis.rs`, `flow.rs`, `optimize.rs`, `constants.rs`, `compact.rs`, the `Planner` in `plan.rs`, most of `rewrite.rs`, `extract::JavaScriptView`); the fixed two-file resource cut (`physical_export.rs`, `javascript_resource*.rs`, `publication_fixed_resources.rs`, `fixed_resource.rs`, `resource_identity.rs`, `artifact_resources.rs`, `ResourceView` plumbing); `examples/structured-slice.rs`; the checker's dead `EscapeState` | Keep: `interpreter.rs` (the oracle), `primitive.rs`, `typed_array.rs`, `literal.rs`, `js_string.rs`, `js_regex.rs`, `js_syntax_target.rs`, `compression.rs`, `stable_hash.rs` |
| M1.7 Names | Rename by role: `semantic.rs` + `semantic/` → `check/`; `semantic_program/` → `program/`; its `javascript*.rs` and `structured_js/` → `js/`; its `native*.rs` → `native/`; `compiler_service` → `build`. `lib.rs` re-exports only the build API and the tools. `docs/current-status.md` is rewritten. The docs that describe the old route (`optimization-coverage.md`, `knowledge/compilation/current-architecture.md`, `modules-and-delivery.md`'s SSA references, `web-platform.md`'s name tables) are rewritten or retired. `docs/compiler-design.md` becomes a pointer | Mechanical; output byte-identical |
| M1.8 Verify | `cargo test` (all targets); the census on production lanes; probelil; the reference port suites. Built outputs are compared with the last pre-deletion binary: the same bytes are expected, and differences are explained | — |

**What M1 does not restore** (the old route had it, the remaining compiler does not yet; each has an owner task):

| Old-route capability | Restored in |
|---|---|
| The `pure` contract check | M4.3 |
| Removal of discarded pure calls | M5.2 |
| Unrolling of `inline for` | M8.9 |
| `@pool` | M8.9 |
| Same-named private classes in two modules | M8.1 |
| `object` singletons and `export constructor` | M8.2 |
| Native `Record`/JSON | M9.4 |
| The C extern ABI | M9.3 |

The old route's native C does not compile under Clang 18 today (0/72), so no working native feature is lost in practice.

**Exit.**
- No `backend`, "legacy" or "semantic route" appears in code, the CLI, configuration or current docs (history files excepted).
- One binary; all tests green.
- Every tool uses the one compiler.
- Built outputs are unchanged, or each difference is explained.

### M2 Verification ladder and a green CI

| Task | Content |
|---|---|
| M2.1 Green CI | Fix `cargo fmt` and `examples/semantic-integrated.rs`. One Linux job under about 15 minutes: fmt, `cargo test --lib`, the case runner, the `comparison/cases` micro gate with cached competitor artifacts, probelil, the codec contract, and the differential with a random seed. Publishing steps (web catalog, VS Code packaging, Playwright, Closure) move out of the gating path |
| M2.2 Case runner | One runner over `tests/cases` plus the harvested regressions. Lanes: `{formation-only, production} × {brotli, gzip, raw} × {script, module, C}`, plus one lane per optional family with only that family vetoed. It replaces the development-mode census and the knob configs in `tests/config/` |
| M2.3 Oracles | `.out` files come from the reference interpreter wherever it covers the program (51 of 72 today). Blessing is refused when the interpreter disagrees. Tests observe through a declared host sink or `print`, and `print` is never stripped (the language decision in §12 of the architecture) |
| M2.4 Admission parse | Every delivered JavaScript file is re-parsed by Oxc inside admission (the architecture's A5); the structural digest must match the printed tree |
| M2.5 Micro gates | `comparison/cases` and `comparison/algorithms`, per codec, reported every batch. A reported scoreboard until M5 lands the folds, then required (no losses) |
| M2.6 One port runner | One versioned runner (from `finer/tools/semantic-port-tests.mjs` and the `~/lilscript-work/tools` scripts). It records failing-test **sets**, diffs them against an expected-failure ledger with an owner per entry, and pins the compiler by digest |
| M2.7 Differential | The generator becomes type-directed with per-target feature masks. Its hand-pinned prologue shapes move to `tests/cases` |

**Exit.** CI is green on the one compiler. The case runner and micro gates run every batch.

### M3 Honest configuration, one public API, effort that buys something

| Task | Content |
|---|---|
| M3.1 Schema v3 | The axes of the architecture's §14: contract (`[target.javascript]` with `execution`, `world` and `format` independent), objective, effort, resources, execution and generated families. It ships with a translator in which every old key maps to a new key, a warning or a refusal |
| M3.2 Family registry from producers | Every program rule, JS rule and choice registers `{id, mandatory or optional, legality, risk}`. `TargetCompaction` splits into its real families. The five tactics with no producer are removed or given producers. The receipt lists the families that actually ran |
| M3.3 Public API | `build::{check, build, with_session}` with a typed `BuildReceipt` (no JSON prose), and `Delivered { files, manifest, sizes }`. The multi-objective CLI (`--objective raw,gzip,brotli`) produces one winner per objective |
| M3.4 Effort | A versioned schedule per level, re-derived on this compiler, replacing the old route's tables and the hard caps in `compiler_service::search_request`. Budgets enter the receipt and the fingerprint. `LILSCRIPT_SEMANTIC_WORK` is removed |
| M3.5 Codec pool | Bounded threads for render and codec work, with deterministic batch order; `--jobs` and `[execution]` take effect |
| M3.6 Environment variables | Only diagnostic environment variables remain. Anything that changes bytes is a policy key |

**Exit.**
- No accepted key is silently inert.
- `--print-policy` equals the build's request.
- Level 13 against level 15 changes the search measurably on the reference ports.

### M4 The fact spine

Facts on the Program IR, each with one owner. **Each task deletes the re-derivations it replaces in the same batch.**

| Task | Fact | Deletes | First reproduction |
|---|---|---|---|
| M4.1 Identities | `NominalId` for every nominal kind; class fields as `FieldRef{nominal, slot}` places; allocations carry their nominal; `Type::JsValue`; `CellBinding::Ambient`; declaration attributes (`pure`, `debug`); unit origin | Name lookups of classes (`Program::class`, `from_source.rs:1916`, native linear searches), the `"$js"` string tests, `ambient.rs`, the `debugLog` name tests | Printed output byte-identical |
| M4.2 Call graph | Complete call sets, value calls resolved to sealed producers, SCCs, the address-taken set, generalized from `CallableInputs` | The call-only scans in `inline.rs`, name-observability predicates in `javascript.rs:1265-1537` | — |
| M4.3 Effects | Per-operation and per-unit summaries, seeded by checked `pure` and trusted `pure extern`, with the termination rule of architecture §7. The `pure` contract becomes a check-phase diagnostic | `facts.rs`'s unknown calls, `demand.rs`'s private model, `helper_family` composition, `structured_js/analysis.rs` effects, `inert`, `runs_no_user_code`, `inert_value` | `scratch-program/ip/pure.lil`; motionlil's `warning`/`invariant` |
| M4.4 Values | One lattice: exact, finite set, int32 range, primitive class. Per value, formal, result and `(nominal, slot)` | `javascript_int32.rs` proofs, `NumberFacts` sources, `binding_classes` derivation, `simplify::known` | `scratch-target/a.lil` (`9+40\|0`); probe `f1` `(a.total\|0)` |
| M4.5 Initialization order | Settled root bindings; "not invoked before root statement S" | `quiet.rs` order and factory shapes, `root_constants.rs`'s own proof, `demand.rs:initialized` duplication | zodlil's 41 enum constants; `scratch-target/b.lil` |
| M4.6 Escape | Per allocation site: local, typed or host; a compare with `null` is not an escape | `scalar_objects.rs`'s syntactic escape test | snippet `s17`; probe `f1`'s per-iteration `[d,d+1\|0]` |
| M4.7 Field facts | Per `(nominal, slot)`: read, written-value join, host-reachable, reflective, exported shape | name-keyed field logic | probes `p2` and `p5`; motionlil's dead fields |

- M4.6: the lint rule `performance/aggregate-escape` (removed in M1.2) returns with the escape fact.
- Lint IR rules see the program before program rules; they move after the M5 rules when those land.

**Exit.**
- Every fact has one owner.
- No JS tree pass computes effects, initialization order or value domains from syntax.
- The verifier covers the new identities.

### M5 Edit kernel, rule scheduler and program rules

| Task | Content |
|---|---|
| M5.1 Kernel and scheduler | Structural `EditBatch` (insert, remove, splice, clone, delete unit, change signature, retype allocation) on the existing transaction; the rule scheduler of architecture §8; the verifier after each rule set in debug builds. DCE (demand's liveness as an edit) is the first production rule, and native gets it |
| M5.2 Removal rules | Discarded effect-free calls; unused `let`s whose initializer is removable; dead stores |
| M5.3 Parameters and returns | `Dropped` and `Constant` transports when the call set is complete; unused results. Negative cases: effectful arguments stay in order; `arguments`, rest, exported and host-visible `length` |
| M5.4 Root constants and defines | Constants settled before every read are substituted where they are no longer than the name. Longer literals and aliases are choices. Build-time `define`s resolve here |
| M5.5 Inlining | Direct and block inlining with the splice edit, ordered by an exact local print delta. Runtime adapters (`JS.invoke`, `JS.call`) are inlined. Known-closure calls, constant-capture cloning, constructor chains. Deletes `helper_family`, `forwarding_builtin`, `undefined_call` and the JS tree inliners' semantic parts; `place_single_calls` no longer makes IIFEs |
| M5.6 Namespaces and emulated methods | Collapse constant namespaces and devirtualize single-definition methods of compiler-owned objects, on allocation identity and initialization order. Deletes `flatten_constant_objects`'s syntactic proof |
| M5.7 Fields | Dead fields, constant fields and overwritten stores, per `(nominal, slot)`, reported for open and closed worlds |
| M5.8 Folding | Constant and branch folding on values; `\|0` elision from ranges before printing |

**Exit.**
- The census, `comparison/cases` and `comparison/algorithms` no longer lose to the reference binary in total.
- Every M4 reproduction is fixed.
- motionlil gains at least its measured T7.1 share.

### M6 Canonical formation and the annotated target tree

| Task | Content |
|---|---|
| M6.1 Annotation columns | Domain, `FieldRef`, `AllocSite`, `UnitId` and function facts, callee, initialization order, `GlobalId`, observation and spelling on tree nodes. The arena renumbers them itself. `literal_alternatives`, `binding_classes` and `defined_parameters` move onto nodes |
| M6.2 Target journal and scheduler | Typed mutation helpers with a journal; the JS canonical rules run in the scheduler. `javascript.rs:663-966` is deleted |
| M6.3 Canonical formation | `JS.call`/`JS.methodN` formed as method calls; constructions formed by the layout choice; defaults placed by the transport. Deletes `self_method_calls` (except for user-written `JS.call`), `dissolve_receiver_adapters`, `array_receiver_calls`, `inline_initializers`, `drop_redundant_init_stores`, `drop_default_arguments`, `native_default_lengths`, `drop_typed_default_checks` |
| M6.4 Pure printer | Structure rewrites leave the printer (`return c?a:b` consuming the next statement, logical statements, loop heads) and become spelling attributes. `\|0` and `++` come from facts |
| M6.5 Host modules | A typed Oxc visitor produces host units; the ESTree JSON walk is deleted |
| M6.6 Runtime helpers | Table decoder, reference helpers and adapters are written as LilScript prelude code, compiled through the pipeline and demand-pruned |

**Exit.**
- No pass recovers a shape formation emitted.
- No side table needs a hand remap.
- The printer holds no semantics.

### M7 Choices, challengers, naming, layouts and data

| Task | Content |
|---|---|
| M7.1 Choice interface | `Choice` + `ChoiceMap`. Product, record and string families and function layout become implementations. `implementations.rs`'s per-family vectors are deleted |
| M7.2 Objective as a judge | `raw_structure` and `raw_spelling` become per-family seeds. Every family is available to every objective |
| M7.3 Terminal challengers | A codec-verified stage on the final artifact. Spelling families (013-T7.2's subset: −462 Brotli over six ports measured) ship here |
| M7.4 Estimator and finalists | A per-objective size estimator ranks; Brotli-11 only on finalists; monotone incumbents; lower-effort replay |
| M7.5 Naming | One allocator: interference slots, printed-order assignment inside a scope, seeds judged by codec, alphabet per objective. Headroom measured: −353 Brotli over five ports against Terser's walk order |
| M7.6 Property names | Rename and ambiguate private `FieldRef`s (markedlil −252 measured), with the open and closed lanes reported separately |
| M7.7 Layouts | Per nominal or allocation: scalars, positional, named or class; struct parameters as fields; `JS.assume` view or decode |
| M7.8 Data | String tables become a choice with an estimator (no 64 / 0.85 thresholds); numeric and columnar tables; literal pooling at naming time, with real name lengths |
| M7.9 Function folding | Identical, permuted and one-constant functions, as a choice |

**Exit.**
- The search has real alternatives; each effort level's work and gains are reported.
- No objective-conditioned rule remains.

### M8 Language for size

| Task | Content |
|---|---|
| M8.1 Nominal identity per module | Classes and enums scoped per module like structs; the 16 port renames reverted |
| M8.2 One checker entry | A single file is a one-module graph; explicit phase products. `object` singletons and `export constructor` work |
| M8.3 Node ids | Ids on identifiers, declarations and statements; span-keyed fact maps deleted |
| M8.4 Operation catalog | `BuiltinCall` and `Intrinsic` merged into one declarative catalog: signature, defaults, effect class, fold, JS and C spelling, target capability. The "never rename" host surface is derived from `extern` declarations |
| M8.5 Checked `pure` and declared logging | `pure` validated (with M4.3); a `debug` effect class; `print` as a program effect |
| M8.6 Defines | Build-time constants from `[target.javascript.effects] define` |
| M8.7 Visibility and reflection | Per member, sealed by default, feeding field facts |
| M8.8 Types ports need | Unions and intersections, float `%`, non-null assertion, typed host shapes for `JS.assume`, TS-checked externs |
| M8.9 Directives as choices | `inline for`, `@pool`, region-scoped policy |
| M8.10 Ports use the language | Each port's `JsValue` transliteration is replaced where a typed construct now exists. The language analysis (`~/lilscript-work/out/arch/language.md`) ranks the targets |

### M9 Native and cross-target

| Task | Content |
|---|---|
| M9.1 Toolchain owner | One owner for compiler discovery, flags, strictness and sanitizer profiles, used by the CLI, tests, case runner and scripts. Native diagnostics carry spans and render as text |
| M9.2 Plain arithmetic | Remove the per-operation `volatile` (4.5× on float loops), keeping the static and runtime ABI guards |
| M9.3 Externs per target | `extern` declarations bind a JS host name or a C link name; the header is generated. `examples/extern_abi.lil` returns to `scripts/verify.sh` |
| M9.4 Portable records | `Record<T>`, `Object.keys/values/assign`, JSON natively |
| M9.5 Shared facts | Native consumes liveness (DCE), initialization order (no runtime guards), escape (stack storage, refcount elision), effects and specialization |
| M9.6 Portable subset | Exceptions (status propagation driven by effects), a string ABI that reclaims memory, regex, generators/async as a target-neutral transform |
| M9.7 Capabilities | Every operation and type declares its targets; the checker diagnoses against the requested target set |
| M9.8 Profiles | Cross triples and wasm32-wasi as toolchain profiles; the case runner runs one cross triple |

### M10 Qualification and publication

| Task | Content |
|---|---|
| M10.1 Ports own their sources | Each port's rewrite is committed to its own repository; `finer/port-migrations/` is retired. Sibling-line knobs leave port configs |
| M10.2 No post-minifiers | micromarklil's "smaller of compiler or esbuild" step, motionlil's esbuild+Terser `full.js` and zodlil's esbuild re-bundle are replaced by compiler-written files for every export condition |
| M10.3 Every library wins | For each maintained library and each objective, the compiler's artifact is at most the strongest pinned competitor (D4 threshold), open and closed worlds reported. The work list starts with motionlil (+8,612) and zodlil's package (+12,211), then the 20+ ports with unknown standings |
| M10.4 Rebuild and publish | Every port's `dist/` and Pages site is rebuilt by the pinned compiler, with compile times recorded in `site/results.json`; `npm run check:site` passes; one report of improvements against the last release and against Terser, Oxc/Rolldown and esbuild |
| M10.5 Receipts | Refreshed on the final binary; one scoreboard; separate conclusions for architecture, correctness, cost and size |

---

## Where the old milestones went

| Old | Now |
|---|---|
| 001 baselines, 002 boundaries, 003 policy, 004 facts and edits, 005 service, 006 integrated proof, 007 coverage | Foundations of the compiler. Their receipts are refreshed in M10.5 |
| 008 whole-program JS and delivery | M6 (formation, delivery plan), M3 (bundle manifest) |
| 009 reusable families | M5 (rules), M7 (choices) |
| 010 bounded codec search | M7 |
| 011 public compiler integration | M1, M3 |
| 012 speed and resource gates | M3.4, M3.5 |
| 013 compression qualification, 013-T1..T7 | M4–M7 (T7.1 → M4.3/M5.2, T7.2 → M7.3, T7.3 → M5.1/M6.2, T7.4 → M4.5/M5.4, T7.5 → M5.3, T7.6 → M5.5, T7.7 → M4.6/M7.7, T7.8 → M4.1/M4.7, T7.9 → M5.7, T7.10 → M5.8/M6.4, T7.11 → M4.4, T7.12 → M5.6, T7.13 → M7.6/M7.7, T7.14 → M7.9) and M10 |
| 014 retirement and certification | M1 (retirement), M10 (certification) |

---

## Next action

M1, in batches, on a branch cut from `d362338f`, with M2.1 and M2.2 alongside.
1. M1.1 moves.
2. Then M1.2–M1.4.
3. Then M1.5 and M1.6.
4. Then M1.7.

After M1 closes, M4.1 and M4.3 go first. They restore the `pure` contract and open every other fact.
