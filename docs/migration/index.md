# Compiler Migration Plan

Updated 2026-09-20. **This is the only active migration plan, including tasks, dependencies, progress and completion rules.** [Compiler design](../compiler-design.md) remains the target architecture and objective, unchanged by this consolidation. Its A1-A7 contracts govern implementation; D1-D5 retain their stated status. Language/configuration contracts and source/tests describe current support, not automatic adoption of the proposal.

Read this page's ownership rules, the assigned numbered section and the relevant design contracts. All fourteen sections live here; there are no separate step files, subordinate plans or parallel boards. `/home/azureuser/lilscript-migration` is a symlink to this folder, not another copy. Historical plans were moved outside the repository; [source disposition](#source-disposition) records retained requirements, superseded assumptions and unresolved archive availability.

**Implementation status: 0/14 milestones verified.** Bounded implementation, verification and cost diagnostics are recorded under 001, 003-006 and 012. Existing mechanisms and historical results can be requalified, but are not completion receipts for this plan.

## Objective and Acceptance

For each maintained equivalent library boundary and each frozen release profile, independently select raw, gzip and Brotli artifacts. Each must preserve behavior and public APIs and be no larger than its strongest eligible pinned competitor for that objective. Count complete delivery: adapters, helpers, dependencies, initialization, chunks and required resources. Invalid artifacts are ineligible; averages and cross-codec wins cannot erase a losing cell. D4's numeric strict-win threshold remains open.

Keep native executable/library support for the supported portable contract. Require useful compilation speed and bounded compiler/runtime costs. TOML separates language/host assumptions, objective, effort, family permissions and runtime constraints. More compiler effort grants no new semantic or runtime permission. Exact old bytes are unnecessary. Package detection, benchmark specialization of reusable libraries and post-score modifications cannot satisfy the objective.

Architecture conformance, full-library correctness, compilation/runtime envelopes and competitive size are separate assertions. All are required at completion. This plan provides tests for them; it cannot promise superiority on every future input.

## Fast Feedback and Search Policy

Optimize useful compressed quality per compiler cost, with Brotli the primary tuning priority and independently selected gzip/raw artifacts. We cannot certify a global optimum for arbitrary programs. An exact oracle is practical only for small, explicitly finite neighborhoods; broader search is bounded and heuristic.

Keep three decisions separate: **legality**, **exploration**, and **incumbent selection**. Every explored implementation must preserve the declared semantics and permissions. Its current byte count need not improve anything. Score estimates can influence scheduling, never prove equivalence or replace an exact requested-codec score. Promote only completely packaged, eligible artifacts; retain the previous admitted incumbent while exploring losses. A temporary loss is an experiment, not a reason to waive final qualification.

**Strategy defaults are judged on the fleet average** (owner, 2026-09-20). Optimization is not deterministic: a strategy that wins on average is often worse for some libraries, and every such decision is already a per-port flag in `lilscript.toml`. A default is adopted when it wins across the fleet; a library it hurts opts out in its own configuration, and that is a tuning task for the library, not a veto on the default. Qualification cells are still judged per library, with that library's best configuration.

Do not require each pass, naming change, or intermediate representation to shrink. Combined sharing, inlining, layout, strings, names and ordering can win where single changes lose. Conversely, a possible future interaction does not justify retaining every losing state indefinitely. Bound state count, retained bytes, work and codec probes; report discarded/unexplored regions and the actual stop reason. Protect some interaction/diversity work as well as promising immediate scores. A raw loser, a naming interaction, and a genuinely losing structural parent are different test cases.

Use this verification cadence:

| Point | Required feedback |
|---|---|
| Local owner change | Static checks and exact affected tests, plus nearby refusal/cleanup invariants; one incremental build per coherent change |
| Search or cross-family change | Small exhaustive oracle, interaction traps, independent codec winners, deterministic budgets and incumbent fallback; reuse the same binary for parameter sweeps |
| Coherent integration | A few frozen real-library boundaries and affected original cases, including held-out cases; independently measure delivered bytes |
| Public contract/entrypoint promotion or milestone closure | Relevant broader repository, library and native suites and effort/profile matrix |
| Final qualification | Complete required fleet, competitors and cost gates on the final production build |

Do not rebuild merely because docs changed or rerun every library after each edit. Reuse a binary only after checking its input/build identity; reuse behavior evidence only for matching tested artifacts and contracts. Log compilation/build time separately from test execution and compiler performance. A passing small test never silently replaces required final coverage.

Compare incumbent bytes against codec probes, logical work, wall/CPU time and memory, including time to the first valid artifact. First diagnose duplicate scoring, unavailable choices, lost combinations and spending on low-value neighborhoods. Add more complex ranking only when paired ablations and held-out cases justify its overhead. A smaller final artifact with unacceptable compiler/runtime cost is not a complete win.

## Starting Point

The 2026-09-19 source inspection establishes the following work, not fresh test results:

| Evidence in this checkout | Consequence |
|---|---|
| [`src/compiler.rs`](../../src/compiler.rs) public routes still use `lower_to_control_flow`, CFG optimization and generated-JS processing | Public service adoption in 005/011; obsolete ownership removed in 014 |
| [`src/semantic_program/`](../../src/semantic_program/) and [`src/structured_js/`](../../src/structured_js/) contain facts, edits, choices, target identities and bounded backend mechanisms | Reuse by contract and measured cost; module existence does not establish production coverage |
| [`examples/semantic-integrated.rs`](../../examples/semantic-integrated.rs) uses the scoped public service; source buffers, parser arenas and verifier scratch are admitted before growth, with remaining frontend gaps explicit | Complete remaining owner admission in 003/005/011/012 |
| Example direct/edit/search/native delivery now uses common admission, including unknown-runtime rejection and retained-error cleanup | Keep these regressions while expanding public delivery in 005/008/011 |
| Semantic search has age/diversity service and permits losing states, but its unmeasured-state and pending-artifact ordering relies on raw-size proxies | Measure Brotli quality per cost; do not describe it as wholly greedy or assume raw savings predict compressed savings |
| Search discovers one original-snapshot inventory and combines its proved choices; it does not discover newly enabled rewrites | Dependency-directed rediscovery and interaction tests in 006/010, not a whole-program rescan per candidate |
| Legacy `compile_path_explained_inner` finalizes module output after selection metrics are calculated | 001 measures actual delivered bytes; 005/008 finalize delivery before scoring |
| [`maintained-workloads.json`](../../benchmarks/libraries/maintained-workloads.json) has 27 libraries plus Probe and related `lil-solidjs` coverage | Preserve all 29 identities and roles; Marked, Probe, HAST, Zod, Remark, Mdast-to-Hast, Remark-Rehype, jQuery, Micromark, Motion and Remark-GFM have scoped required-case inventories, with qualification limits recorded below |
| Historical semantic, native and sixteen-profile results are bounded and predate this plan | Requalify relevant inputs; example trials and old gate status cannot certify the fleet |

Inspect source again before implementation. Dirty-tree content and installed dependencies, not just a Git commit, identify an input. Missing support and measurements remain explicit.

## Ownership and Replacement Rules

Implement one source-to-artifact pipeline. Each phase establishes an invariant in its owner and an ordinary production consumer. Connecting old subsystems with another coordinator, or moving files without removing duplicate authority, does not complete a phase.

| Responsibility | Target owner | Replacement and closure |
|---|---|---|
| Language meaning and checked identities | `SemanticProgram` (A1) | Transport checker knowledge once; no second mutable semantic model or recovery from emitted names. Establish in 004, cover the language in 007 |
| Facts, legality and edits | Revision-qualified facts and atomic `EditBatch` (A2/A3) | Remove duplicate effect/alias caches and untracked optimization mutation as families move, 004/009 |
| Configuration and resources | One resolved policy and compilation resource owner (A6/A7) | Stop lower-level TOML reinterpretation and private budgets. Establish in 003, integrate discovery-to-output in 005/011 |
| Alternatives | Compatible `Choice`/`Candidate` assignments (A4) | Common fact, edit, compatibility and risk interfaces; no solver or pipeline per family, 006/009/010 |
| JS syntax, bindings and delivery | `TargetProgram` and `DeliveryPlan` (A5) | Replace string-based binding recovery, grammar repair and post-score wrappers, 005/008 |
| Native layout and calls | Native lowering from the same checked meaning | Necessary target-specific layout/ownership without duplicate semantics or JS search, 005/007 |
| Eligibility and selection | One admission function, immutable `ArtifactRecord`, one search scheduler (A6) | Common eligibility on direct/edit/replay/search; naming search is a component, 003/005/010 |

These are responsibilities, not instructions to create a new Rust type for every name. Reuse implementations when they fit. Every new view, cache or layer must name its invariant, consumer, lifetime and measured benefit over simpler ownership/recomputation.

During transition the old compiler may be an explicitly selected reference. New-route tests report the backend and fail on unsupported forms; they cannot silently invoke old lowering or splice old output into a candidate. A necessary compatibility adapter gets an owner, supported contract, tests and removal condition here. Remove replaced responsibilities when consumers migrate; 014 audits remaining removal rather than postponing all cleanup. Retain independent parsers, test oracles and frozen reference artifacts as verification tools.

## Progress and Dependencies

A checked box requires implementation, accepted evidence and review of the assertions. `ready`, `waiting`, `active`, `implemented`, `verifying`, `verified`, `failed`, `blocked` and `stale` are distinct states. Bounded child work does not close its enclosing milestone or waive prerequisites.

| Done | Step | Prerequisites for closure | State | Accepted receipt |
|---|---|---|---|---|
| [ ] | [001 Baselines and support inventory](#001-baselines-and-support-inventory) | None | implemented; instrument frozen, 14 ports source-built and case-qualified on the main line after the config and fold fixes, awaiting ledger refresh and independent review | [accepted.json](../../benchmarks/migration-results/accepted.json) |
| [ ] | [002 Language and public boundaries](#002-language-and-public-boundaries) | 001 examples and support map | implemented; D2/D3/D5 settled, 18/18 boundary cases and the 72-case census pass with zero miscompiles, public export names and callable kind preserved; D3.6-D3.10 cases assigned to 007 | [accepted.json](../../benchmarks/migration-results/accepted.json) |
| [ ] | [003 Policy and resource ownership](#003-policy-and-resource-ownership) | 001, 002 | implemented; schema generated and checked, `--print-policy` receipt, 13 precedence and 75 policy tests pass, ownership table written; frontend allocation coverage assigned to 005/011 | [accepted.json](../../benchmarks/migration-results/accepted.json) |
| [ ] | [004 Semantic facts and checked edits](#004-semantic-facts-and-checked-edits) | 002, 003 | implemented; checked dead-value cleanup is the first real consumer, lineage generalized per rule, primitive-local and initialization-order facts added; control cleanup assigned to 009 | [accepted.json](../../benchmarks/migration-results/accepted.json) |
| [ ] | [005 Public fast JS and native service](#005-public-fast-js-and-native-service) | 004; consumes 003 policy | implemented; public source/path/CLI semantic route, full library suite 2,970/2,970 incl. native GCC+Clang separate-TU executions, measured first-artifact/RSS baseline; public value-struct adapters assigned to 006, remaining frontend allocations to 011 | [accepted.json](../../benchmarks/migration-results/accepted.json) |
| [ ] | [006 Integrated architecture proof](#006-integrated-architecture-proof) | 001-005 | implemented; D2 public value-struct adapter and D5 level-16 grant on the semantic route, reuse-versus-fresh measured under one ledger, owners traced, limits recorded; boundary shapes a copy cannot preserve assigned to 007, formation cleanup to 008, wrapper inlining to 009 | [accepted.json](../../benchmarks/migration-results/accepted.json) |
| [ ] | [007 Language and port coverage](#007-language-and-port-coverage) | 006 | implemented; census script/module/native C 72/72/72 with zero miscompiles and every native program clean under ASan/UBSan/LSan; 24 of 25 ports build semantically, 18 pass their whole suites and the rest wait on 008 or the environment; syntax and class inheritance closed; dynamic `import()` moved to 008 by decision | [census 007e](../../benchmarks/migration-results/2026-09-21-semantic-census-007e/receipt.json), [ports 007e](../../benchmarks/migration-results/2026-09-21-semantic-ports-007e/receipt.json) |
| [ ] | [008 Whole-program JS and delivery](#008-whole-program-js-and-delivery) | 006, 007 | implemented. Every delivery mode: single, preserve-modules, split, lazy `import()` and carried host modules. Liveness is verified across modules, and six compaction batches landed. Real libraries run unchanged in multi-file delivery. Semantic Brotli is now below the default route on zodlil and within 3% on markedlil. Codec alternatives (for-head, logical ifs) and per-stream scoring go to 010. | [then vs now](../../benchmarks/migration-results/2026-09-22-then-vs-now/README.md) |
| [ ] | [009 Reusable compression families](#009-reusable-compression-families) | 006, 008 | implemented. Five batches of generic target edits: unobserved function names, forwarding-builtin substitution, single-expression and statement inlining with arena renumbering, literal and store folds, spellings. Brotli since the milestone began: katexlil 61,391 → 57,589, zodlil 29,580 → 28,326 (below the default route's 29,682), markedlil 9,641 → 9,398 (default route 9,360), probelil 1,905 → 1,872. No runtime regression. The proof-heavy families measure 0–8 bytes and are kept until 013's fleet ablation. | [009 sections](#009-batch-1-substitution-and-folding-on-the-finished-program-2026-09-22) |
| [ ] | [010 Bounded codec search](#010-bounded-codec-search) | 006, 009 | waiting | None |
| [ ] | [011 Public compiler and fleet integration](#011-public-compiler-and-fleet-integration) | 007-010 | waiting | None |
| [ ] | [012 Compilation speed and resource gates](#012-compilation-speed-and-resource-gates) | 011 | waiting; phase telemetry, production-slice cost and probe ablations pass | None |
| [ ] | [013 Compression qualification](#013-compression-qualification) | 011, 012 | waiting | None |
| [ ] | [014 Retirement and final certification](#014-retirement-and-final-certification) | 001-013 | waiting | None |

Baseline collection and contract discussion can proceed together. 008 and 010 may begin against 006's validated interfaces, with closure prerequisites still binding. **No broad language/optimization migration before 006 passes.** Small interacting family implementations and exact selection belong in 006; they cannot wait for 009/010.

| Design contract | Establish and prove |
|---|---|
| A1 Semantic ownership and both backends | 002, 004, 005, 007, 014 |
| A2 Facts independent of output and precise effects | 004, 006, 007, 009 |
| A3 Atomic edits and dependent invalidation | 004, 006, 012, 014 |
| A4 Shared compatible alternatives | 006, 009, 010, 013 |
| A5 Target identities and complete delivery | 005, 008, 011, 014 |
| A6 Common admission, exact scores and bounded work | 003, 005, 006, 010-014 |
| A7 Independent configuration axes and useful fast route | 003, 005, 010-012, 014 |

## 001 Baselines and Support Inventory

Contracts: objective, A6/A7. Start with the workload manifest, public entrypoints, `finer/tools/{artifact-evidence,node-test-evidence,migration-inventory,portgate}.mjs`, `benchmarks/{codec-contract,statistics}.mjs` and `comparison/large-libraries/contract.mjs`. Use the repository-pinned Node version.

Freeze all 29 identities, roles, public boundaries, sources, tests, dependencies and deployment files. Reconcile other supported suites. A kernel, parse-only entry, application bundle or reduced export set is its own boundary, not a whole-library win. Related `lil-solidjs` coverage is not a duplicate independent win. Missing checkouts, adapters or suites remain required work.

Inventory every supported language feature, CLI/library/build entrypoint, JS/native mode and delivery mode, including diagnostics/source maps and other frontend consumers where applicable. Classify current and new-route support as implemented, partial, unsupported or unverified; attach real callers and allocate gaps to 007/008/011. Preserve complete required tests and downstream consumer coverage.

Pin runnable eligible competitor recipes, including applicable Terser, Oxc/Rolldown and Closure ADVANCED comparisons, with equivalent APIs and host/runtime assumptions. Record installed tool/dependency content as well as lockfiles. Freeze a tuning set and separate validation cases. Build valid incumbents from source; tests execute the scored artifacts. A failed build cannot inherit stale `dist`; a successful package command alone does not prove which bytes ran.

Before tuning, freeze a finite nonempty boundary x release-profile x codec matrix including default/release routing. Define complete delivery/transport accounting, numeric compile-time/RSS/runtime envelopes, representative speed targets, warm/cold definitions and sample/noise rules. Separate diagnostic profiles without moving losing release cells into them. Deterministic codec byte differences are real; timing requires repeated measurements. Keep the strict-win threshold open.

**Exit:** complete inventory and gap assignments, baseline receipts, comparison recipes and cost policy. Missing downstream adapters are explicit tasks, not omitted rows. Add a small receipt/progress validator using existing sound evidence primitives. Negative probes reject stale artifacts, missing cases, changed installed dependencies/configuration, tampered hashes, empty competitor sets and checked milestones without accepted evidence. This freezes the instrument, not future compiler qualification.

Bounded progress: [inventory receipt](../../benchmarks/migration-results/2026-09-19-inventory/receipt.json) records 29 identities, 27 maintained-library roles, 2,725 LilScript files and 220 direct installed package roots. Thirty-one focused evidence/inventory checks pass, including tampering and changed-source/dependency rejection. This is a census, not a complete transitive dependency graph or source-built baseline. Marked/Probe have frozen Node case inventories; Zod collection and Vitest production-byte observation are being qualified. Jest file lists are not case inventories. All 27 library production-test adapters remain unverified; missing dependencies, runtime delivery dependencies and vendor-source test bypasses remain explicit gaps. Public-entrypoint inspection predates the new 005 service and must be refreshed before acceptance.

Subsequent [Vitest evidence receipt](../../benchmarks/migration-results/2026-09-19-vitest-evidence/receipt.json) records 39 focused checks, structured collection of all 1,353 selected Zod cases and exact native loading of nine required production files. All selected cases pass, but 98 upstream implementation modules also execute through Vite; certification remains unverified. Excluded original files and additional CJS/package/browser coverage remain required gaps. The refreshed source-inspected census includes 22 compile/profile functions and the explicit semantic service/CLI route; it is not execution qualification.

The [HAST adapter and source-built baseline](../../benchmarks/migration-results/2026-09-19-hast-adapter/README.md) reuse the Node evidence helper, freeze 460 required identities and pass 35 focused evidence checks. An isolated source build with the qualified debug legacy-default compiler passes all 460 identities; scored ESM/closed hashes match observed production loads. Complete packaged ESM measures 30,101 raw / 9,971 gzip / 8,807 Brotli bytes; raw compiler output is a distinct boundary. Subsequent unchanged `check:pack` and source-built `check:site` commands pass, with the site's copied production JS matching scored ESM. The gate remains unverified for executable package/CJS, UMD/browser, declarations and complete delivery/dependency coverage; source-presence site assertions do not execute browsers. This is neither migrated whole-library support nor a release-speed claim.

The [existing language-case census](../../benchmarks/migration-results/2026-09-19-artifact-service/support-2026-09-19T13-10-34.721Z/receipt.json) uses the qualified service binary, development mode and `tests/config/no-optimization.toml` across all 72 existing script cases, without rebuilding Rust. JS passes 35 and diagnoses 37; native C passes 22 and diagnoses 50. Every emitted case matches its unchanged `.out`; no observed-output mismatch or native C build failure occurred. This is a first-error corpus census, not proof that all forms inside rejected cases are unsupported. Exports/module mode and individual source APIs remain separate contexts. Gaps belong to 007: module classes/enums/defaults; missing callback/constructor/template/statement conversion; additional JS primitives and script value-struct adaptation; native types, module captures, primitives, callable/default and print representations. Async/generator forms, other source APIs and complete feature/port maps remain uninventoried by this corpus.

HAST's [public-binding diagnostic](../../benchmarks/migration-results/2026-09-19-hast-adapter/public-boundary.json) also finds a gap outside its passing suite: upstream `toHtml.name` is `toHtml`, while source-built legacy ESM reports `kb` and closed output `mb`. Arity remains two; constructibility/prototype differ under profiles explicitly requesting arrow spelling. Resolve those public-contract/profile differences in 002/007/011; do not credit them as an equivalent compression win.

The [installed CJS supplement](../../benchmarks/migration-results/2026-09-19-hast-adapter/source-built-package-cjs/receipt.json) runs offline pack/install with scripts disabled, then observes the actual 31,100-byte CJS load. Seven of eight cases pass: resolution, exports, arity and four original-derived behavior assertions. The unchanged public-name expectation fails (`kb` versus `toHtml`), so the receipt remains failed/unverified with all cases preserved. This is a real observable difference under the current conservative comparison policy; upstream documents the named export but does not explicitly promise function-name reflection. D2 must resolve that distinction, not silently waive it for size. Source/workspace/installed-package hashes remain stable. This does not replace the original 460-case ESM/closed suite or establish complete CJS, browser or declarations coverage.

The [bounded evidence-command receipt](../../benchmarks/migration-results/2026-09-19-bounded-commands/README.md) passes 63 focused tool checks without a compiler build or full library rerun. Builds, tests, runtime probes and codec measurement now use one awaited process-group owner, with explicit timeout/output/cleanup failures. Node test evidence pins tool/runtime inputs. Escaped sessions, supervisor SIGKILL, event-loop/kernel stalls, synchronous copying and process RSS remain outside the bound. Marked's 75-case inventory now keeps additional closed-profile, VM/browser and package/deployment gaps machine-readable.

The [Marked debug baseline attempt](../../benchmarks/migration-results/2026-09-19-library-baselines/marked-debug-timeout/README.md) timed out after 300 seconds in its first Brotli compile using the preserved 760-check debug legacy-default compiler. It produced no invocation receipt, eligible artifact, size or executed test result. Source, workload and binary identities are preserved; the executed old synchronous runner was not source-pinned and is not retroactively identified as the new runner. No release-speed conclusion follows.

The [release integration checkpoint](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#release-integration-checkpoint) qualifies unchanged production source through the 89-check public cohort and original CLI/example consumers. Effective Cargo profiles and inherited override handling are recorded; all fifteen compared deliveries, six combined JS outputs and two native payload pairs match debug output. Five binaries are preserved. This is a release integration witness, not broad language migration, a per-codec win or a compiler-speed gate.

The [source-pinned Marked release attempt](../../benchmarks/migration-results/2026-09-19-library-baselines/marked-release-baseline/README.md) preserves the unchanged four-profile build, current runner and 75-node inventory. The first Brotli-profile compile finishes in 244.740 seconds with 34,092 bytes, but the whole build reaches its 300-second limit during the next profile. Canonical scoring and all tests are unreached, so the partial artifact is unqualified. Original sources, complete installed dependency identities and compiler inputs remain unchanged. One completed invocation exposes 268 emit calls, 541 codec calls and 23,924 generated-JS lexes scanning 813.35 MiB. Emission's accumulated 403.233 seconds includes IR-to-JS generation and six text folds; codec time is 13.300 seconds. These nested/concurrent elapsed scopes are not additive wall or process CPU time. Prioritize identifying work inside the broad emission scope before adding codec effort; this is cost evidence, not a measured optimization benefit.

A separate [retained-open Marked supplement](../../benchmarks/migration-results/2026-09-19-library-baselines/marked-retained-open/README.md) packages that complete first artifact through the unchanged fast path, with no recompilation. All 29 original nodes in three complete test files pass; observed ESM/CJS loads match the independently scored files. ESM is 34,176 raw / 10,450 gzip / 9,409 Brotli bytes; CJS is 37,339 / 11,296 / 10,152. Sources, all 1,109 dependency entries and tools remain stable. The failed parent remains failed, with 46 required identities and additional coverage still unexecuted. VM UMD observations do not establish exact scored-byte execution. This is a scoped older-backend baseline, not a size win, current-compiler qualification or milestone closure.

The [preserved-release Probe attempt](../../benchmarks/migration-results/2026-09-19-probe-baseline/README.md) passes both frozen original base cases and the first ten unchanged matrix profiles. The 22-profile command reaches its 240-second limit while processing `balanced`; twelve rows remain incomplete, including four unreached historical `name_ordering` configurations. No flags or expectations were translated and no retry was run. All completed outputs have exact production-load and independent codec evidence, with the required 70-byte host runner scored separately. Among completed observations, `searchOff` is 4,093 raw / 1,678 gzip / 1,556 Brotli bytes, while `searchAlways` is 3,920 / 1,569 / 1,465; their single compile observations are 0.206 and 55.088 seconds. Level 15 is three Brotli bytes larger than level 9. These are real size differences and useful cost diagnostics, not controlled timing claims or grounds to discard intermediate losses. The whole canary remains unverified; this older legacy release does not qualify current compiler inputs.

The [Remark adapter](../../benchmarks/migration-results/2026-09-19-remark-breaks-adapter/README.md) adds pinned prerequisite commands to the existing Node evidence owner, with failure, timeout and input-drift refusal before the original suite. Thirty Node/inventory, fourteen artifact and two phase-timeout checks pass in separately pinned cohorts. A later three-check CLI cohort also rejects explicit null prerequisites; the accepted library receipt retains its earlier pinned runner. Existing-dist discovery freezes all 23 original identities across four entries; it is not source-built qualification. Build, test and codec caps now have optional separate overrides, with defaults preserved and invalid values rejected before workspace creation.

The independently reviewed [Remark source-built baseline](../../benchmarks/migration-results/2026-09-19-remark-breaks-baseline/README.md) then passes both unchanged open/closed compiler invocations, the original type prerequisite, all 23 original Node identities and the separate original package dry-run in one attempt. Exact observed packaged ESM measures 2,783 raw / 1,243 gzip / 1,118 Brotli bytes; CJS is 3,793 / 1,678 / 1,525 and closed is 2,767 / 1,364 / 1,225. All 94 preserved outputs, 1,034 installed project dependency files and 2,106 global npm files are independently verified. The compiler belongs to the preserved accepted release, not newer Rust or semantic-backend inputs. Original CJS assertions cover export/function shape, not full behavior; installed-package execution, fresh site and UMD/browser qualification remain open. Static site assertions and package dry-run success do not close those gaps. No retry, source/flag/assertion change, competitor win or speed claim follows.

001-MH: [Mdast-to-Hast discovery](../../benchmarks/migration-results/2026-09-19-mdast-to-hast-adapter/README.md) freezes all 152 actual original identities across three entry files and 27 imported official modules. The unchanged type prerequisite passes; all 21 outputs, 61 source files, complete project/global npm trees and four exact ESM/closed loads pass independent verification. This existing-dist diagnostic is not source-built qualification. Both subsequent source-built attempts below retain the original bounds: 300 seconds build, 90 Node, the original 60-second prerequisite, 60 codec/package, clipped by a 600-second shared and 630-second outer bound. Source, configuration and assertions stay unchanged; no unchanged-failure retry occurs. Snapshot before edits remains `/tmp/lilscript-mdast-to-hast-before-20260919/`. The original upstream numeric footnote oracle remains distinct from candidate conversion. Additional CJS/installed-package/site/UMD obligations remain required; static site checks and dry-run packaging are not browser or installed-package execution.

001-F1 is a verified bounded [Marked feature-to-caller inventory](../../benchmarks/migration-results/2026-09-19-inventory/marked-public-features.json), SHA-256 `83a40672d804af9f39d6951e7b5f196adc1166155907f86d0043e887ef07bde4`. Six exact passing original case IDs establish four requirements: shared live defaults/mutation, ESM factory independence from live defaults, ESM/CJS callable aliases and exact public option keys. Source, original report, all 30 retained outputs and exact executed/scored ESM/CJS pins were independently checked without recompilation. Successive factory-result inequality, general reflection and exact UMD-byte execution are not inferred. `JsValue` and extern-class shapes are not D1 value structs: observed shared foreign objects must not become copies merely because their fields are known. Current semantic-route nominal registration rejection is separately source-inspected, not whole-library execution qualification. D2 and the complete feature inventory remain open.

001-MH-R1 [verifies retirement of an unsafe assignment sink](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#unsafe-assignment-sink-retirement). The [failed Mdast source build](../../benchmarks/migration-results/2026-09-19-mdast-to-hast-baseline/README.md) executes all 152 original identities: 60 pass, 92 fail (71 direct failures and 21 propagated containers), despite both fresh builds and type checking passing. All 117 outputs and complete inputs are verified. The same sink implementation exists in that old release and the newer compiler, so this is not attributed to class ownership. Root removes the optional rule and two helpers from `js_peephole/folds/copies.rs` and its two production entries from `mod.rs`, retaining implicit declarations in place. Six public-entry tests first reproduce five failures, then pass unchanged after retirement: 611 affected checks pass, with one pre-existing manual diagnostic ignored. The three obsolete private-fold tests are retired; old source and original behavior programs are preserved. Snapshot: `/tmp/lilscript-assignment-sink-before-20260919/`. No replacement heuristic or package exception is introduced; other first-use rewrites remain unchanged. Independent review verifies all 972 inputs, exact regression identity and both receipts under existing 600-second bounds. The failed library receipt remains failed; new-source replay, release and size qualification remain separate.

001-MH-R1 [release qualification passes](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#post-retirement-release-checkpoint): 89 public-integration library tests, one CLI unit test and all original example/CLI consumers. A cached follow-up passes all eleven new sink/class tests on the identical release binary without recompilation. Independent review verifies all 972 current inputs, effective release profiles, five preserved binaries and unchanged bytes for fifteen original deliveries. The existing Mdast wrapper accepts explicit SHA-pinned parent/binary arguments; eleven bounded argument/guard checks and independent review pass without another wrapper copy. Its failed attempt's archived wrapper remains immutable and its current-path tool pin is historical. Release fixture equivalence does not establish full compiler/fleet correctness or a competitive size gain.

001-MH-R1 is a verified bounded remediation after the [changed-source Mdast replay](../../benchmarks/migration-results/2026-09-19-mdast-to-hast-baseline/source-built-after-retirement/receipt.json), SHA-256 `f02752899c5970ab11fb91e098dd6a2cdf3bb883b945a99fee3dc488b68853e1`. Both original compilations, type prerequisite, all 152 frozen identities and the separate package dry-run pass, including every previously failing identity. Independent review verifies all 121 outputs, 24 input pins, unchanged source/dependency closures, four exact ESM/closed loads and five canonical codec rows. Executed ESM measures 13,965 raw / 4,753 gzip / 4,288 Brotli bytes. The historical failure and all 117 outputs remain unchanged; invalid earlier output is not a size baseline. The broader portgate remains unverified and exits 1 for unmet delivery obligations, while the wrapper's declared original-test/package boundary passes. Its inventory-derived text still calls the separate package check unexecuted; actual pinned command/result evidence supersedes that phase-stale wording without rewriting the receipt. No unchanged retry, package exception, full semantic-backend qualification, isolated speed/size attribution or milestone closure follows.

001-RR [discovery and inventory are verified](../../benchmarks/migration-results/2026-09-19-remark-rehype-adapter/README.md): the unchanged `npm run test:types && node --test test/*.test.mjs test/official/test.js` passes all 21 actual identities (18 test nodes including the official parent, three suites) across four original entries. The frozen inventory SHA-256 is `87a17c0cfb2899f4a6e571732aad4207cb60e948e98d2cc716cadfcc45fb23cf`. Independent review verifies all 21 preserved outputs, eleven tool/input pins, 35 source files, complete project/global npm trees and four exact ESM/closed loads. ESM exercises real processor/async integration; closed coverage is only callable shape. The sole existing-dist attempt exits 0 under its original 90-second outer/type-60 bounds, with no compiler invocation, install, source/configuration/assertion change or retry. Discovery alone remains explicitly unverified as compiler qualification. Only its inventory, one manifest row and evidence recipe were added. The separate source-built result below retains the existing package/banner version discrepancy as an observation, not a workload patch.

001-Q1 is a [verified shared-owner slice](../../benchmarks/migration-results/2026-09-19-node-library-qualification/README.md): the existing Mdast body now lives in `finer/tools/qualify-node-library.mjs`, with small data/CLI clients for Mdast and Remark-Rehype. The before-edit wrapper remains at `/tmp/lilscript-node-baseline-owner-before-20260919/qualify.mjs`, SHA-256 `7944deecdccbc96525eedc5458eabe6c65871b3b1ded756a4883da70f26905f7`. Portgate retains isolation, compilation, prerequisites, original tests and first measurement; the same extracted owner retains complete snapshots, trusted parent/binary checks, exact fresh invocation/delivery checks, original package check, codec replay and failure evidence. Recipes cannot bypass gates, and both owner and caller are pinned/archived. All 37 focused checks pass: five new owner tests, fourteen artifact checks and eighteen Node checks, with all 89 inputs stable. Negative cases verify no compiler execution; fully pinnable parent failures archive evidence, but missing/unreadable initial inputs have no complete archival guarantee. The independently reviewed 001-RR attempt below verifies the real flow. No Mdast replay, Rust rebuild or bounds change occurs. Archived wrappers/receipts stay immutable; Remark-breaks' older wrapper is historical-only because its discovery lacks the current npm-runtime snapshot contract. No compatibility branch or weaker pins are added.

001-RR's [source-built original boundary is verified](../../benchmarks/migration-results/2026-09-19-remark-rehype-baseline/README.md), receipt SHA-256 `9398ebb22148544f342edc9e97a5abafb74e9a3e73c5bec72a6a37bdf64e4f1a`. One attempt on the preserved qualified 19:48 release passes both original compiler invocations, type checking, all 21 identities, separate package dry-run and canonical codec replay. Independent review verifies all 96 outputs, 25 input pins, complete source/dependency trees, four exact ESM/closed loads and all five scores. Executed packaged ESM measures 14,192 raw / 4,845 gzip / 4,351 Brotli bytes. Actual outer supervision completes without timeout under the unchanged 300/90/60 phase and 600/630 shared/outer bounds. This parent predates C2 and does not qualify current semantic inputs. Portgate still exits 1 for broader obligations while the declared original-test/package boundary passes. Closed conversion behavior, CJS/installed-package/UMD/browser/fresh-site coverage, competitors and cost gates remain open. No retry, library/config/assertion change, speed/size win or milestone closure is claimed.

001-JQ's [existing-dist discovery and inventory are verified](../../benchmarks/migration-results/2026-09-19-jquery-adapter/README.md): all eight actual original identities, seven tests and one suite, pass through the existing Node evidence owner. Receipt SHA-256 is `c622329fefc6428424d3b3fb342a29dceb875b8ecd360877040e6eeb96191ddb`; inventory is `b5cf45d76b1089102502bcbdc071e3bceef682c7e57babda2a7ddf5b26fd6da6`. The unchanged `node --test test/compat.test.mjs` observes exact candidate ESM/CJS loads; upstream jQuery remains the separate expectation oracle. Independent review verifies all 15 outputs, eight input pins, 264 non-dist workspace files, 270 full-original files and all 1,829 installed dependency entries. Only the jQuery manifest row changes. Actual outer supervision finishes in 2.683 seconds under its 90-second bound, without timeout, compiler, npm, installation, source/configuration/assertion change or retry. There is no original type prerequisite or closed output. Original `check:names`, `check:pack`, `check:site`, full upstream/other existing consumer suites, browser coverage and source-built qualification remain explicit requirements. The current source qualifier's two-invocation/type/closed contract does not fit jQuery's single compile and custom delivery wrappers; do not fabricate stages or copy a runner. Its original `name_ordering` configuration also needs explicit compatibility resolution before source qualification. No current-compiler or compression claim follows.

001-JQ's [original consumer supplement](../../benchmarks/migration-results/2026-09-19-jquery-adapter/README.md#failed-consumer-supplement) **fails** on those same retained ESM bytes. The unchanged `benchmarks/popular/verify-jquery.mjs` already supports a SHA-pinned artifact override that skips compilation. One actually supervised 90-second attempt exits 1 in 6.459 seconds: `Deferred.pipe` throws `TypeError: Cannot read properties of undefined (reading '2')` at candidate line 2, column 4144, called from the original verifier's lines 282/294. None of its six completion markers is reached; they are script markers, not additional Node case identities. Independent review verifies all 34 outputs, 11 pins, unchanged source and both full dependency trees, all 15 copied discovery outputs and four exact same-process loads for candidate, original verifier/helper and separate upstream oracle. Receipt SHA-256 is `c55bddd96a4460c79fc0b81c5bae08168d505a95ec7cf3ad49c43faf46bcb9e3`. No compiler, esbuild build, npm, installation, original assertion/configuration change or retry occurred. Preserve the eight passing original identities and this distinct failing consumer boundary; the retained artifact is not a whole-library compatibility baseline. Current-source/compiler causality, later consumer sections and broader upstream/browser coverage remain unqualified.

001-JQ-R1's [generic capture reduction passes](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#rest-capture-reduction); **the original consumer failure remains open**. Starting from C3's accepted 974-input digest `b368f233b2f4e8ba730618796a08861d097aff4fe55abcbd952b88319a67c74b`, only test registration and a focused test file change. The initial two fixed-index witnesses and eight existing nearby tests pass. Refined dynamic-index witnesses then pass six emitter and four effort-15 configured-pipeline executions, including search Off/Production, nested name reuse, retained readers, unobserved final clearing and distinct per-call/enclosing identities. Independent JS references and exact callback/index-call counts constrain the observations. Each coherent test revision uses one build; only the two changed tests rerun after refinement. Final receipt SHA-256 is `976301c525f05fb4466ef8505a29eb689d4a7d999e20b2b6edfa46d9971eaee2`; the read-only audit verifies all 975 inputs and preserved debug binary. No production repair, schedule change, package-specific logic, library build or original assertion/configuration change occurs. The failure's historical pass and full current-source behavior remain unproven; do not infer a repaired library or a current capture defect from this passing reduction.

001-MM's [existing-dist inventory is frozen](../../benchmarks/migration-results/2026-09-19-micromark-adapter/README.md): all 1,963 original Node identities pass in one unchanged three-entry attempt. Six public root/stream/closed/UMD files and one separately identified test-only utility artifact have exact passive load records. Root's read-only audit verifies all 24 outputs, eight input pins and complete source/dependency closures; no independent agent review is claimed. Receipt SHA-256 is `7644929e24cbfae5c25ca9ef4fb79bcdceb177130b3e1934e53595f3d74fccb5`. This is discovery, not source-built qualification: original build inspection finds four compiler invocations, outside the current two-invocation qualifier. No compiler rebuild, package install, assertion/configuration edit, typecheck or codec ran. Original GFM, tokenizer and stream callers strengthen 002's evidence without deciding D2; CJS root/closed/UMD coverage remains narrower than ESM. Type/pack/site, installed-package, real-browser and fresh source-built coverage remain open.

001-MO's [original non-browser inventory is frozen](../../benchmarks/migration-results/2026-09-19-motion-adapter/README.md): all eleven identities pass from the recorded workspace in an isolated byte-identical copy. Its installed dependencies are absent; the explicitly recovered sibling tree is pinned completely and five relevant direct versions match the recorded lock. This does not substitute sibling sources or claim the historical installed environment. Eleven original production loads cover the barrel/feature graph, root CJS and full/mini/debug ESM; original Terser/esbuild/Vite consumers also pass. Root's read-only audit verifies all 24 outputs, eight input pins and complete source/dependency trees. Receipt SHA-256 is `13478e56418f011009b0b74e39fd92065f4516d22294fe4a2550eb34909d3b77`. Playwright is absent; original type/browser/pack/site, installed-package and broader delivery requirements remain open. Build inspection finds nine concurrent compiler invocations and explicit compatibility facades, outside the current two-invocation qualifier. No compiler, installation, codec or source-built qualification.

001-MO-R1's [geometry supplement passes](../../benchmarks/migration-results/2026-09-19-motion-adapter/README.md#geometry-supplement): five independent expectations pass separately against upstream Motion and the retained full candidate, ten new checks total. They verify destination/leaf identity, overlapping source/destination, ordered writes before an ordinary throw, a setter replacing the next leaf, and sequential aliased delta application. The first runner setup is refused before tests because its evidence directory is inside the supplied workspace; the corrected invocation preserves identical assertions and the failed receipt. Accepted receipt SHA-256 is `462522c111e9cbb28d4fb26f2569062e19115abe15e70df832e3bcec5181706c`. Parent inputs, full candidate and upstream loads remain exact and stable. These new cases are not original-suite identities, approved arbitrary-input semantics, current-source qualification or a reason for a speculative compiler patch.

001-MH-R2's [frontend-release replay passes](../../benchmarks/migration-results/2026-09-19-mdast-to-hast-baseline/README.md#frontend-release-replay) on 003-R1's explicit parent: both original compilations, type prerequisite, all 152 identities and separate package dry-run. All five JS outputs are byte-identical to the accepted post-retirement build, preserving every raw/gzip/Brotli coordinate; executed ESM remains 13,965 / 4,753 / 4,288 bytes. Root's read-only audit verifies all 122 outputs, 25 input pins, full source/dependency trees and four exact ESM/closed loads; no independent agent review. Receipt SHA-256 is `cb3783ef3c05c443cacc25d4ae20042f0ab39fb964c6647bc161a033c149196e`. One attempt uses the unchanged shared qualifier and original bounds, without another Rust build, source/configuration/assertion change or retry. This supplies one real-library checkpoint across C2-C7, not fleet, semantic-backend, competitor, speed/size-win or broader delivery qualification. The historical failure remains intact.

001-Q2 [guards expected fixtures before execution](../../benchmarks/migration-results/2026-09-20-fixture-preflight/README.md) in the existing Node evidence owner. Missing/changed pins prevent prerequisites and self-updating suites; prerequisite mutation prevents the suite, and final mutation checks remain. Complete dependency-tree capture moves from the source qualifier into the existing artifact owner, preserving nested/symlink identities and bounds without another implementation. Seven new checks plus existing affected cohorts provide 44 accepted checks. The first run passes 43/44; a new test's symlink cleanup fails, then its test-only correction passes all 17 affected artifact checks while 27 source-identical passes are reused. Root's audit preserves that failure/source and verifies both prior Mdast dependency snapshots and the unchanged 979 compiler inputs. No independent agent review, Rust build, source-built replay or milestone closure.

001-GF's [original existing-dist inventory is frozen](../../benchmarks/migration-results/2026-09-20-remark-gfm-adapter/README.md): unchanged `check:types` and all 22 original identities pass once in an isolated byte-identical copy. Eight parseable expected trees, all 46 fixture pins and both workspace copies remain unchanged with `UPDATE` absent. Four exact loads cover retained ESM, CJS and closed files; CJS/closed behavioral breadth remains limited. Root's audit verifies 21 outputs, 11 pins and complete source/dependency trees; the existing inventory consumer accepts the frozen IDs without a rerun. Receipt SHA-256 is `b584655ac815c09b7a7c6353d06757cc24cceef4f9a421050664662ef1ad32b0`. An initial manifest placement error was caught and corrected; only GFM's canonical row changes, and the intermediate manifest is preserved. No independent agent review. Original configuration, source and assertions are untouched. Fresh source build, branch-specific flag compatibility and broader delivery/competitor/cost obligations remain open; no compiler or codec ran. The differently named type prerequisite is handled by 001-Q3 below.

001-Q3 [verifies original prerequisite admission](../../benchmarks/migration-results/2026-09-20-original-prerequisite/README.md) in the shared source qualifier. The hardcoded `test:types` name is replaced by the exact declaration proven by both pinned discovery report identities, successful original cases, complete prerequisite supervision and before/after/final input identities. Exactly one npm-run prerequisite and all existing bounds remain required. Eleven focused tests pass, including read-only consumption of three real discovery receipts and four entrypoint probes proving refusal before producer execution; valid GFM `check:types` reaches the next gate. Root's audit verifies 25 final pins, 14 outputs, all 46 original GFM fixtures and unchanged 979 compiler inputs; no independent agent review or Rust/library rebuild. Read-only history identifies `terminal_cleanup_chain` and `wide_single_use_collapse` on sibling `migration/target-tree`, with neither introducing commit in current HEAD's ancestry. No equivalent current mapping is proven, so original source qualification stays unverified; no flags/scripts are dropped, aliased or renamed. This is tool admission evidence, not a compression or milestone-completion result.

### 001 Instrument: derived adapters, cost policy, recipes and a progress gate

The bespoke per-library discovery script is retired. Four owners now carry 001's exit, each with its own refusals:

| Owner | Responsibility | Refuses |
|---|---|---|
| [`adapter-derivation.mjs`](../../finer/tools/adapter-derivation.mjs) | Parse a library's own `npm test` script into a prerequisite list, a runner selection and explicit omissions | A script it cannot account for token by token; a reconstruction that differs from the original command |
| [`adapter-discovery.mjs`](../../finer/tools/adapter-discovery.mjs) | Run that command once against the existing distribution in an isolated copy, observing which artifacts load | A changed original workspace, dependency tree or tool; a pinned fixture that differed before the run; a declared entry no case loads is recorded as a delivery gap, never waived |
| [`cost-policy.mjs`](../../finer/tools/cost-policy.mjs) | Derive the frozen boundary x profile x codec matrix from the workload manifest and carry the numeric envelopes | A profile whose role is undeclared; a boundary with no primary objective or no release profile |
| [`migration-progress.mjs`](../../finer/tools/migration-progress.mjs) | Reconcile this table, the accepted-evidence ledger and the receipts on disk | A checked box without accepted evidence, a tampered or missing receipt, a stale artifact, changed installed dependencies or configuration, an empty or unpinned competitor set, a case that did not pass, a prerequisite milestone still open |

Seventeen negative probes in [`migration-progress.test.mjs`](../../finer/tools/migration-progress.test.mjs) exercise each refusal; the validator's first run on this page rejected a state cell that named no declared state, which is now corrected.

[`competitor-recipes.mjs`](../../finer/tools/competitor-recipes.mjs) makes the comparison runnable rather than remembered. Each recipe names its installed tool identity, its renaming policy and the codec that measured it; Terser, esbuild and Oxc through Rolldown run today, and Closure ADVANCED is refused for any boundary that supplies no externs, because running it without them deletes the public API and produces a dishonest number. Comparability is decided by the published name sets of the two artifacts rather than by a size ratio: on `markedlil` the reconstructed upstream publishes eighteen names against our eight, so that comparison is reported NOT COMPARABLE with `Hooks`, `Lexer`, `Marked`, `Parser`, `Renderer`, `TextRenderer`, `Tokenizer`, `lexer`, `parser`, `use` and `walkTokens` named as absent. A reduced export set is its own boundary.

[`source-baseline.mjs`](../../finer/tools/source-baseline.mjs) builds an incumbent from source with a named compiler binary, measures every produced artifact with the canonical codec, and runs the frozen required cases against those exact bytes in one receipt. It empties the committed `dist` in its isolated copy first, so a build that does not happen cannot inherit stale bytes, and it records compile wall separately from test wall.

### 001 Inventory state

Eighteen of twenty-seven maintained libraries now carry a frozen required-case inventory: the eleven recorded above plus `katexlil`, `posthoglil`, `rehypelil`, `rehype-katexlil`, `rehype-stringifylil`, `remark-mathlil`, `solidlil` and `unifiedlil`, derived from their own unmodified test commands. The nine that remain are tasks with named reasons, not omitted rows:

| Boundary | Reason it is not frozen | Assigned to |
|---|---|---|
| `cnlil` | Original command runs `node scripts/test-upstream.mjs`, a hand-written driver rather than the built-in runner | 001 follow-up: a driver adapter, or a declared inventory the driver emits |
| `mobxlil` | Original command runs Jest under `--experimental-vm-modules` | 001 follow-up: a Jest case-identity reporter, as Vitest already has for `zodlil` |
| `zodlil` | Original command runs `node scripts/test.mjs`; its Vitest inventory exists separately and is not yet reconciled with this derivation | 001 follow-up: reconcile the existing Vitest inventory into the same shape |
| `monacolil` | `package.json` declares no test script at all | 002/007: the boundary has no executable contract to freeze |
| `vuelil` | Workspace publishes no entry point and has no built distribution or installed dependencies | 001 follow-up: restore the checkout, or retire the row |
| `playcanvaslil` | The original `test:differential` prerequisite does not pass against the existing distribution | 007: a real failure of the shipped bytes |
| `mdast-util-from-markdownlil` | Original suite fails on the existing distribution: two `site` identities, a recorded-size assertion | 007: a real failure of the shipped bytes |
| `remark-parselil` | Original suite fails on the existing distribution across API, closed, package and site identities | 007: a real failure of the shipped bytes |
| `remarklil` | Original suite fails on the existing distribution across API, closed, differential, official corpus and to-markdown identities | 007: a real failure of the shipped bytes |

The last four are the important ones: they are not tooling gaps. The committed distributions of four maintained libraries do not pass their own original suites today. `rehypelil` looked like a fifth until its suite was run with the compiler pinned; it compiles a fixture inside its own test and had been reaching for a compiler path that an isolated copy does not have. Several ports' original suites invoke the compiler, so `LILSCRIPT_COMPILER` is now a pinned, recorded input of every observation rather than an ambient one.

### 001 Source-built incumbents

First run of [`source-baseline.mjs`](../../finer/tools/source-baseline.mjs) over the eighteen inventoried boundaries, on the compiler built from this working tree. **Six are source-built and case-qualified**: the compiler produced the bytes, the canonical codec measured them, and every frozen required case executed against those exact bytes and passed.

| Boundary | Compile wall | Required cases | ESM delivered raw / gzip / Brotli |
|---|---:|---:|---|
| `hast-util-to-htmllil` | 0.734 s | 460 / 460 | 30,101 / 9,971 / 8,807 |
| `rehype-katexlil` | 0.190 s | 67 / 67 | 2,235 / 1,033 / 898 |
| `remark-breakslil` | 7.980 s | 23 / 23 | 2,783 / 1,243 / 1,118 |
| `remark-mathlil` | 13.068 s | 63 / 63 | 6,382 / 2,533 / 2,290 |
| `remark-rehypelil` | 34.883 s | 21 / 21 | 14,192 / 4,845 / 4,351 |
| `mdast-util-to-hastlil` | 36.327 s | 152 / 152 | 13,965 / 4,753 / 4,288 |

The twelve that did not qualify separate cleanly into four causes, none of them a missing test adapter:

* **Unsupported configuration field** (nine boundaries: `jquerylil`, `katexlil`, `micromarklil`, `posthoglil`, `rehype-stringifylil`, `rehypelil`, `remark-gfmlil`, `unifiedlil`, and `solidlil` for a different reason below) — the build exits in under a tenth of a second because the compiler refuses the port's own `lilscript.toml`.
* **Compile envelope** — `markedlil` reached the 300-second per-profile ceiling during its multi-profile build and was terminated. Its partial output is not eligible, and the committed `dist` is not a substitute.
* **Environment** — `solidlil` reads `SOLIDLIL_LILSCRIPT_BIN` rather than `LILSCRIPT_COMPILER`; the driver now reads each build script and sets every compiler-shaped variable it names. `motionlil`'s recorded workspace had no installed dependencies, so the driver now prefers the first recorded candidate that has both a manifest and `node_modules`, recording the substitution. `zodlil` carries a directory symlink the input snapshot cannot hash in place; those are now named in the receipt as unpinned rather than failing the run.
* **No build command** — `probelil` has none recorded in the workload manifest.

Sixteen receipts are now accepted for 001 in [`accepted.json`](../../benchmarks/migration-results/accepted.json), each with what it is accepted *for*: eight frozen case inventories, six source-built incumbents, the cost policy and the configuration audit. `migration-progress.mjs` refuses the ledger if any of them is later edited, if a recorded input or the pinned compiler changes, or if 001's row claims more than they carry.

### 001 Competitor recipes: what survives an honest comparison

Thirteen boundaries can be reconstructed from an installed npm package. Each was bundled with esbuild, leaving external exactly what our own artifact leaves external, then minified by Terser 5.51.2, esbuild 0.28.1 and Oxc through Rolldown 1.2.5, and measured with the repository codec. Comparability takes two signals, both necessary: the two artifacts must publish the same names (same API) and their raw sizes must sit within 0.8-1.25 of each other (same program). Several ports bundle a pinned source graph rather than the npm package's dependency tree, and the npm entry then yields a much smaller program.

| Boundary | Our bytes | From | Best bar | Verdict |
|---|---:|---|---:|---|
| `hast-util-to-htmllil` | 8,807 | source-built | 9,833 Terser | **win 1,026** |
| `mdast-util-to-hastlil` | 4,288 | source-built | 4,857 Terser | **win 569** |
| `mobxlil` | 15,057 | committed dist | 16,665 Oxc | win 1,608 |
| `unifiedlil` | 4,642 | committed dist | 4,347 Terser | loss 295 |
| `katexlil` | 64,620 | committed dist | 63,044 Terser | loss 1,576 |
| `react-markdownlil` | 41,831 | committed dist | 30,837 Terser | loss 10,994 |

Three wins and three losses on the six cells that are the same program with the same API. **Only the first two rows are complete evidence.** Their bytes came out of a build this compiler performed, were measured by the canonical codec and passed every frozen required case; the other four compare a bar against a committed distribution, which is evidence about whichever compiler produced it. The committed distributions are in fact smaller than what this compiler now emits — `hast-util-to-html` ships 8,459 where the current build produces 8,807, and `mdast-util-to-hast` ships 4,232 against 4,288 — so a committed-dist comparison currently flatters us by a few hundred bytes. Both source-built wins clear the provisional strict-win threshold with an order of magnitude to spare. The other seven are reported NOT COMPARABLE with the reason named: `markedlil` publishes 8 names against upstream's 18; `zodlil` publishes 2 against 240; `jquerylil` 3 against 1; `motionlil` 326 against 312; and `mdast-util-from-markdownlil`, `remark-parselil` and `remarklil` face bars at 63.5%, 63.5% and 72.1% of their raw size, which is a different dependency graph rather than a 90% loss.

This is deliberately narrower than the fleet scoreboard in `finer/status.md`, which reports eleven wins and eleven losses against pinned numbers. The pinned numbers are not wrong, but several of them compare against a program that is not the same program, and until each of those boundaries is either matched in scope or re-declared as its own reduced boundary, its cell is not eligible under the objective.

### 001 Configuration support: the binding gap

[`config-support.mjs`](../../finer/tools/config-support.mjs) compiles one trivial source with each declared port configuration and reads the compiler's own diagnostic. Of **77 declared configurations, 61 are accepted and 16 name a field this compiler does not know.** None had drifted from its recorded hash.

| Unsupported field | Callers | Consequence |
|---|---|---|
| `name_ordering` | `jquerylil`, `katexlil`, `micromarklil`, `mobxlil`, `rehype-stringifylil`, `rehypelil` — all in `lilscript.toml` | Six boundaries cannot be built from source at all on the default profile |
| `terminal_cleanup_chain` | `posthoglil`, `remark-gfmlil`, `remarklil`, `unifiedlil` — all in `lilscript.toml` | Four more default profiles rejected |
| `wide_single_use_collapse` | `mdast-util-from-markdownlil`, `remark-parselil` — `lilscript.toml` | Two more default profiles rejected |
| `source_map` | `markedlil` hidden, inline and linked source-map profiles | The diagnostic source-map profiles cannot run |
| `analysis_map` | `markedlil` source-map analysis profile | Same |

**Twelve of twenty-seven maintained boundaries cannot be source-built on their own default profile with the main-line compiler.** `name_ordering`, `terminal_cleanup_chain` and `wide_single_use_collapse` appear nowhere in `src/` and nowhere in its history; they come from the sibling `migration/target-tree` line, and `source_map`/`analysis_map` from the source-map work that was never integrated. This is the reason 001's baselines have not landed, and it is a configuration-compatibility gap rather than a slow build.

Reading the sibling line settles what each field means, and the answer is not that main-line is missing three features.

* `name_ordering` is an enum on `migration/target-tree` — `emission-walk`, `frequency-desc`, `idiom-converged` — selecting how bindings are spelled after layout. All six callers ask for `idiom-converged`, a re-spelling pass in which repeated token shapes get the same spelling, decided from the tree. Main-line's nearest field, `idiom_directed_naming`, is **not the same mechanism**: it *offers an idiom-directed candidate beside the canonical one*, scored like any other, so it cannot make an artifact worse — and 059 measured that it never makes one better, with the cost monotone in the dose (four claimed bindings +21 Brotli on jQuery, sixteen +35, two hundred and fifty-six +130). One replaces a spelling, the other proposes one. Mapping the port flag onto it would be exactly the silent aliasing this plan forbids.
* `terminal_cleanup_chain` re-opens the canonical peephole on each finalist's text during cleanup. It is off by default on the sibling line too, priced there at 19 bytes for 60 seconds across ten ports. Four ports set it to `true` anyway.
* `wide_single_use_collapse` is the port's own answer rather than the plan search's, held constant across every plan of a compile. Two ports set it to `true`.

Every one of the twelve sets a non-default value, so none of this is cosmetic: **the maintained ports' default build configurations were written for the `migration/target-tree` compiler, not for the main line.** That is the finding. 001 asks for source-built incumbents from "the compiler" and there are two, with the fleet configured for the other one.

**Resolved on the main line (2026-09-20).** These are optimizer knobs — each decides how the output is spelled, never what the program does — and the owner's standing guidance is that strategy choices are per-port flags judged on the fleet average. So `src/config.rs` now accepts all three, validates their values (a misspelled `name_ordering` is still an error, and so is an unknown field), and the CLI prints one warning per knob that it is accepted and has no effect on this line. Nothing is aliased: `name_ordering = "idiom-converged"` is deliberately *not* mapped onto `idiom_directed_naming`, because one re-spells and the other only proposes. Three unit tests cover acceptance, silence at the anchor values and rejection of a misspelled value. The re-run audit: **73 of 77 declared configurations accepted**, the remaining four being `markedlil`'s diagnostic source-map profiles, which ask for a feature (`source_map`, `analysis_map`) rather than a strategy and hold no release cell.

Every port now builds on the main line with that line's default strategy. Whether any of the three knobs is worth porting is then a byte difference per library — the port's target-tree build against its main-line build — and a knob earns a port only if the fleet says so.

### 001 Evidence that survives a rebuild

The first receipts pinned `target/release/lilscript`, which is a build output: the next `cargo build` rewrote it, and `migration-progress.mjs` correctly reported every one of them stale because the binary they named no longer existed. During a migration the compiler is rebuilt constantly, so pinning the mutable path meant each rebuild silently voided all earlier evidence. [`preserved-binaries.mjs`](../../finer/tools/preserved-binaries.mjs) now copies each binary a run uses into a store keyed by its own SHA-256 (`~/lilscript-evidence/binaries/<sha256>/`), read-only and outside both the repository and `/tmp`, and the run executes and records that copy. The source-baseline, adapter-discovery and competitor drivers all use it, for the compiler and for the codec. A later rebuild changes `target/`, not the evidence, and a receipt stays checkable for as long as the store is kept.

Two recorded workspaces had also lived under `/tmp` and were removed between sessions. `vuelil` now points at the in-repository `labs/vue-client`, whose three configurations match the recorded hashes byte for byte; `motionlil` points at its durable checkout, whose default `lilscript.toml` differs from the removed audit copy and is re-pinned with that difference recorded in the manifest's `workspaceHistory` rather than silently.

### 001 The first source build found a miscompile

Accepting the sibling-line knobs let `unifiedlil` build on the main line for the first time. It built, and then two of its 227 required cases failed: `instanceof VFileMessage` was `false` for every message the library produced. The committed distribution passes those cases, so this was the main-line compiler, not the test.

`VFileMessage` is hand-written ceremony, because the language cannot `extend` a host class. Its constructor returns `createMessage(...)`, which links each new message to the module-level `messagePrototype`:

```js
b = new Error, Object.setPrototypeOf(b, i), b.ancestors = void 0, ...   // committed dist
b = new Error, b.ancestors = void 0, ...                                // main-line build
```

The call was **deleted**. `Object.setPrototypeOf` mutates its argument, so removing it because its result is unused drops an observable effect and every message loses its prototype.

Bisection with `LILSCRIPT_LIST_FOLDS` and `LILSCRIPT_SKIP_FOLDS`: kept at optimization levels 0, 3 and 8, dropped at 12 and 13, and kept with all 119 folds skipped — so a peephole fold. Skipping either half of the fold list still dropped it, meaning more than one culprit, so delta debugging shrank the skip set instead: twenty-four compiles, converging on a minimal set of two. **`fold_constructor_prototype_tables_to_classes` and `strip_stale_set_prototype_of` each delete the call independently.**

Both share one root cause. They decide whether a `setPrototypeOf` is dead by reading identifiers *as text*, in *text order*. Neither holds after minification:

* `createMessage`'s body appears textually before the module-level `i = m.prototype`, so `i` looks like it is "not yet a prototype" at the call. But a function body runs when it is called, after module initialization, when `i` is current. Text order is execution order only for straight-line module code.
* The names are minified. `parent_alias_is_unusable_prototype` decides `i` is unusable from occurrences of `i` in *other functions entirely* — a different binding that mangling happened to spell the same way.

This is precisely what A1 and A5 forbid — "no second mutable semantic model or recovery from emitted names", "replace string-based binding recovery" — and it is the sharpest argument so far for the migration: the bug is not a slip in one fold, it is what reasoning over emitted text costs.

The fix restores the guarantee both folds assumed. A `setPrototypeOf` removal now requires that the call executes at module level (a single function spanning most of the program is the module wrapper of a closed or UMD build, so those still fold), and the class-table fold only consumes a call sitting in the same function scope as the class declaration it is folding. All 601 peephole tests pass, plus a regression test built from unifiedlil's shape — including a reused minified name in an unrelated scope — which fails on the pre-fix source and passes on the fixed one. `unifiedlil` now builds from source and passes all 227 required cases.

Two lessons are worth carrying into 004 and 009. Text-order reasoning over minified identifiers is unsound at any effort level, and only `rehype-stringifylil`-style scope guards make it safe; and a port that cannot be built is a port whose miscompiles cannot be found, which is why the configuration gap above was worth closing before any tuning.

### 001 A second miscompile, found the same way

`remark-gfmlil` built on the main line and nine of its cases crashed with `Cannot read properties of undefined (reading 'call')`; the committed distribution passes all twenty-two. It passed at optimization level 8 and with every fold skipped, and delta debugging over its 128 folds converged on one: **`fold_void_initializers_off_fresh_vars`**, which rewrites `var x=void 0` to `var x` when it believes the reset is unobservable. The compiled micromark extension merger shows why it is not:

```js
for(var c in b)if(P(b,c)){var d=void 0;P(a,c)&&(d=a[c]),v(d)&&(d={},a[c]=d);…
```

`var d=void 0` sits in a loop and must reset `d` on every pass. The fold's loop detector walks braces, and this loop's body is a brace-less `if`, so it saw no loop; one hook's object leaked into the next iteration, the missing hook was never created, and a later `.call` read `undefined`. A second hole was found while fixing it: a function *declared* after the reset is hoisted and can write the binding before the reset runs. The fold now keeps the initializer when any loop between the function start and the declaration can reach it — braced bodies are bounded exactly, brace-less ones are assumed to — and when a later hoisted declaration, or anything nested in one, names the binding. Function expressions after it cannot run earlier and no longer block it. Two regression tests cover both shapes; all 603 peephole tests pass; `remark-gfmlil` builds from source and passes all 22 required cases. **Fifteen ports are now source-built and case-qualified on the main line.**

### 001 Cost policy

The frozen matrix is derived, not typed: twenty-nine boundaries, 228 cells, 186 of them required, over thirteen release profiles and twelve diagnostic ones. Profiles are classified by their own token, so a port that adds `lilscript.closed.toml` is classified without editing the table and an unclassified token is reported rather than assumed to be a release. `lilscript.check.toml`, `lilscript.nomangle.toml`, the four `sourcemap-*` profiles, `packs-safe`, `perf`, `performance`, `memory-parity`, `dev` and `debug` are diagnostic and may never hold a cell a release profile would lose.

Envelopes, each carrying the observation that set it: 300 seconds of compile wall per profile (Marked's first Brotli profile finished at 244.740 s and its four-profile build reached the limit during the next one); 1,500 seconds per library; 6 GiB peak compiler RSS; 60 seconds to the first valid artifact; a 0.95 ceiling on the codec's share of compile wall, which the 12-module fixture already sits at with canonical Brotli taking 1,877.934 ms of a 1,965.194 ms service wall; and a 1.10 runtime-regression ratio per harness lane. Timing uses five samples after one discarded warmup, median, arms interleaved, threads pinned and the host's burstable credit state recorded. Deterministic codec byte differences are real at any magnitude; a wall-clock difference smaller than either arm's interquartile range is not a result. Delivery accounting counts the artifact, its adapters and helpers, every chunk a consumer must fetch, bundled dependencies, initialization and required startup resources, and excludes source maps, declarations and development-only files. D4's numeric strict-win threshold remains open.

**The build pool is gone.** `finer/tools/workers.mjs` targets a scale set that no longer exists, and the subscription lists none. Every source build in this section therefore ran on this burstable host, one at a time. That is the binding constraint on the remaining baseline cells, and recreating the pool is an owner decision.

## 002 Language and Public Boundaries

Contracts: D1-D5, A1/A2/A5/A7. Consume 001's callers and [language](../language-v0.1.md)/[host](../web-platform.md) contracts; inspect checking, primitives and struct/native mechanisms.

D1 chooses value structs and explicit mutable references. Specify nested/generic/nullable/container copies, defaults, returns, captures and reference fields. Define overlapping aliases, escape/lifetime, retained callbacks and reentry. Include receiver/callee snapshots before argument evaluation and a callback replacing a parent during assignment. Physical scalarization implements these rules; it cannot choose them.

Resolve D2 with Motion mutating helpers, Micromark snapshots and their JS callers: retained objects, identity, mutation, enumeration, descriptors, serialization, callbacks and trusted/untrusted arguments. Specify adapters and public function observations, including `name`, `length`, constructibility and `this` where supported. Static annotations cannot prove arbitrary JS inputs are primitives or that fields are private.

Settle D3's exact exception/resource rule and cross-target obligations. Preserve results, ordinary throws/argument errors, host effects and divergence; keep constant evaluation bounded. Specify coercion/getter effects, short-circuiting, async/suspension and initialization. Distinguish Script, strict and module frames; a closed world alone does not imply strict mode. Resolve D5's effort/runtime separation and level-16 compatibility without recording proposals as owner decisions. D4's no-loss objectives remain fixed; its strict-win percentage stays open.

**Exit:** executable language/ABI tables with independent expected observations, positive/negative JS/native cases and explicit port migration requirements. Accidental aliasing or wrong output is a discrepancy, not an oracle. Keep unanswered decisions blocked only for dependent work; never invent public semantics to close a box.

Current caller evidence, source-inspected rather than newly executed: Motion's pinned upstream geometry tests require mutation of a distinct destination object, and projection nodes retain geometry and pass it to callbacks. Its maintained `copy.lil` and `delta-apply.lil` helpers currently mutate non-`ref` parameters. Under D1 those internal mutators need explicit references; public adapters must preserve actual destination/leaf identity and observable access order. Snapshot-in/bulk-copy-out is not automatically equivalent under aliasing, getters or partial throws. Existing package tests also observe callback retention/unsubscribe and prototype/method identity. Historical legacy-package passes do not qualify semantic-backend support.

001-MO now executes the recorded Motion workspace's original non-browser callers in an explicitly recovered dependency environment. Prototype/shared-method identity, arity, retained callbacks, accessors and promise identity pass unchanged. Its [separate geometry observations](../../benchmarks/migration-results/2026-09-19-motion-adapter/public-features.json) demonstrate why a bulk snapshot wrapper cannot preserve all existing JS behavior: overlapping input observes earlier output writes, an ordinary getter throw preserves partial mutation, and a setter can replace the next leaf before its copy. Both upstream and retained candidate satisfy independent expectations. These constrain a compatible boundary without choosing D2 or portable overlapping-reference semantics. An owner question now asks whether to adopt explicit JS boundaries with compatible adapters; until answered D2 remains open.

Micromark already separates private `PointState` copies from fresh public point objects in `micromarklil/src/create-tokenizer.lil`. Tokens and contexts are different: enter/exit share the supplied token, real GFM extensions mutate it and parser state, and original slice-serialization tests retain `[kind, token, context]` tuples. Explicit adapters can preserve those live objects; independent value copies cannot. An unexecuted compatibility discrepancy also needs an expectation: original initial-point fields retain truthy values, whereas the port's `jsNum` coerces them. Existing numeric Proxy tests prove a single read, not general coercion equivalence. These observations inform D2 but do not decide it or narrow required suites.

001-MM now executes the unchanged original Micromark selection against retained distribution files. The [case-to-feature mapping](../../benchmarks/migration-results/2026-09-19-micromark-adapter/public-features.json) distinguishes direct assertions from source inspection: numeric point fields are read once in order, inherited hooks execute, noncallable truthy resolvers throw, and event slices retain expected text. Stream consumers require real EventEmitter prototype/method identity, retained callbacks, once wrappers, removal and callback `this`; `node:events` is a required runtime boundary. GFM output passes but does not independently assert every token/context identity. Neither this passing existing-dist evidence nor the numeric point test settles arbitrary coercion, current-source behavior or D2.

Marked's executed original callers in 001-F1 reinforce the distinction: shared mutable defaults and callable aliases have observable identities, whereas factory results are independent of live defaults. Their declared extern-class shapes describe foreign objects, not value-copy permissions. Preserve these requirements when choosing adapters; the scoped cases neither settle broader getter/proxy/reflection semantics nor approve D2.

jQuery's original compatibility test adds an executed existing-dist D1/D2 witness in 001-JQ: `Tween.run` passes the same public tween to the step callback, binds `this` to the destination object, observes the callback's `now = 99` mutation during rendering and returns the same tween. Both the separate upstream oracle and candidate ESM pass the unchanged independent expected observation. `jquerylil/src/effects/Tween.lil` currently casts foreign objects with `JS.assume` into ordinary `TweenView`/`AnimOptsView` structs and passes `TweenView` by value-shaped syntax. These declared shapes do not permit losing foreign identity or callback mutation under D1. A supported migration needs an explicit compatible boundary, not accidental legacy reference behavior. The example does not settle arbitrary getters, proxies or D2, and does not qualify a current-source compiler build.

001-GF adds [original plugin callers](../../benchmarks/migration-results/2026-09-20-remark-gfm-adapter/public-features.json): `JS.method1` receives a foreign processor as `this`, and registration mutates its live `data()` store. Original direct/use calls and parser/serializer fixtures pass on retained artifacts. This supports an explicit host-object boundary, not privacy inferred from typed values or general getter/proxy/alias compatibility. D2 remains open.

### 002 The executable boundary table

D2 is settled (2026-09-20): typed internals meet JavaScript through **explicitly declared boundaries with compatible adapters**. That decides the shape of the answer, not what compatible means for any one observation, so each obligation is now a case that runs.

[`boundary-conformance.mjs`](../../finer/tools/boundary-conformance.mjs) runs every case twice — against the upstream library and against our artifact — and the rule is deliberately asymmetric. If upstream fails the case, the case is wrong and is reported as a broken expectation, never as a discovery about the compiler. If upstream passes and we do not, that is a boundary obligation this artifact does not meet. An expectation recorded from our own output would only prove we agree with ourselves, so `expected` is never taken from the candidate. Each case also records whether the observation is *documented* or merely *upstream behavior*, because that distinction is the part of D2 that stays open per case, and recording it is not waiving it.

First table, [`boundary-cases-unist.mjs`](../../finer/tools/boundary-cases-unist.mjs), covers the two boundaries that already have a source-built, case-qualified incumbent. **Sixteen of eighteen cases pass, and the two failures are the same defect.**

| Boundary | Cases | Result |
|---|---:|---|
| `mdast-util-to-hastlil` | 10 | 9 pass; `d2/function/public-name-and-arity` fails |
| `hast-util-to-htmllil` | 8 | 7 pass; `d2/function/public-name-and-arity` fails |

What passes is the substance of D1 and D3 at these boundaries: the caller's tree is not written to, the result does not alias the input, a node placed at two positions converts at both into distinct objects, caller accessors are read in upstream's order, an ordinary throw from a caller's getter reaches the caller unchanged, returned key order matches, returned properties are ordinary writable/enumerable/configurable data rather than accessors, the whole result serializes to identical JSON, a caller-supplied handler receives the caller's own node object, escaping is byte-identical, an unknown option is ignored rather than rejected, and the documented array argument form works.

What fails is function reflection, and it fails in two distinct ways at once:

| Observation | Upstream | Ours |
|---|---|---|
| `toHast.name` / `toHtml.name` | `toHast` / `toHtml` | `Ce` / `kb` |
| `Object.hasOwn(fn, "prototype")` | `true` | `false` |

The name is the failure 001's installed-CJS supplement already recorded. The missing `prototype` own property is new: our public entry points are spelled as arrows or methods, upstream's are ordinary function declarations, and `fn.prototype` is observable. Arity and constructibility already match.

These are exactly the two observations D2 now has to rule on per boundary — not whether to preserve behavior, which it does, but whether a *name* and a *prototype slot* are part of a declared public boundary. Upstream documents the named export and does not explicitly promise name reflection; it promises neither way about `prototype`. Both are cheap to preserve and both cost bytes, which is why the decision belongs here rather than inside a naming heuristic. Until it is made, both remain failing required cases rather than accepted differences. The two conformance receipts are preserved as failed evidence and are deliberately **not** in the accepted ledger: a table with a failing required case supports a finding, not a closure claim.

**Both observations are priced, and neither is expensive.** `hast-util-to-htmllil` explicitly sets `function_spelling = "arrow"`, which is why its public entry has no `prototype` own property. Rebuilding the same source with `function_spelling = "function"`:

| Arm | raw | gzip | Brotli | `hasPrototype` | `name` |
|---|---:|---:|---:|---|---|
| baseline, arrow as shipped | 30,101 | 9,971 | 8,807 | `false` | `kb` |
| `function_spelling = "function"` | 30,403 | 9,954 | 8,780 | **`true`** | `kb` |

The prototype slot costs +302 raw and moves Brotli by −27, which is inside the +/-100 noise floor: the honest reading is that it is **byte-neutral**, not that it is a win. One of the two failing observations is therefore available for approximately nothing, and the port's own `arrow` setting is buying nothing in exchange for a broken reflection contract.

The name is not yet available through configuration. The artifact emits `let kb=(a,h)=>jb(a,h); export{kb as toHtml}`: the export *name* is already preserved (`mangle.exports = false`), but the binding it re-exports is mangled, and `Function.prototype.name` reads the binding. Spelling that binding with the export's own name would emit `function toHtml(a,h){…}; export{toHtml}`, trading a longer identifier used twice against dropping the `as toHtml` rename — plausibly within a few bytes either way, but it needs a compiler change rather than a flag, so it is a 007 task with its price to be measured rather than assumed.

### 002 Positive and negative cases on the new backend

[`semantic-census.mjs`](../../finer/tools/semantic-census.mjs) compiles every `tests/cases/*.lil` program with `--backend semantic` for JavaScript and C, runs it, and compares its output with the case's `.out` file — written against the legacy backend, so an independent expectation. Each row lands in exactly one state; `diagnosed` is an honest refusal (a negative case that holds), and `wrong-output` is a miscompile, the one state never tolerated. On the current compiler:

| Target | Passed | Diagnosed | Wrong output | Crashed | C rejected |
|---|---:|---:|---:|---:|---:|
| JavaScript | 35 | 37 | **0** | 0 | — |
| C | 22 | 50 | **0** | 0 | 0 |

These are the same counts as the September 19 census, so the new backend's language coverage has not moved since then, and it has never miscompiled this corpus. What it refuses, grouped by the reason it gives — which is also **007's work queue, largest first**:

| Cases (JS / C) | What the semantic backend does not yet support | Examples |
|---:|---|---|
| 15 / 15 | Class and enum declarations in direct module checking | `19_class`, `20_class_loop`, `29_class_scalar_replacement` |
| 6 / — | JavaScript call implementation for array mutation, strings, floats, `slice` | `14_array_mutation`, `15_strings`, `21_float`, `array_slice` |
| — / 10 | Native source types (arrays, string arrays, function arrays) | `11_array_index`, `31_string_array`, `39_function_array` |
| 5 / 5 | The constructor-call contract | `float32_array`, `float_array_union`, `local_collection_elision` |
| 4 / 4 | Checked call signatures for higher-order callbacks | `12_array_map`, `13_filter_reduce`, `26_for_each_capture` |
| 3 / — | Value-struct callable frame adaptation in script mode | `17_struct`, `interprocedural_values` |
| — / 3 | Native print representation | `03_branches`, `string_code_units` |
| — / 3 | Native captured module storage | `06_short_circuit`, `23_global_constant`, `24_global_mutation` |
| 2 / 2 | Expression conversion (templates, type guards) | `16_templates`, `type_guards` |
| — / 2 | Native method primitives; — / 2 prepared call convention | `27_string_case`, `scalar_math` |
| 1 / 1 | Statement conversion; source parameter defaults | `41_inline_for`, `optional_comparators` |
| — / 2 | Native callable defaults and payload types | `integer_bitwise`, `interprocedural_array_length` |

Class and enum support alone would move fifteen cases on each target — more than the next three reasons combined.

### 002 Function reflection resolved: all boundary cases pass

The owner's standard for the public edge is that a JavaScript caller cannot tell the port from the original library. That settles the two open reflection cases in favour of preserving them: an exported function's `name` and callable kind are part of the declared boundary. Two changes deliver it:

* **Public names.** An exported function's binding is now spelled with its export name before any other name is allocated (`reserve_exported_function_names`, using the allocator's `claim_name`, so a reserved word or a name a host binding already owns keeps the mangled spelling). `let kb=(a,h)=>jb(a,h);export{kb as toHtml}` becomes `function toHtml(a,h){…}export{toHtml}` — the export also loses its `as` rename, which pays for most of the longer name.
* **Frozen public callable kind.** `function_spelling` used to set public *and* private functions (`javascript-shape-abi` already called that a legacy combination to normalize). It now governs private functions only; exported functions stay ordinary `function`s — constructible, with their own `prototype` — in both the emitter (`public_function_arrows = false`) and the compilation contract (`public_function_spelling` frozen to `Function`), so the compiler's own callable-ABI verifier checks the result. The first build after the emitter change was *refused* by that verifier until the contract agreed, which is the safety net working.

Result, on freshly source-built artifacts: **18 of 18 boundary cases pass** on both `mdast-util-to-hastlil` and `hast-util-to-htmllil`, and both still pass every original frozen case. The cost of exactness is **+9 Brotli bytes on each port** — well inside the ±100 noise floor — and both remain wins against Terser (8,816 against 9,833; 4,297 against 4,857).

**002 exit, accounted for:** executable language/ABI tables with independent observations (the 18-case boundary table against the upstream oracle, and the 72-case JS and C census against `.out` expectations, zero miscompiles); positive and negative cases on both targets (the census's passed and diagnosed rows); explicit port migration requirements (the public edge uses `export`, `export constructor`, `extern class` and host-class inheritance, `JsValue` only for genuinely dynamic values); D2 chosen by the owner, D3 worded in full, D5's flag model adopted. What 002 does *not* claim: D3.6–D3.10 have no executable case yet, and each needs one before a family that could affect it is enabled by default — assigned to 007.

### 002 The typed boundary for host classes

The owner's direction (2026-09-21): write libraries **typed** so the compiler can flatten them, and mark the public edge so it stays bit-exact to the original JavaScript API. The language already had most of that vocabulary — `extern class` for live JavaScript objects with exact names and identity, `export` for the public API, and `export constructor` for a class whose name, arity, constructibility and prototype methods are ABI while an ordinary class may be dissolved. What it lacked was the reason ports fell back to `JsValue`: **an internal class could not extend a host class.** `class VFileMessage extends Error` was rejected, so `unifiedlil` hand-built the prototype chain with `JsValue`, `JS.method3` and `Object.setPrototypeOf` — the very code the class folds miscompiled.

That is now supported on the main line:

```lilscript
extern class Error { string message; init(string message); }
class VFileMessage extends Error {
  string reason;
  init(string reason) { super(reason); this.reason = reason; }
}
export constructor VFileMessage;
```
```js
class VFileMessage extends Error{reason="";constructor(e){super(e);this.reason=e}};export{VFileMessage}
```

Name, arity, `instanceof` of both the class and `Error`, a native `message` and `stack`, own keys and calling without `new` all behave as a native subclass. The pieces: the frontend accepts internal-extends-extern and still refuses extern-extends-internal; an extern class may declare its host constructor with `init(params);` once, without defaults, and is still never `new`-ed from LilScript; a class with a host ancestor is always identity-observed, exported or not, because its instances are host objects — dissolving one stranded `super` outside any class, which a probe caught before it shipped; and `super(...)` to a host base rides on a non-pure `HostCall` with a reserved name, which every analysis already treats as an opaque, non-removable effect on `this`, so no optimizer pass needed to learn a new operation. Native targets refuse host-class inheritance with a diagnostic. Three new tests cover checking, refusals and runtime behaviour; the library suite passes apart from thirty native-harness tests that need a clang with the UBSan runtime, which this host does not have.

This is the port migration requirement 002 asked for, stated once: **a port's public edge uses `export`, `export constructor`, `extern class` and host-class inheritance; `JsValue` is for genuinely dynamic values only.** The semantic backend still lacks classes entirely (fifteen census cases), so the same feature must land there in 007 before the public route moves.

## 003 Policy and Resource Ownership

Contracts: A6/A7, D5. Reuse sound pieces of `src/{config,compilation_policy,output_budget}.rs`; reconcile [configuration](../configuration.md) with source.

Resolve TOML/CLI discovery and precedence once into contract, objective, effort/resources, family permissions, runtime limits and versioned schedule. Specify compatible keys, conflicts, unknown-key diagnostics and retirement of migration-only translation. Reconcile source effort 0-16 with older 0-15 prose using 002's level-16 decision.

Configuration inventory gap — **resolved 2026-09-20.** Probe's four `name_ordering` profiles, Micromark's and jQuery's (below) name a setting from the sibling `migration/target-tree` line. The main line now accepts `name_ordering`, `terminal_cleanup_chain` and `wide_single_use_collapse`, validates their values, and warns that each has no effect here; nothing is aliased onto the main line's different `idiom_directed_naming` (see 001, *Configuration support*). The historical paragraphs below are kept as the record of how the gap was found.

001-MM finds `name_ordering = "idiom-converged"` in Micromark's unchanged open configuration as well. Its four-invocation source build therefore has both a recipe-shape prerequisite and the same source-inspected schema compatibility gap. The passing retained-distribution suite neither executes the current compiler nor resolves either prerequisite.

001-JQ confirms the same configuration gap in jQuery's original build: `lilscript.toml` sets `name_ordering = "idiom-converged"`, absent from the `deny_unknown_fields` JavaScript schema used by direct TOML deserialization. Current `src/config.rs` SHA-256 `157bc865c87df7c637177c34c20ceedfadfb96110536c4cf69b47c998bd735da` exactly matches both input manifests of the preserved 19:48 release. Rejection follows from source inspection, not an executed compiler diagnosis. No other unsupported main/app keys or compression values were found in that bounded review; this does not establish whole-program compilation. The original build explicitly selects the main config, so swapping in the app config would change the boundary. Historical `name_ordering` and `idiom_directed_naming` coexisted as separate settings; no equivalent translation or D5 decision is established. Retain this prerequisite before any unchanged-source jQuery build attempt.

For optional families, `off` vetoes direct and searched use, `on` permits but never forces it, and `auto` follows declared policy. The registry distinguishes mandatory semantic/ABI lowering: all-optional-off still emits source literals, legal temporary names and required adapters. Exact-value analysis remains available with folding off. Unsafe assumptions and log stripping belong to the contract.

Establish one final admission interface for direct, edited, replayed and searched artifacts. Separate estimates from measurements; unknown runtime evidence cannot satisfy required numeric constraints. Establish resource ownership before discovery, with interfaces for checking/conversion, facts, failed attempts, edits, queues, rendering, codecs, retained storage and concurrent scratch. Instrument bounded consumers now; assign remaining public-phase integration to 005/011. Native performs no JS codec work.

**Exit:** schema, resolved-policy receipt, ownership/lifetime table and consumer tests for every axis, precedence and invalid limits. Test unknown/known runtime costs on every route, zero optional effort, mandatory-baseline failure, cancellation/cleanup and atomic refusal. State logical-accounting approximations separately from RSS and hard OS limits. Parser tests alone do not close this gate.

Bounded ownership task verified: immutable artifact records own naming/output choices and aggregate tactic/risk provenance, removing duplicate frontier/producer owners. Common qualification derives exact transfer evidence, checks the formation contract and aggregate dependency risk, and returns a receipt bound to source snapshot, codec, artifact and policy. Direct/edit/replay clients can use the same authority as search; explicit unqualified inspection remains separate. JS and typed native C/header outputs use the same arena. Complete package inspection is supported, but single-file handoff deliberately rejects packages. Frontend allocation coverage and whole-milestone policy decisions/gates remain open.

Scoped [artifact/service receipts](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md) first passed 154 focused Rust library checks, then 743 at the shared-owner boundary, 760 after checked rewrites/factory finalization, 842 after source/parser/verifier admission, 897 after token storage/semantic construction, 1,036 after checker fixed-table admission and 1,065 after parse-once module discovery. Later affected cohorts verify template/identity/target reuse (512), checker ownership (243), module interfaces/scheduling (298), canonical binding types (393), lexical work (246), parser lookahead (264), declaration vectors (348) and Analyzer storage (258 final affected checks); they do not rerun those broader checkpoints. Complete descriptors share admitted backing across candidates, search states and artifacts and survive candidate/search disposal. Frozen producer recipes and rewrite lineage participate in resource comparison/hash; resource format is 3 and the latest accepted schedule identity is 22. Cold failed preparation installs no descriptor cache, and recoverable service handoff errors release staged records. Native source-file counts are not executable-size measurements. Recipe replay and complete frontend allocation accounting remain open.

The 842-check resource slice admits source-buffer growth, source/discovery/main multi-file parser arenas and the existing full semantic verifier's work/scratch. Source charges remain live until storage drops; refusals stay typed and import diagnostics no longer eagerly copy parent sources. Exact-cap and below-cap tests independently calibrate successful and invalid verifier paths. Nonzero legacy generated-loop specialization now diagnoses before semantic parsing/I/O instead of invoking an unowned AST pass; legacy behavior is unchanged. All 15 original-consumer JS/C deliveries match the previous checkpoint. Render counters include shared allocation/movement work. This is neither whole-frontend accounting nor a speed claim.

The [897-check resource slice](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#token-storage-and-semantic-construction) admits lexical token vectors, including overlapping nested parses, and the existing semantic Lower's graph/temporary storage. Shared string decoding covers UTF16/UTF8 transient peaks. Prepared graph charges partition exactly into the existing publication owner without payload copies or a second reservation; adoption and store-construction refusal return the original ledger for source cleanup. All 15 original-consumer deliveries again match the previous checkpoint. Tests cover typed refusal, exact transfer, nested types/captures and independent string code-unit observations.

The [1,036-check slice](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#checker-fixed-tables) admits the checker's fixed source-node tables and outer module-facts vector through the existing Analyzer and one scoped callback. Lower runs while the checked model remains live on the same ledger; the model and syntax then drop before backend work. All 15 delivered JS/C files still match. At that checkpoint, template scanner buffers, pre-adoption source identities, remaining checker allocations, metadata and diagnostics were separately tracked gaps.

The [1,065-check parse-once slice](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#parse-once-module-discovery) removes the second full parse in multi-file compilation. The same loader retains programs against separate stable source storage, preserving existing graph identities, paths and diagnostics. Syntax and graph metadata drop before backend work; source backing drops after compilation, before its charge. No second grammar, self-referential graph or discovery semantic authority is added. Source insertion has a charged transient copy. Thirty paired debug trials retain identical bytes; combined discovery/parse medians improve but source backing grows and whole-compilation times are mixed. This is not a release-speed claim or complete resource admission.

Current bounded owner work, not additional prerequisites invented for 006:

| Task | Status | Required evidence |
|---|---|---|
| Remove the source-identity allocation | verified slice | Opaque inline identity preserves clone/source-mismatch behavior; typed capacity accounting replaces both old Arc charges; actual syntax/checker cleanup remains witnessed |
| Admit template storage in the existing lexical owner | verified slice | One grammar/scanner, unchanged public Logos and wrapper behavior, pre-growth frame/span/table admission, refusal/unwind and nested-parser overlap |
| Reuse baseline target formation across optional naming | verified slice | Separate baseline/optional budgets and incumbent fallback; one formed target/two naming bases for three styles, unchanged fixture observations/bytes and final release |
| Preserve unchanged checker signatures and release dead resolution scratch | verified slice | Nested/default-free signature ownership, unchanged finalization and cross-module observations; 243 affected checks, no new accounting claim |
| Admit module interfaces and initialization scheduling | verified slice | One shared checker/graph algorithm, exact AST-derived capacities, work/memory/deadline refusal and callback cleanup; 298 affected checks, schedule 14 |
| Remove duplicate value-binding type payloads | verified slice | Existing binding-span table references canonical symbols; nested payload addresses, detached parameters, defaults, model clones, nominal bindings and aliases preserved; 393 affected checks |
| Move lexical input work into the existing lexer | verified slice | Pre-admit every stream's byte tariff, including path and nested parses; preserve distinct read/hash/check/conversion work; deadline/EOF/refusal cleanup; 246 affected checks |
| 003-L1: admit unbounded parser lookahead | verified slice | One existing ledger, fallible arrow/type/reference probes; exact work/refusal and syntax parity; 264 affected checks, schedule 16; no additional depth cap or scan cache |
| 003-C1: remove duplicate class-member ownership | verified slice | One canonical member map pair; move own payloads during existing hierarchy resolution; preserve substitutions/identity/order/defaults/diagnostics; 409 affected checks, schedule 16 unchanged |
| 003-C2: remove whole-class copies at constructor use sites | verified slice | Borrow class/base metadata; retain only required names/shared signature; reuse resolved parameter types; five new tests and 298 affected checks, no new owner or accounting claim |
| 003-C3: admit canonical declaration vector backing | verified slice | Existing callback budget admits all four vectors, paired publication and growth overlap; six new tests and 348 affected checks pass, schedule 17; maps/nested types remain separate |
| 003-C4: admit Analyzer scope and callable-context backing | verified slice | Same budget admits six existing vectors; paired publication, typed refusal and per-Analyzer drop before release; four new tests and 258 final affected checks pass, schedule 18 |
| 003-C5: admit binary-expression continuations | verified slice | Existing iterative checker and callback budget; exact backing/growth and visit/probe work, nested overlap, refusal/scope cleanup and diagnostics; six new tests and 277 affected checks pass, schedule 19 |
| 003-C6: admit condition-narrowing traversal | verified slice | Same scope-sensitive algorithm and budget; two temporary vector backings/growth and leaf/probe work admitted; seven new tests and 284 affected checks pass, schedule 20; repeated-prefix work quantified, not optimized |
| 003-C7: remove empty-guard prefix rechecks | verified slice | Existing binary continuations carry syntax-only query inputs/projections; current-scope guard evaluation and diagnostics retained; five new tests and 289 affected checks pass, schedule 21; lower sparse-query work with a measured larger frame |
| 003-R1: refresh the frontend release checkpoint | verified slice | Unchanged C7 inputs pass 303 distinct release library tests, one CLI test and original consumers; cached checker cohorts reuse the identical binary, all 15 compared JS/C deliveries remain byte-identical; no full-fleet or speed/size-win claim |
| 003-C8: move owned resolved type arguments | verified slice | Seven built-in construction sites move nine Type payloads; nominal parameter metadata borrows; four new tests and 260 accepted affected checks preserve nesting, inference, shadowing and diagnostics; schedule 21 unchanged, no new accounting or measured speed claim |

The [512-check receipt](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#template-storage-and-target-reuse) verifies these slices after correcting two old test callers. The new target remains admitted across sealing only when continuation is possible, then drops before structural discovery; there is no persistent target cache or baseline-to-optional budget switch. Prior release/fleet evidence remains tied to its preserved inputs. This is not a release-speed or size improvement claim. At that checkpoint ordinary lexer work was still unadmitted; subsequent receipts below address it and lookahead, while parser depth and remaining checker maps/types/metadata stay open.

The subsequent [243-check ownership receipt](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#checker-signature-and-scratch-ownership) preserves shared signatures when default stripping is a no-op, detaches only changed branches, and drops module-resolution scratch at its last use. Five new ownership/default regressions and an executed cyclic-module observation pass alongside affected checker, source/module, public-service and JS integration tests. One build passed without retries. Schedule 13 is unchanged; the work neither admits remaining checker allocations nor establishes a measured library speed/RSS/size gain.

The [298-check interface/schedule receipt](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#module-interface-and-schedule-admission) puts public and admitted module checking through one implementation. Interface backing uses exact dependency/specifier/export counts, while the existing iterative scheduler admits its order and traversal storage. Graph checks and row publication charge work; actual model storage drops before its callback scope releases. Eight memory-refusal boundaries, seven work cutoffs, error/panic/deadline cleanup and original JS/native integration pass. Schedule 14 records this changed accounting; caps and expectations are not loosened. Maps, nested types, declarations and remaining traversal/diagnostics are still outside the accounting claim.

The [393-check canonical-binding receipt](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#canonical-binding-type-ownership) verifies the subsequent bounded owner task in `semantic.rs` and `semantic/modules.rs`. Value-binding types stay in the existing declaration symbols, with identities in the existing binding-span map. Type-only nominal entries remain inline; ordinary identifier-use spans do not acquire binding entries. The accepted 298-check manifest is its prerequisite, with the before-edit snapshot at `/tmp/lilscript-binding-type-before-20260919/`. Seven new regressions cover exact nested payload addresses, defaults, cloned models and module/foreign aliases. Entry and whole-row layouts do not grow on the tested build. One incremental build and thirteen affected cohorts pass under the existing 600-second command bounds, without library/profile reruns. No new semantic owner, allocation-admission or speed/RSS/size claim is introduced; schedule 14 is unchanged.

The [246-check lexical-work receipt](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#lexical-work-ownership) verifies the next owner task: `lexer.rs` admits each input stream's byte tariff before Logos traversal, with one cooperative check after every call including EOF/errors. The source service's old pre-parse tariff moves into this owner; distinct read/copy/hash, checker and conversion charges remain. Nested and path streams gain work coverage; templates retain detailed scanner work. Four new regressions and affected cohorts pass after one build without retries or higher memory caps. The 393-check manifest is its prerequisite, with the before-edit snapshot at `/tmp/lilscript-lexer-work-before-20260919/`. Schedule 15 identifies the changed accounting. No second grammar, prescan, persistent counter, hard-preemption claim or whole-fleet qualification is added; remaining recursion, checker and metadata work stays explicit.

003-L1 is [verified by 264 affected checks](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#parser-lookahead-work), based on accepted input `9514c49ed06f38a097e320f02a2f5a25aa88b078c148b9bd15b274a485a29db0` above. Root owns `parser.rs`, policy/report changes and the single build; focused tests stay in `parser_admission_tests.rs`. Before-edit files are at `/tmp/lilscript-parser-lookahead-before-20260919/`. Arrow/type/reference lookahead now charges each token probe, including existing EOF probes, through `Admission.work`; malformed lookahead remains an ordinary mismatch, while refusal propagates as a typed resource error. Six new tests verify exact tariffs and lower cutoffs, deadline/refusal, unchanged storage ownership and structured syntax parity. The same affected cohorts plus formatter/lint pass under the existing 600-second command bounds, without retries, higher memory caps or a fleet rerun. Schedule 16 records the coherent slice. Repeated scans, general traversal/recursion/stack depth and escaping diagnostic storage remain separate; this is not an algorithmic speedup or hard-preemption claim.

003-C1 is [verified by 409 affected checks](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#canonical-class-member-ownership), based on the accepted 264-check input `99b9f69cb9057dd4e417a7b008383b123461a379eb9033a325c88c153428581b`. Root changed `semantic.rs`; five tests live in `semantic/class_ownership_tests.rs`. Before-edit source remains at `/tmp/lilscript-class-ownership-before-20260919/`. Private duplicate declared-member maps and redundant whole-class clones are removed: definitions finish before the sole hierarchy pass, and its existing completed set distinguishes resolved classes. Borrowed base metadata and necessary inherited substitutions precede collision checks; own payloads then move into the effective maps. Tests verify allocation identities, substitutions, member order/slots, defaults, merged objects and exact diagnostics. Existing partial slot updates before a later collision are preserved. One build and 22 non-overlapping affected filters pass under existing 600-second bounds, with no retries; all 971 inputs remain stable. No new semantic/budget owner, grammar, cap or allocation-admission claim; schedule 16 is unchanged. Release, full-library, RSS, speed and size qualification remain separate.

003-C2 is [verified by 298 affected checks](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#constructor-metadata-ownership), based on the accepted 972-input release digest `e5a33cae693ff58a818bfc395a55bc6d958b6bf21c0679ae82e39f86311a9554`; all prerequisite pins were rechecked before edits. Before-edit source remains at `/tmp/lilscript-constructor-ownership-before-20260919/semantic.rs`. Four whole-`ClassInfo` copies are removed from `analyze_constructor`, both metadata lookups in `analyze_super_call`, and `ExprKind::New`. These consumers borrow class/base metadata; only required type-parameter names and a shared constructor signature survive mutable argument checking. Necessary inherited substitutions remain. Already-resolved constructor parameter types move into declarations instead of resolving again, and the `this` type borrows parameter names. Five new tests cover repeated/generic construction, defaults, inherited `super`, inference revalidation and twenty exact diagnostics; existing runtime callers also pass. One incremental build and 23 non-overlapping filters pass without retries or ignored tests. Independent review verifies all 973 current inputs, preserved binary and source scope. Absence of temporary whole-class copies is source-reviewed, not inferred from stable canonical pointers. No new helper/cache/ledger, accounting/schedule change, measured speed/RSS claim or library/profile rerun; release qualification remains tied to its earlier inputs.

003-C3 is [verified by 348 affected checks](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#canonical-declaration-admission), starting from C2's independently re-enumerated 973-input digest `51f23bccba5fc6faeb63739f7116ffdec3a53cff098210a39d0c32111eb29666`. Before-edit files remain at `/tmp/lilscript-declaration-admission-before-20260919/`. The existing callback's `AllocationBudget` admits all four `DeclarationTables` vector backings and old/new growth overlap. Both symbol capacities and publication operations are admitted before either logical row or scope/source binding is installed; partial reservation may keep capacity until cleanup, not publish half a pair. The same private Analyzer propagates typed resource failures with canonical module attribution; public unmetered APIs retain semantic diagnostics. Six new tests cover first/later growth, memory/work/deadline refusal, detached symbols, module-shared storage and callback error/unwind cleanup with independent exact capacity/work expectations. One incremental build and 25 non-overlapping filters pass, with all previous cases retained and no retry/ignored test. Independent review verifies all 974 current inputs, seven changed/new files, exact test identities and preserved binary; receipt SHA-256 is `c99b9df51c2eacdf8ba12886ad63ea252c4e13308f914a47952f6a61c94979ec`. Schedule 17 records the admitted work. No new checker, ledger, refusal flag or guessed map tariff; nested type/signature/default payloads, maps, Analyzer stacks, diagnostics, debug consistency scratch and comprehensive checker work remain outside this slice. No release/fleet, speed/RSS or compressed-size claim follows.

003-C4 is [verified by 258 final affected checks](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#analyzer-storage-admission), starting from the accepted 975-input digest `504cd39f6d3aff54c89c79f7408e6cd4810e4230656b9c32fa0f2d2ec9b7a813`. Before-edit files remain at `/tmp/lilscript-analyzer-storage-before-20260919/`. The existing scope, narrowing, type-parameter, return, constructor and generator vector backings use the same budget. Paired lexical/narrowing scopes publish together. Each short-lived Analyzer drops and releases only its own backing, preserving canonical declarations across module passes. Four new tests cover exact growth/work, failed construction, partial contexts, deadline/error/unwind cleanup and shared lifetime. The first build passes 352 of 354 checks; two new fixtures fail due to constructor spelling. Corrected fixtures and resource-report wording are the only subsequent changes; a second incremental build passes all six affected cohorts, without rerunning the fleet or unchanged broader cohorts. Root's read-only audit verifies all 976 inputs and the preserved binary; this is not independent agent review. Final receipt SHA-256 is `eeb54c085c768a7cf20331eed5e48ed044a55ea0c56f4db46efccdbc0a371276`, input digest `d2862e88e4e41815230e90268edc47b4f2c85da516d446a5988e2fb520a2f43b`. Schedule 18 records the added accounting. Maps, nested type payloads, alias/local-resolution and binary-expression continuation scratch, diagnostics and full traversal/native call-stack accounting remain separate. No new ledger, release/fleet, speed/RSS or compressed-size claim.

003-C5 is [verified by 277 affected checks](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#binary-continuation-admission), starting from C4's accepted 976-input digest above. The existing binary-expression worklist uses the same callback budget for backing, old/new growth overlap, visits and continuation probes. It drops and releases on success and ordinary errors, including nested calls; panic unwinding drops actual storage before the existing callback scope rolls back, without claiming resumable Analyzer recovery. Six new tests cover exact capacity/work, both tree directions, every work cutoff in two fixtures, nested overlap/reuse, short-circuit scope restoration, diagnostics, deadlines and live-worklist unwind. The existing module-work expectation includes the new nine-unit binary tariff rather than relaxing a cap. One build and eight non-overlapping cohorts pass, with no ignored tests, retries or fleet sweep. Root's read-only audit verifies all 977 inputs, five before-edit files and the preserved binary; no independent agent review is claimed. Receipt SHA-256 is `040684ed2ddf57f3feb30e0d013cfca8a17b9eb3cb4c4926e81d961333d5dff9`, input digest `3b739ba7e605fe838937bcee96f73f9dd656a457793b6b0c50e9bb7a3d7d8350`. Schedule 19 identifies the additional accounting. Nested type payloads, separate condition-narrowing worklists/maps, alias/local-resolution scratch, diagnostics and comprehensive traversal/native-stack coverage remain open. No new ledger, traversal algorithm, depth cap, release/fleet, speed/RSS or compressed-size claim.

003-C6 is [verified by 284 affected checks](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#condition-narrowing-admission), starting from C5's accepted 977-input digest. The same callback budget admits the existing pending/answer vectors and their growth overlap, fast-path leaf queries and iterative probes. Both temporary buffers drop before release on normal success/refusal/semantic failure; callback-scope rollback handles unwinding. Branch-map construction, left-to-right diagnostics, scope-sensitive lookup, assignment invalidation and shadowing remain unchanged. Seven new tests cover exact work/peak bytes, all four first/growing allocation boundaries, every work cutoff in a composed fixture, independent expected maps, scope changes, deadline/error/unwind cleanup and repeated-prefix cost. The prior binary fixture's analysis expectation gains exactly three leaf queries, without a cap relaxation. One build and eight non-overlapping cohorts pass, zero failures/ignored tests. Root's read-only audit verifies all 978 inputs and the preserved binary; no independent agent review. Receipt SHA-256 is `70828fd8a797b065fce38b9eafb109ac3bdc30c9f8968c7772872952eb972388`, input digest `e483ca9178e1abdb33d63418ceefc43e39ff76056423b67c59879dd7806055ce`. Schedule 20 records the added work. For constructed all-true 64-operator trees, combined binary/narrowing Analysis work is 6,433 units left-associated versus 322 right-nested. This verifies the existing repeated-prefix cost, not a speedup or permission to reassociate source. Returned maps, map merging, nested types and comprehensive checker accounting remain separate; there is no expression-only cache, new ledger, release/fleet, RSS, speed or size claim.

003-C7 is [verified by 289 affected checks](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#sparse-narrowing-inputs), starting from C6's accepted 978-input digest. Existing binary continuations now retain a syntax-only guard input and branch projection, not resolved maps. Empty narrowing branches need no query; a retained guard is evaluated in the current scope even when its projected results are discarded. One shared leaf classifier preserves type-check/null-comparison recognition. Multiple relevant guards retain a full subtree and unary negation remains conservative; there is no source reassociation or general linear-time claim. Five new tests compare 2,560 shape/context pairs with full traversal, preserve unknown/missing-fact diagnostics and every work cutoff in a sparse fixture, verify exact sparse-query work and execute JS script/module short-circuit observations. The prior guard-free witness now uses 258 Analysis units at 64 operators in either direction, versus C6's 6,433/322. A read-only debug-layout check also shows the cost: binary frames grow from 64 to 80 bytes, including nonlogical trees, which can increase scratch pressure. One build and eight non-overlapping cohorts pass with no retry/ignored test. Root's audit verifies all 979 inputs, six before-edit files and the preserved binary; no independent agent review. Receipt SHA-256 is `06762f10049771ac8b158a29d8a64277b1d1c5c39734cbc20470b89d67c7d364`, input digest `269288a54694ef126db4e16bba350254c0a7cc2629a6c6641cd5cf672511295a`. Schedule 21 records the changed work/layout. This is a bounded algorithm improvement and explicit memory tradeoff, not measured release speed, RSS, Brotli size or full migration completion.

003-R1's [scoped release checkpoint](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#frontend-release-checkpoint) qualifies the same 979 inputs as C7. The existing public-integration owner passes 89 library tests, one CLI test and original direct/search/paired/native consumers; 214 affected checker/capture/sink tests then reuse the identical cached release binary. All 303 library identities are distinct, with no failed or ignored tests. Fourteen original JS deliveries retain independent observations and codec replay; their bytes and one native C delivery match the prior accepted release. Five binaries are preserved at `/tmp/lilscript-frontend-release-20260920/`. Root's read-only audit verifies the inputs, outputs, release profile and binary pins; no independent agent review. Main receipt SHA-256 is `e6301eff8cf5d57d39313452af9e6d1babb09894406e3bf43e6c967b7c5a116a`. This supersedes the older release only for its stated boundary; no source edit, fleet qualification or speed/size improvement is claimed. The single selected real-library checkpoint, 001-MH-R2 above, also passes with unchanged artifacts.

003-C8 [verifies resolved type ownership](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#resolved-type-ownership). Explicit Map/Set constructors and Map/Set/Task/Generator/Record annotations consume newly resolved arguments instead of cloning nested payloads; nominal struct/class parameter lists borrow canonical metadata. Necessary contextual-inference clones remain. The initial cohort passes 256/260 checks; four new fixtures expose existing nested-closer grammar and type-span conventions. Their test-only correction passes all four, reusing 256 source-identical checker/service/conversion/module/JS/native passes. Both attempts, binaries and initial assertions are preserved. Root's audit verifies all 980 current inputs and that only the new test differs between attempts; no independent agent review. Final input digest is `ef50031f3894c1771480f02496c58d5672a5bc7997229b1fdd7e3eb36b0b68f6`. This removes repeated subtree copying without a new resolver/cache/owner or changed semantics. Schedule 21, accounting gaps and migration prerequisites remain unchanged; no release/library/fleet rebuild or measured speed/RSS/size result. The preceding release qualifies its earlier inputs only.

### 003 Closure evidence

**Schema.** [`docs/knowledge/config/schema.md`](../knowledge/config/schema.md) is generated from the doc comments of every field in `src/config.rs` and `src/compilation_policy.rs` by [`config-schema.mjs`](../../finer/tools/config-schema.mjs): 136 accepted keys across ten sections, each with type, default and meaning, and every table marked *closed* when it rejects unknown keys. Hand-kept docs had drifted — nineteen accepted keys were documented nowhere, two of them added this week — so `--check` now fails when the generated reference and the source disagree, or when an accepted key has neither a source doc comment nor a prose entry. The seven section containers that had neither now carry doc comments.

**Resolved-policy receipt.** `lilscript <input> --print-policy` prints the policy the compiler will actually use after defaults, the TOML file and command-line flags are combined: contract, objective, effort, all fourteen tactic permissions, resources and constraints, with its fingerprint, the configuration file it came from, any accepted-but-unimplemented knobs, and an `execution` block (threads, codec workers, mode, backend, target) that is deliberately **outside** the fingerprint because thread counts must never change output.

**Precedence and invalid limits** — [`policy-precedence.test.mjs`](../../finer/tools/policy-precedence.test.mjs), thirteen cases through the real command line: TOML resources apply without flags; `--jobs`/`--codec-jobs` override TOML; thread counts never enter the fingerprint; identical inputs give identical fingerprints; `--mode development` overrides configured search and changes the fingerprint; effort follows `optimization_level` including 16; level 17 is refused; an explicit tactic permission reaches the resolved policy; an unknown key is an error; `codec_workers = 0` and `--jobs 0` are refused; a sibling-line knob is accepted, reported and still validated; a `lilscript.toml` beside the input is discovered; an explicit `--config` wins over discovery.

**Ownership and lifetime.**

| Resource | Owner | Lifetime | Bound | Accounting |
|---|---|---|---|---|
| Logical work | `BudgetLedger`, one per compilation attempt | The attempt | `policy.resources.logical_work`, and always the finite `BaselineFirstPlan.logical_work` | Logical units charged once per unique dependency attempt; a cache hit charges exactly like a cold run. Not CPU time |
| Retained output storage | `AllocationBudget` scopes borrowing the ledger | *Retained* may outlive its phase and moves to the parent on success; *scratch* is phase-only; a scope's drop rolls back both | `policy.resources.retained_bytes` and `BaselineFirstPlan.retained_bytes` | Declared buffer and layout capacities — **not** allocator bookkeeping and **not** process RSS |
| Baseline reserve | `BaselineFirstPlan` | Until sealed | Its own finite ceilings, reducible by `policy.resources` | Mandatory output completes before exploration; a sealed baseline may release storage but never grow it |
| Terminal work | `BaselineFirstPlan.terminal_work` | After the baseline seal | Reserved up front | Withheld from both baseline construction and exploration |
| Wall time | The ledger's deadline | The attempt, including clones of the ledger | `policy.resources.wall_time_ms` | Cooperative: checked at every work and memory admission, never preempts between them; releasing memory is always allowed after expiry |
| Codec probes, retained candidates | The objective | One search | `optional_codec_probes`, `retained_candidates`, `retained_candidate_bytes`, `beam_width` | Counts and bytes as declared |
| Threads, codec workers | `[compiler.resources]`, overridden by `--jobs`/`--codec-jobs` | The process | `NonZero` | Execution only; outside the policy fingerprint |
| Frontend buffers (source, parser arenas, verifier scratch) | Admitted through `AllocationBudget` before growth | Per module | The same ledger | Partial: remaining frontend owners are explicit gaps assigned to 005/011 |
| Physical CPU and RSS | Not owned by the compiler | The process | 001's cost-policy envelope (6 GiB peak RSS) | Real OS measurement (GNU time), reported separately from every logical count above |

**Behavioural tests already in the tree, mapped to the exit.** Zero optional effort: `zero_optional_work_still_has_a_measured_direct_route`. Mandatory-baseline failure: `unavailable_baseline_and_overflow_fail_without_partial_charges`. Cancellation and cleanup: `deadlines_are_explicit_cooperative_limits`, `deadline_blocks_memory_admission_but_always_allows_cleanup`, `cloned_ledgers_keep_the_compilation_deadline_origin`. Atomic refusal: `baseline_first_phase_gates_are_atomic_including_zero_admissions`, `deadline_checks_all_work_admissions_without_partial_charges`. Unknown and known runtime costs: `startup_effort_and_recurring_permissions_are_candidate_specific`. Permission precedence: `every_tactic_obeys_explicit_off_and_on_without_forcing_a_winner`, `legacy_lists_cannot_reenable_off_through_another_producer`, `hard_constraints_precede_size_and_explicit_tactic_permissions`. The receipt runs all 75 policy, configuration and output-budget tests together.

The one open item is inherited, not new: frontend allocation coverage is partial, and the plan already assigns the remaining public-phase integration to 005 and 011.

## 004 Semantic Facts and Checked Edits

Contracts: A1-A3. Inspect `src/semantic_program/{mod,ids,storage,from_source,facts,publication,publication_edits,verify,uses,uses_update}.rs` and checked frontend interfaces.

Establish checked ownership of stable units, values, mutable places/cells, fields, calls, captures, types, module order and public obligations. Transport frontend knowledge once with diagnostic spans. Share unchanged units/tables; begin with measured unit-level copying and bounded caches.

Facts are revision/contract-qualified `Known`, `Unknown` or `Truncated`. Keep bounded symbolic/exact knowledge independent of literal emission. Distinguish reads, writes, allocation/identity, throws, divergence, reentry, suspension and control transfer. Discard, duplicate, move and speculate are separate queries; no-write alone proves none of them.

One atomic edit owner checks expected revisions and rule preconditions, updates uses/captures/call/module indexes and invalidates dependent facts, including negative-use and caller dependencies. Distinguish meaning-changing `SourceChange` from `EquivalentRewrite`; well-typedness and fresh facts do not prove equivalence. Different source meanings cannot compete in one portfolio. Include analysis bounds in cache qualification so stronger warm analysis cannot silently change deterministic low-effort behavior.

**Exit:** a real checked-source consumer, preserved old candidates and incremental/full-recomputed agreement after local/cross-unit edits. Changing return 7 to return 9 is a valid source edit but an invalid equivalence claim. Reject stale proofs, invalid operations/indexes and budget-refused batches without partial publication. Test getter/coercion interference, throws/divergence, identity-sensitive duplication and bounded huge strings. Counters show unaffected-unit reuse and explain global invalidation. Cheap dead-value/control cleanup uses these APIs.

Bounded task 004-P1 is implemented and verified in the [760-check receipt](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#checked-rewrites-and-factory-finalization), not milestone completion. One literal-integer binary-fold rule uses the existing atomic edit transaction and private checked evidence. Exact snapshots remain distinct from checked meaning classes; only accepted equivalent rewrites preserve meaning. Complete rewrite/tactic lineage survives JS/native admission and later source edits, whose inherited history is not replay proof for the new meaning. Twelve focused tests reject wrong constants, annotated raw JS operands, foreign/stale proofs, forbidden policy and resource-refused publication without partial changes; they use the existing primitive evaluator, independent JS/native observations and incremental/full-use-index comparison. Receipts include proving work and new lineage storage. No broader folding, cleanup, replay engine or language expansion is authorized by this child task.

Bounded task 004-P2 is verified in the [63-check multi-unit receipt](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#multi-unit-atomic-batches). Existing `publication::edits` tests now cover two-unit fork/unique/shared advance, independent rebuilt indexes and observations, unaffected cached facts, retained old artifacts, an invalid second replacement after both swaps, and calibrated resource/unwind rollback without partial publication. Production transaction and API remain unchanged. The existing qualifier's explicit `--test-filter=<Rust fragment>` mode builds once and runs only affected cohorts, with pinned inputs and 600-second command bounds. A zero-match filter correctly failed; correcting its module path reused the same binary. No unrelated CLI, codec or library sweeps were rerun, and no new production-feature or milestone credit is claimed.

### 004 The first real consumer: checked dead-value cleanup

004's exit asks for "a real checked-source consumer" and that "cheap dead-value/control cleanup uses these APIs". Until now the checked-edit machinery had one rule, a literal-integer fold proven on test fixtures. It now has a second rule, and the lineage it writes into is general.

**`Compilation::drop_dead_value`** retires one operation at a time through the existing atomic edit transaction. Its proof is bound to the snapshot, meaning, table and unit revisions and the policy fingerprint; commit re-derives it and refuses anything that moved. The rule applies when:

* the operation is a value producer (arithmetic, unary, copy, load, intrinsic, select) — never a call, store, allocation or control operation, whatever its effect summary says;
* its single result has **no use**, *or* every use is the `Initialize` of a **write-only local cell** (local, never read, referenced, captured, bound as a parameter or catch variable, or exported). The probe that motivated this: every LilScript local lives in a cell, so a dead local's value is "used" by its cell's initialization and the no-use rule alone never fires;
* the local facts say `can_drop` for a discarded result: no throw, divergence, reentry, suspension, control transfer or write;
* its type has a neutral constant (`0`, `+0`, `false`, `null`, `undefined`), so the verifier's type check still holds.

The operation becomes that constant with no operands, which frees its operands, so cleanup **cascades** one checked, individually published step at a time. On `int a=3;int b=a+4;int c=b*5;return a;` the multiply, the load of `b` and the add all retire, `a` is untouched, the incremental use index agrees with a full rebuild, the original snapshot survives unchanged, and the rendered function still returns 3.

Two facts refinements made that possible, both sound and linear:

* **Primitive locals.** A load from a never-reassigned local this unit initializes is primitive when every initializer is — previously every local read counted as possibly non-primitive, so any arithmetic on a local was summarized as a coercion. A unit that passes storage by reference anywhere gets no upgrade.
* **Initialization order.** A local read that follows its cell's `Initialize` **in the same region** cannot hit the temporal dead zone, because a region runs in list order and control only moves forward or leaves. Reads in nested regions or other units keep the conservative may-throw summary.

**Lineage is general now.** `CheckedRewriteStep` carries a `RewriteRule` (literal fold or dead-value drop), and each lineage node accumulates exactly the tactics its history used. Previously `tactics()` reported `ConstantFolding` for *any* non-empty history, which would have made a policy forbidding folding reject artifacts that never folded, and a policy forbidding dead-code elimination accept ones that used it.

**Exit, accounted for.** Real consumer: dead-value cleanup and literal folding. Old candidates preserved and incremental/full agreement: the cascade test checks both. `return 7` → `return 9` as a source change but not an equivalence: the existing P1 test. Stale and corrupted proofs refused without partial publication: four corruptions, none published, retained bytes unchanged. Getter/coercion interference: an exported `int` parameter can carry a `valueOf` hook, and an unused `value*3` on it is kept — the rendered artifact still calls the hook. Throws/divergence: an unproved local read keeps its may-throw summary and is not droppable; calls stay divergent. Identity-sensitive duplication and bounded strings: the existing facts tests. Unaffected-unit reuse counters: the existing publication receipts. Policy: the rule needs `dead-code-elimination`, records only that tactic, and a later rule under a policy forbidding it is refused. **Not done:** *control* cleanup (dead branches, unreachable regions) — assigned to 009 with the other families.

### 002 follow-up: public callable kind follows the source

Fixing the export spelling in 002 first forced every export to `function`. The new backend's tests showed why that was wrong: `export auto callback=()=>9` is an **arrow** in the source, and making it constructible would itself change the API. The new backend already had the right rule — a declaration becomes a `function`, a closure stays an arrow — whenever the contract carries no public spelling override. The contract now never carries one: `function_spelling` governs private functions only (in the new backend, private spelling becomes a searched choice in 009/010 rather than a knob). A side effect worth recording: an exported closure that reads module `arguments` used to be *refused* when the knob forced `function` spelling onto it; it now stays an arrow and compiles, still reading the global lazily on every call.

## 005 Public Fast JS and Native Service

Contracts: A1/A5/A6/A7. Inspect semantic JS/native/artifact/module owners, `src/structured_js/{mod,lower,verify,print,naming}.rs` and public compiler/library routing.

Build one production service consuming checked meaning and resolved policy. Public source/path entrypoints own discovery-to-output resources and call that service. Make the new route explicitly selectable during migration; examples become clients. Support the declared integrated module/function/closure/struct/reference slice, a real public adapter and portable native consumer. Unsupported forms are explicit 007 work.

Produce a deterministic direct artifact with useful cheap cleanup, binding-aware names, legal grammar, required helpers and complete packaging, without optional search or generated-text semantic recovery. Direct/edit outputs share the final admission authority used by search/replay; native uses corresponding ABI/ownership checks. Reserve mandatory work and return source/policy/recipe/artifact identity and phase/resource counters.

**Exit:** public CLI/library execution of the same slice, independent JS parsing and native separate-host/header execution, preserving copies, explicit mutation, captures, errors and initialization. Demonstrate admission parity, family vetoes, mandatory output with optional families off, tiny-budget behavior and clean cancellation. Measure frontend-inclusive time to first valid artifact and RSS. This establishes a baseline, not a speed claim. No example-only bypass or separate fast-mode optimizer qualifies.

Bounded implementation: `compile_source_semantic` and `compile_path_semantic` consume checked meaning through existing compilation/search/artifact owners; CLI `--backend semantic` selects this route explicitly. The default remains legacy. Unsupported conversion/delivery/profile cases diagnose without fallback. Native-only requests do no JS codec work; optional-off JS follows the same baseline and final admission path. Source/module observations, independent codec replay and exact stdout/file handoff pass scoped qualification. A resource ledger exists before discovery/parse, with source-buffer, parser-arena and verifier admission; reports enumerate remaining frontend gaps. Public source/path factories now qualify two native callback fixtures through separate-host/header execution; broader ABI coverage and complete resource admission remain open.

Scoped verification passed in the shared artifact/service receipt. `with_checked_source` and `with_checked_path` give the semantic service and migrated example one checked frontend and compilation owner. Direct, searched, retained/advancing-edit and portable native modes execute unchanged original consumers and independently replay exact codec scores; complete descriptor details survive handoff. Native records remain admitted while JS searches. Checker/AST scratch releases before adoption. Factory-owned finalization releases the compilation before owned path source buffers and their charges, then reports, including callback errors and resumed panics; caller-owned source strings remain explicitly separate. Effective policy/API caps determine per-attempt work and cache partitions, including a successful 1 MB raw helper-search regression. Changed example timing scopes are explicit. Remaining frontend allocations and public consumers still need migration; no release-speed claim follows.

### 005 closure: the measured baseline

Every exit item now has current evidence on this host. The public route is `compile_source_semantic`/`compile_path_semantic` in [compiler_service.rs](../../src/compiler_service.rs) and `lilscript --backend semantic` on the CLI; the legacy route stays the default until 011. The [full library suite receipt](../../benchmarks/migration-results/2026-09-21-receipts-4/full-library-suite/receipt.json) passes 2,970 of 2,970 tests, one ignored. It was first 2,959 of 2,959; the receipt is re-run as later milestones add tests. That covers admission parity, family vetoes, mandatory output with optional families off, tiny budgets, cancellation, and independent JS parsing and execution. It also covers native separate-host/header execution: GCC and Clang at O0, O2 and UBSan, with every translation unit compiled separately. Clang comes from a user-space extraction of Ubuntu's clang-18 packages in `~/toolchains/clang-18` (nothing installed system-wide). The receipt records `LILSCRIPT_NATIVE_CLANG` and pins the Clang binary as an input.

The [baseline receipt](../../benchmarks/migration-results/2026-09-21-semantic-service-baseline/receipt.json) comes from [semantic-service-baseline.mjs](../../finer/tools/semantic-service-baseline.mjs). It runs every `tests/cases` program the semantic route compiles through the release CLI: development mode, one thread, one discarded warmup, five samples, median. The compiler binary is preserved by content.

| Route | Cases | First valid artifact, ms (median / p95) | Process wall, ms (median / p95) | Peak RSS, MB (median / max) |
|---|---:|---:|---:|---:|
| JavaScript module | 38 | 3.069 / 5.814 | 12.048 / 14.876 | 11.133 / 11.543 |
| Native C | 22 | not reported by the C route | 8.262 / 11.325 | 9.035 / 9.410 |

"First valid artifact" is the compiler's own `first_artifact_ns`: discovery, parse, check, lowering, and admission of the first artifact. Process wall adds process start, output and exit. This is a baseline for 012, not a speed claim, and these are small programs. The same host is a burstable VM, so absolute numbers only compare within one session.

Out of scope for 005, with owners: an exported function whose signature carries a value struct is refused with `public value-struct ABI adaptation`, and 006 owns that adapter under D2. Frontend allocation owners the ledger does not yet admit move to 011. Broader native ABI coverage beyond the declared slice moves to 007.

## 006 Integrated Architecture Proof

Contracts: A1-A7. Consume 004/005 interfaces and existing family/search/target mechanisms only where they fit. **Stop expansion if this architecture does not fit.**

Through the public service compile one original multi-module program combining value structs, mutable-reference helpers, shared mutable closure state, a throwing/reentrant path, an observable public adapter and a computed string. Supply a portable native consumer and maintained-library slices with original callers.

Exercise compatible object/scalar, shared/inline helper and computation/literal/shared-data choices. Update producers, consumers, captures and adapters together. Score complete combinations after naming/packaging; retain independent raw/gzip/Brotli winners. A tiny exhaustive space checks compatibility and exact selection independently. Include a pinned interaction where a locally worse alternative must survive to find a better combination; every family need not win.

Edit one unit and compare reused and fresh compilation. Test older candidates, stale evidence, conflicting layouts, public names, identity, thrown discarded computations, coercion hooks and escaped references. Exercise family vetoes, runtime admission, tiny-budget incumbent fallback and atomic refusal.

**Exit:** JS/native observations, exact score replay, a scored combined candidate and bounded work/storage receipts through public APIs. Compare bookkeeping/reuse cost with simpler ownership or recomputation under the same rules. A reviewer traces concrete owners. Duplicate facts, special drivers, eager constant commitment or admission bypasses fail the gate. Revise the relevant design contract and this section before expansion; added compatibility glue does not pass. Record the slice's limits.

The [public-factory integration witness](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#public-factory-integration) passes 89 affected checks with unchanged production code. Two existing cross-family recipes in three naming styles receive common retained-artifact admission, complete descriptors, required-family vetoes, independent codecs and original observations after selected candidate disposal. With scoped names, additional helper inlining improves raw/gzip but changes Brotli from 3,686 to 3,695 bytes. This is a finite manual comparison, not automatic optimal selection. Two original native callback fixtures pass public source/path factories, qualified C/header handoff and sixteen separate-TU compiler/sanitizer executions. Factory ledgers finish at zero. The original twelve-map inspection test remains separate evidence. General public value-struct adapters and level-16 runtime permission still depend on D2/D5; complete milestone prerequisites remain open.

The [repeated ownership comparison](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#repeated-ownership-comparison) is verified against the preserved 1,065-check driver: five alternating pairs, sixteen edits each, a retained first branch, fifty artifact executions and fifty codec replays. Exact deliveries and original observations match. Retain/advance post-first-fork edit-plus-release medians are 2.050/1.421 ms; all-edit copied payload is 8,128/1,016 bytes. The advancing arm reuses fourteen edited units. Whole-driver RSS and wall medians do not improve. This is an existing ownership-policy comparison with all samples retained, not fresh recomputation or release performance; it supports the scoped unit-reuse mechanism without settling every cost gate.

### 006-P1 Exact-Score Reuse

Status: implemented and independently reviewed; focused verification passed, not milestone completion. Owner: existing `ArtifactArena` and semantic search portfolio. Scope: `src/semantic_program/{artifacts,artifact_reuse_tests,search_selection,search_score_reuse_tests,search_tests}.rs` and the schedule identity in `src/compilation_policy.rs`. No public-boundary decision or new optimization family is needed.

Reuse requested exact codec coordinates across already retained artifacts only after primary bytes and every dependency stream match, preserving file boundaries. Keep distinct recipes, provenance, legality checks and continuation cursors. Charge lookup/comparison work; add no unbounded score history. Do reuse before checking missing-codec probe allowance. This does not authorize new renders after the existing probe preflight stops discovery.

Prerequisite replay: the source-qualified September 18 debug test binary matches 976 recorded compiler/build/fixture inputs before edits. Its twelve-state oracle and five staged tests pass. Factory winners are raw/gzip/Brotli 205/159/124 bytes with 36 renders and 70 optional package-codec probes. The four-point interaction is 143/144/144/142 Brotli bytes; it does not prove traversal through a losing structural parent.

Assertions: equal complete bytes reuse only completed requested coordinates; different dependencies or file partitions do not reuse; exhausted-probe staged reuse preserves per-recipe admission; budget refusal leaves incumbents and resource cleanup sound. Run owner tests, affected search scheduling/resource/provenance tests and the same baseline oracle, not the full fleet. Reuse `target/qualification-internal-provenance-20260918-v3` for one incremental `cargo test --lib --no-run` build, then execute focused filters. Record exact source/build identities, commands and results under `benchmarks/migration-results/2026-09-19-exact-score-reuse/` before marking this task verified. Debug trial times are not release-speed evidence.

Result: [receipt and limitations](../../benchmarks/migration-results/2026-09-19-exact-score-reuse/README.md), input digest `7c74c3def9cfb3cb5a6d9ad4ba994c499aee3357ff87fe0e03455f02ba333013`. All 92 focused tests pass on Node 24.11.1; six are new. Global/Scoped naming trials with equal output share gzip/Brotli scores while a distinct Source trial is measured separately: three trials need two optional probes. All 16 paired existing schedule cases retain identical winner bytes. The twelve-state fixture has distinct output, so its 70 probes and 205/159/124-byte winners are unchanged; full staged lookup adds 1,115 logical work units. No general speedup is established. Schedule identity is now 5. No compiler-design contract changed.

### 006-P2 Structural Interaction Regression

Implemented and verified in the [shared artifact/service receipt](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md). The checked-source `search-structural-valley` fixture has three independent helper choices and an exhaustive eight-subset, three-naming oracle. Direct Brotli is 162 bytes; two individual choices are 175 and 187, their union is 161, and all three together reach 143. The real bounded search with beam width 2, 24 proposals and 48 optional probes reaches the union and 143-byte artifact despite evictions. Width 1 misses the combination and retains 162. All observed outputs execute the original public calls, including name/arity, coercion order and throws. This proves one finite loss-crossing interaction, not global optimality or the complete integrated architecture gate; it adds no package-specific scheduling rule.

### 006-P3 Finite Neighborhood Diagnostics

The [joint-string-pool oracle](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#joint-string-pool-oracle) is verified in the 897-check receipt: six existing manual recipes times three naming styles produce 18 commonly qualified outputs, each independently executed and rescored. Automatic inventory exposes only singleton groups, while the existing family proves a joint group. Computed / literal / separate-pool / joint-pool style minima are respectively 147/117/85, 139/114/85, 155/123/91 and 119/115/98 raw/gzip/Brotli bytes. The joint raw win is a Brotli loss; neither tested pooling combination proves a Brotli valley. No automatic heuristic changed. Keep this negative result when judging grouped-discovery cost and priority.

The [24-output source-name experiment](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#source-name-neighborhood-oracle) is verified in the 1,036-check receipt: two eligible overrides, three existing styles and two structural recipes. Binding identities are obtained anew for each prepared structure; common qualification, independent codecs and Node observations cover every output. Overrides produce five distinct new byte streams per structure, but none beats that structure's existing style minima in raw, gzip or Brotli. This is a negative finite result, not proof against naming interactions elsewhere. Production heuristics remain unchanged; require a reusable positive witness and bounded cost evidence before expanding this neighborhood by default.

### 006-P4 Search Schedule Ablation

The [32-row finite ablation](../../benchmarks/migration-results/2026-09-20-search-schedule/README.md) compares beam widths 1/2, immediate/staged scoring, Brotli-only/all-codec requests and four fixed probe ceilings on the existing three-helper interaction fixture. The independent 24-output oracle and 64 winner executions preserve original observations; separate canonical codecs replay every unique artifact. At beam 2 and eight optional probes, staged scoring reaches 151 Brotli bytes versus immediate scoring's 162, but renders 16 artifacts versus 9 and spends 67,053 versus 34,188 optional logical work units. Both schedules reach the 143-byte finite optimum with 17 probes at higher ceilings. All width-one rows remain at 162; spending more probes cannot recover evicted combinations. All-codec requests retain independent winners and reach finite minima 243/167/143 at ceiling 24. These are useful quality/cost tradeoffs, not wall/CPU or fleet measurements. No production heuristic, default or schedule changes.

Four focused tests pass after a preserved zero-match filter refusal; the corrected filter and diagnostic replay reuse the same cached binary. Root's audit verifies 980 input pins, all 32 rows and 30 diagnostic outputs; no independent agent review. Only the test file changes from C8. The next scheduling work must respect emission cost rather than treating spare codec probes as the sole budget. Current inventories and proof seeds remain snapshot-local: equivalent-rewrite rediscovery needs revision-qualified proofs and explicit rebasing through the same owners, not a rescan that mixes old and new snapshots. This result does not waive 006's remaining prerequisites or implement rediscovery.

### 006-P5 Search Reclamation Admission

The [reclamation receipt](../../benchmarks/migration-results/2026-09-20-search-reclamation/README.md) replaces upfront states-by-artifact-capacity work reservations with admitted visits inside the existing state and entry owners. Pin lookup charges physical slots, including holes, and stops at the first match; active/empty states avoid unnecessary pin scans. No new cache, pin-count table, allocation, representation or search default is added. Schedule 22 identifies changed accounting. Four new tests exercise exact scan cutoffs and partial cleanup, preserve pinned/unvisited candidates and exact incumbents, and finish with zero retained bytes. There are 135 distinct passing affected checks; a zero-match filter refusal is preserved and corrected using the same cached binary. Only cursor test assertions/formatting change between the two compiled attempts, so 105 unaffected passes are reused.

The paired 32-row replay preserves all oracle records, winner bytes/scores/observations, explored combinations, probes, renders, other work kinds, peak memory and stop reasons. Only optional/analysis work changes: 0-667 removed tariff units per row, positive in 28 rows. Matching bytes and scores reuse the pinned independent canonical codec replay; codecs are not rerun. Root's audit verifies 980 final inputs and 70 qualification/replay outputs; no independent agent review. This is budget precision, not a compression, asymptotic or wall-time improvement. The input digest is `728ed9ca7f4aeedc901844d91385cb6c79d371e3d920a34cbb4703387e93adb6`. No release/library/fleet rebuild, rediscovery implementation or milestone closure follows.

### 006 closure: the integrated proof on settled contracts

Two things held 006 open: general public value-struct adapters waited on D2, and the level-16 runtime permission waited on D5. Both decisions are settled and both are now implemented on the semantic route.

**D5, level 16.** The semantic route has no permission logic of its own. Artifact provenance admits through the one `ResolvedPolicy::check_tactic_permissions`, so the documented grant reaches it unchanged. `level_16_grants_declared_startup_risk_and_a_tactic_override_still_wins` ([artifact_provenance.rs](../../src/semantic_program/artifact_provenance.rs)) pins all four cases. Level 15 refuses a startup-risk tactic, and level 16 admits it. `[policy.tactics]` overrides the level in both directions: `'on'` admits at 15, and `'off'` refuses at 16.

**D2, public value structs.** An exported function whose signature carries a value struct used to be refused with `public value-struct ABI adaptation`. It is now published through one wrapper ([javascript_public_structs.rs](../../src/semantic_program/javascript_public_structs.rs)). The wrapper keeps the source name, arity and callable kind. It reads each incoming field once, depth first, into the private positional form; calls the private function; and returns a fresh plain object. The contract is written into [D2 for value structs](../compiler-design.md#d2-for-value-structs). The wrapper is formed inside the one `Formation`, so direct, searched and edited candidates all carry it and pass the same verification and admission. It adds no fact store, driver or admission path. The private function's name becomes unobservable and is dropped. Shapes a copy cannot preserve stay refused by name, and those move to 007: a struct inside a collection, callable, union or nullable; a struct parameter with a default; and a body that observes `this` or `arguments`.

| Evidence | Where |
|---|---|
| Adapter semantics under both output modes: keys, prototype, freshness, one identity for two export names, `name`/`length`, getter read order `p.x p.y a a.x a.y b b.x b.y label`, caller mutation not observed, `TypeError` on `null`, `new` | `exported_value_struct_functions_publish_one_object_adapter_with_source_reflection` |
| Refusals: collection, callable, nullable, observed receiver | `a_public_struct_whose_copy_would_lose_identity_or_frame_stays_refused` |
| Public service with search, all three codec winners executed and independently re-scored | `source_service_publishes_value_struct_functions_through_the_d2_adapter` |
| Public path service, a struct function re-exported under an alias from another module (`move.name === "translate"`) | `path_service_publishes_re_exported_value_struct_functions_with_source_names` |
| Edit one unit: reused against fresh compilation under one ledger | `integrated_edit_reuse_and_fresh_recomputation_under_the_same_rules` |
| Everything above plus the original twelve-module fixture, native entry, family vetoes, runtime admission, tiny budgets and atomic refusal | [full library suite receipt](../../benchmarks/migration-results/2026-09-21-receipts-4/full-library-suite/receipt.json) |

Earlier slices supply the remaining exit items. Exact score replay is 006-P1. A scored combined candidate is the [public-factory integration witness](../../benchmarks/migration-results/2026-09-19-artifact-service/README.md#public-factory-integration). The pinned loss-crossing interaction is 006-P2. Bounded search storage is 006-P5.

**Reuse against recomputation**, from the [receipt](../../benchmarks/migration-results/2026-09-21-receipts-4/006-reuse-versus-fresh/receipt.json). The twelve-module fixture has 44 units; one `return 11` becomes `return 12`, compiled both ways under one ledger. The two arms deliver identical bytes.

| Counter | Fresh | Reused |
|---|---:|---:|
| Fact cache hits | 0 / 44 | 43 / 44 |
| Fact steps physically executed | 6,746 | 10 |
| Fact work billed | 6,746 | 6,746 |
| Adoption or edit work | adoption, in total below | 1,320 |
| Total logical work to the delivered artifact | 230,587 | 215,359 |
| Retained bytes before release | 4,364,818 | 4,375,838 (history kept) |

A cache hit is billed its recorded logical work on purpose, so budgets and outcomes never depend on cache state; the saving is physical. Reuse removes almost all fact execution and the whole frontend pass: parse, check and conversion took about 3 ms, not billed to this ledger. It pays 11,020 bytes to keep the previous snapshot. Formation still reruns in full on both arms, so logical work falls only 6.6%. Incremental formation is the next lever and is not built.

**Concrete owners**, for the reviewer trace:

| Concern | Owner |
|---|---|
| Checked meaning | `Program` in [semantic_program/mod.rs](../../src/semantic_program/mod.rs), built by [from_source.rs](../../src/semantic_program/from_source.rs) |
| Resolved policy and permissions | `ResolvedPolicy` in [compilation_policy.rs](../../src/compilation_policy.rs), resolved once from [config.rs](../../src/config.rs) |
| Work and memory | `BudgetLedger` ([compilation_policy.rs](../../src/compilation_policy.rs)) and `AllocationBudget` ([output_budget.rs](../../src/output_budget.rs)) |
| Snapshots, edits, checked rewrites, lineage | `Compilation` in [publication.rs](../../src/semantic_program/publication.rs), [publication_edits.rs](../../src/semantic_program/publication_edits.rs), [publication_rewrites.rs](../../src/semantic_program/publication_rewrites.rs), [rewrite_lineage.rs](../../src/semantic_program/rewrite_lineage.rs) |
| Facts and uses | [facts.rs](../../src/semantic_program/facts.rs) (session, cache, billing) and [uses.rs](../../src/semantic_program/uses.rs) |
| What the artifact must compute | `DemandPlan` in [demand.rs](../../src/semantic_program/demand.rs) |
| Representation families | [record_family.rs](../../src/semantic_program/record_family.rs), [string_family.rs](../../src/semantic_program/string_family.rs), [product_family.rs](../../src/semantic_program/product_family.rs) with [function_layout.rs](../../src/semantic_program/function_layout.rs), [helper_family.rs](../../src/semantic_program/helper_family.rs), [value_placement.rs](../../src/semantic_program/value_placement.rs) |
| JavaScript formation and public adapters | `Formation` in [javascript.rs](../../src/semantic_program/javascript.rs), [javascript_structs.rs](../../src/semantic_program/javascript_structs.rs), [javascript_public_structs.rs](../../src/semantic_program/javascript_public_structs.rs), [javascript_struct_boundaries.rs](../../src/semantic_program/javascript_struct_boundaries.rs) |
| Target syntax, names, printing, verification | [structured_js](../../src/structured_js/mod.rs): `naming.rs`, `print.rs`, `verify.rs`, `extract.rs` |
| Artifacts, admission, provenance | `ArtifactArena` in [artifacts.rs](../../src/semantic_program/artifacts.rs), [artifact_provenance.rs](../../src/semantic_program/artifact_provenance.rs) |
| Search | [search.rs](../../src/semantic_program/search.rs), [search_selection.rs](../../src/semantic_program/search_selection.rs), [search_entries.rs](../../src/semantic_program/search_entries.rs) |
| Native | [native.rs](../../src/semantic_program/native.rs), [native_plan.rs](../../src/semantic_program/native_plan.rs), [artifact_native.rs](../../src/semantic_program/artifact_native.rs) |
| Public entry | `compile_source_semantic`/`compile_path_semantic` in [compiler_service.rs](../../src/compiler_service.rs); `lilscript --backend semantic` |

**Limits of this slice.**
- Default routing stays legacy until 011.
- The adapter costs bytes. The three-function probe is 375 raw / 177 Brotli bytes on the semantic route against legacy's 145/83. Most of the gap is the semantic route's general direct-artifact scaffolding (`let d;…d=function…;let a=d`, spaced operands), which 008 cleanup owns. The wrapper-to-private call is not inlined because helper inlining sees semantic units, not target wrappers; 009 owns that.
- Nullable and generic structs do not convert to the semantic program at all yet, and the refused boundary shapes above are also unimplemented; both are 007 coverage.
- Formation is not incremental.
- ~~The four D3 clauses without cases (D3.6-D3.10) remain 007 work, as recorded under 002.~~ Done under 007.

## 007 Language and Port Coverage

Contracts: A1-A3/A5, settled D1-D3. Own 001's semantic/target-operation rows; 008 owns delivery and 011 their full-suite combinations.

Complete finite gap groups in checked-source and JS/native owners: calls/defaults/order, classes/nominals/inheritance, generics/nullables/containers, closures/captures/references, control/completion/suspension and intrinsics/host operations. Unsupported source cannot be patched in printers. Arbitrary JS calls remain conservative.

Migrate accidental struct aliasing to explicit references/value transfers with unchanged public expectations, including Motion mutation and Micromark snapshots. Complete exported value-struct adapters, stable keys, untrusted inputs and layout choices. Native covers escaped/shared captures, sibling cells, callback retention/reentry, headers, ownership and cleanup; record genuine host-specific exclusions.

**Exit:** public-route positive/negative tests for every owned row, JS differential observations and appropriate separate-host/native sanitizer checks. Include overlapping references, parent replacement, nullable/generic copies, defaults, callee snapshots and retained callbacks. Run changed libraries' original cases under supported delivery. Cases awaiting 008 become required 011 cells without dropping tests or creating a circular gate. No scope reduction closes a gap without an explicit decision.

### 007 progress: the semantic route learns the rest of the language

This records work in order; the milestone stays open. Each gap is closed through every owner: checker, conversion, verifier, facts and JS formation. Nothing is patched in a printer. The [census](../../finer/tools/semantic-census.mjs) now also measures `js-module` alongside `js` (script) and `c`.

| Gap group | What changed | Census cases unblocked |
|---|---|---|
| Intrinsics | The structured target carries the legacy emitter's spelling for array, string, map/set, typed-array, regex and buffer methods and properties. `Math.*`, `Object.*`, `JSON.*` and `Promise.*` become host member calls; `toInt` is `ToInt32`; `toUnsignedString` is `(x>>>0).toString(r)`; `Map.get` is `??null`; `pop` applies the element type's absent value. Every `int` result is normalized, because host builtins may be patched. | array mutation, strings, float, slice, bitwise, scalar math |
| Callback methods | The checker records the signature each `map`/`filter`/`reduce`/`forEach`/`some`/`every`/`findIndex` call was checked against, so conversion reads one checked contract. | map, filter/reduce, forEach capture, iteration snapshot |
| Builtin constructors | `new Map/Set/ArrayBuffer/SharedArrayBuffer/<TypedArray>/Symbol` verify against their checked shape (operands and result), since generic results have no static contract. Formation uses the native constructor, or calls `Symbol(...)`. | typed arrays, float32, unions, collection elision, symbol keys |
| Templates | New `Template` operation: template-rule string conversion of each operand, left to right. A `JsValue` substitution is converted as soon as it is evaluated, preserving JavaScript's interleaving. Printed as one template literal. | templates |
| Type tests | New `TypeTest(T)`: `typeof` for number/string/boolean/function and `Array.isArray` for arrays, exactly the legacy runtime categories. | type guards |
| `for...of` | An index loop over two synthetic cells. The verifier now allows synthetic cells after the symbol-backed ones. `inline for` uses the same path: unrolling is an optimization, not a meaning. | inline for |
| Parameter defaults | Callers evaluate an omitted default after every supplied argument. The verifier checks each such argument is exactly its parameter's checked default, so a forged count cannot relabel a real argument. The body also guards `undefined` (new `IsUndefined`, never true natively) for host or erased callers. An exported function with a default is refused as `public parameter default reflection` until printed callables carry default syntax, because its `length` would differ. | optional comparators |

Census after this group ([receipt](../../benchmarks/migration-results/2026-09-21-semantic-census-007a/receipt.json)): script JS 35 to 54 passed; module JS 57 passed; native C unchanged at 22. There are zero miscompiles. The full library suite passes after the listed test updates: each refusal test that became supported now asserts the positive behavior, and its negative case moved to a feature that is still refused.

**Classes** landed next, reusing constructs every analysis already treats as references:
- An internal class instance is an `Object` allocation holding every field at its legacy default (base fields first), and a field is a named `Member` place. Struct-style value paths were rejected for this: value-semantics analyses would then treat aliased writes as unaliased.
- Methods and `init` are function units that take the instance first, reached through synthetic function cells and called statically; the checker rejects overriding. `super(...)` calls the base `init`.
- Generic classes compile their bodies as generic functions, and each call instantiates them from the receiver's type arguments.
- The semantic program gained a class table. The verifier checks instance allocations against it and uses the checker's class-hierarchy assignability for upcasts. The module checker now defines classes and enums (a class-free graph skips the pass, keeping its accounting unchanged); extern classes stay refused.
- Struct, class and arrow parameter defaults work. Arrow defaults are applied only by the guarded callee, because converting one arrow at every call site would redeclare its parameters.
- An internal instance never reaches host code. Formation refuses any value carrying one at a `JsValue` position, a host callee, `print`, `throw` or an export, until a real-class boundary representation exists. (Superseded below: host code sees the instance's data object, as on the legacy route.)
- In classic scripts, a struct-bearing frame is printed with a `"use strict"` directive instead of being refused, so a sloppy host sees `caller === null`. A frame that also observes `this`/`arguments` is still refused.

Two output-quality changes were validated against the search tests:
- The printer spells a literal string key as `o.name` / `{name:v}`, except `__proto__:`. The key stays a string occurrence in the IR, so the string family can still pool it.
- Class method names are unobservable and dropped.

Two search tests encoded properties that held only for the old spellings. With the shorter keys, a width-10 beam evicts one subtree holding a new one-byte gzip optimum; the oracle claim is now made with a space-covering beam of 24, which matches the oracle exactly (839/438/388), while even the default-beam best (839/439/388) beats the old exhaustive minima (843/443/390). The raw-loser precondition moved to a long-key variant of its fixture, where pooling wins raw and the repeated literal wins Brotli.

Census now ([receipt](../../benchmarks/migration-results/2026-09-21-semantic-census-007c/receipt.json)): **script JS 72 of 72, module JS 72 of 72**, native C 22 of 72, zero miscompiles. The full library suite passes 2,970 of 2,970.

### 007 progress: the maintained ports through the semantic route

Compiling each maintained port's own sources exposed the remaining gaps. Each was closed in the compiler through its owners, or, where a port relied on a legacy accident, migrated in the port under D1. The [port harness](../../finer/tools/semantic-port-tests.mjs) copies a port beside links to its siblings, applies its recorded migration from [`finer/port-migrations/`](../../finer/port-migrations), builds with `--backend semantic` and runs the port's own tests. Its report pins the compiler, each port's revision and dirty state, and each patch.

Receipt of record: [report](../../benchmarks/migration-results/2026-09-21-semantic-ports-007/report.json) (compiler `4c1cd87a`, Node 24). 18 of 25 ports pass their whole suite. 24 build on the semantic route; react-markdown's build stops at a sibling revision pin before it compiles. Each remaining failure is listed with its cause; none is a wrong answer from the semantic route.

| Port | Own tests on the semantic route | Migration | Note |
|---|---|---|---|
| micromark | 1,963/1,963 | [patch](../../finer/port-migrations/micromarklil.patch) |  |
| zod | 1,353/1,353 | none |  |
| mdast-util-from-markdown | 743/744 | [patch](../../finer/port-migrations/mdast-util-from-markdownlil.patch) | The failure is a site-size receipt: it compares committed legacy dist sizes, which a semantic build replaces. |
| remark | 504/504 | [patch](../../finer/port-migrations/remarklil.patch) |  |
| hast-util-to-html | 456/456 | none |  |
| unified | 224/224 | none |  |
| mdast-util-to-hast | 149/149 | none |  |
| rehype | 159/159 | [patch](../../finer/port-migrations/rehypelil.patch) |  |
| remark-math | 60/60 | none |  |
| marked | 28/29 | none | One closed-world case expects mangled option keys: property mangling is 008/009 work. |
| posthog | 21/21 | [patch](../../finer/port-migrations/posthoglil.patch) |  |
| remark-breaks | 20/20 | none |  |
| remark-gfm | 19/19 | none |  |
| remark-rehype | 18/18 | none |  |
| remark-parse | 15/16 | [patch](../../finer/port-migrations/remark-parselil.patch) | The failure is the same site-size receipt. |
| rehype-stringify | 10/10 | none |  |
| cn | 116,358/116,358 | [patch](../../finer/port-migrations/cnlil.patch) |  |
| katex | 1,251/1,251 | [patch](../../finer/port-migrations/katexlil.patch) |  |
| jquery | 0/1 | [patch](../../finer/port-migrations/jquerylil.patch) | The build compiles; the suite cannot load it: output imports the port's host module `./js-host.ts`, whose helpers the legacy route inlines. Delivering foreign host modules is 008. |
| monaco | no suite | none | The port has no test script; the build compiles. |
| rehype-katex | 63/63 | none |  |
| mobx | 766/780 | [patch](../../finer/port-migrations/mobxlil.patch) | 3 failures and 11 skipped: the failing cases load the production build through the port's jest mapper, which the harness does not set up. |
| probe | 1/1 | none | A differential build: it compiles, runs and compares its own output. |
| react-markdown | 1/8 | [patch](../../finer/port-migrations/react-markdownlil.patch) | The build refuses before compiling: it pins its sibling unifiedlil to a revision the working tree does not have. This is the environment, not the compiler. |
| playcanvas | 187/187 | [patch](../../finer/port-migrations/playcanvaslil.patch) | Its exported `BlockInfo[]` parameter uses the new read-only array adapter. |

**Language and ABI changes**, each through checker, conversion, verifier, analyses and formation:
- **Extern classes.** Module checking defines them; conversion records their fields; a member is an exact host property or host method call with unknown effects. Class, extern class and enum exports are type-only and resolve by name; a renamed one is refused for now.
- **`import extern`.** Module checking admits foreign modules (dynamic and lazy loaders stay refused). Each local must be an extern value declared in its module, as on the legacy route. The program records the import on its module interface, and formation emits an ES import, spelled from the root module's directory as the legacy linker spells it. A classic script refuses one.
- **Value structs in program collections and classes.** Arrays support indexing, `length`, `push`/`pop`/`slice`/`splice`/`concat`/`reverse`/`fill`/`copyWithin`, callbacks and spread. Maps may hold them as values, and classes as fields. A field update through an element, a class field or a nested field rebuilds the stored product from one evaluation of its array and index, which value placement captures. The host model is the one every JavaScript compiler and the legacy route assume: standard `Array.prototype` methods and no index accessors on built-in prototypes. Public boundaries stay exact through D2. Equality (`indexOf`/`includes`), Set elements and Map keys carrying products stay refused, since they would compare backing identity.
- **Struct to `JsValue` is the D2 public shape.** A single-use struct value reaching a `JsValue` position (a local, a store, an argument, a host builtin operand, a property of a host object) is formed through its D2 encoder: a fresh plain object, which is the value's own semantics. A function value is wrapped by a hoisted D2 callable adapter that decodes struct parameters and encodes struct and nullable-struct results. A union holding both `JsValue` and a struct stays refused, since no runtime test separates them; ports make that conversion explicit.
- **Class instances are data objects to host code**, as on the legacy route: fields are own properties under their declared names and methods are never members. An ordinary class may dissolve (language contract), exported or not.
- **Suspension.** `async` bodies and generators have a `Suspension` on their unit and target function. New `Await` and `Yield` operations and a `ForOf` loop over generators run as JavaScript `await`, `yield`/`yield*` and `for...of`. Analyses treat all three as unknown effects. Native targets refuse them.
- **Smaller items.** A projection through a `P?` place narrowed to `P` loads the narrowed value. A function held in a value struct is called without a receiver. A call through a value of a checked function type needs no ownership proof: host code cannot produce one with a product-bearing signature, and an escaping function keeps packed transport. An object property key converts once per access. `codePointLength` is `[...s].length`. An exported function with defaults keeps its JavaScript `length` (`p=void 0`). `init` returns `void`. A record literal typed by a `JsValue` context is a `Record<JsValue>`. Generic externs are admitted.
- **Checker.** A callback that returns `JsValue` on some paths is typed as returning `JsValue`, since falling off its end yields `undefined`. It had been typed `void` while still returning values to its host callers.
- **Miscompile found and fixed.** unified's semantic output printed `for(let a in a)`: scoped naming let a for-in key reuse the name of a binding its head reads, which throws in the head's TDZ. A for-in or for-of head now shares its binding's scope for naming. A regression test fails without the fix.
- **CLI budget.** The semantic route's default work ceiling is 4,000M units; Micromark's 303 KB of source uses 354M. The library default is unchanged, and 012 sets the policy.

**D1 port migrations** (recorded patches, applied only in the harness workspace):
- The micromark family (micromark, remark, mdast-util-from-markdown, remark-parse, react-markdown): the typed stack's length, and three token views (`PointView`, `TokenView`, `AttentionTokenView`) become extern classes, since they alias host token objects.
- cnlil, posthog, katex (2 declarations), jquery (17) and react-markdown: each module declares the host globals it reads; the legacy linker leaked externs across files.
- jquery: its `Support` bag and four `*View` structs become extern classes; one plain object is built with `JS.object`; two callbacks return an explicit `JsValue`.
- rehype: parse5 shares token and attribute objects, so `Attribute` and the five token structs become classes (88 lines).
- mobx: a config override to size-first. This is not a migration: semantic candidates carry no performance estimate yet, which its performance-first policy requires.

Size is not addressed here: semantic output is still larger (Micromark 34,949 vs 27,332 Brotli bytes, same sources). That is 008–010 work.

### 007 progress: native C covers the corpus

The native C target now compiles the whole language corpus. Census ([receipt](../../benchmarks/migration-results/2026-09-21-semantic-census-007d/receipt.json)): **script JS 72, module JS 72, native C 72 of 72**, zero miscompiles (native was 22). Every native program also runs clean under AddressSanitizer, UndefinedBehaviorSanitizer and LeakSanitizer (Clang 18, `-O1`, leak detection on; LeakSanitizer was first confirmed to catch a planted leak). 5,232 formatted floats, including every power of two from 2^1100 to the subnormals, print byte-identically to Node.

Each representation is chosen in the native plan and checked there; the C writer only spells it.
- **Arrays** are reference-counted and growable, one C type per interned element type, so arrays nest and hold callables, objects and tagged values. Stores acquire and overwrites release. Reads past the end follow the JavaScript recipes (`int` 0, `string` `""`, nullable null); any other would be `undefined`, so it stops the program. Every array method has its ECMA-262 order: a callback sees the length fixed at the start and skips indices a callback removed, except `findIndex`, which reads them.
- **Module bindings** that functions read become file-scope slots, guarded until their initializer runs (JavaScript would throw) and released when `main` returns.
- **Strings** made at run time live until execution ends, the lifetime callback ABI v1 already promises for every string payload; `main` frees them. Number to string is ECMA-262's shortest round-trip form, and `print` spells negative zero `-0`, as `console.log` does. Text is written as UTF-8 with lone surrogates replaced, as Node writes it.
- **Class instances** are reference-counted C structs. A derived class embeds its base first, so an upcast is no conversion. Every class slot is one C type; a member access casts to the class that declares the field.
- **Nullables, unions and type parameters** share one tagged value. Moving between a tagged slot and a typed one is a representation conversion that never changes ownership; unboxing checks the tag. A tagged value records the physical signature of any callable it carries. A callable passed to a parameter of another signature travels in an adapter that the caller owns for the call. The main case is a concrete callback reaching a generic `func(T)->T`. Mismatched signatures anywhere else are refused at compile time: this closed a wrong answer found on the way, where a callable boxed at `(string,string)->bool` was called as `(T,T)->bool`.
- **Maps and sets** are insertion-ordered hash tables over tagged keys with SameValueZero equality. **Symbols** are identities.
- **Typed arrays** are views over reference-counted buffers. Writes wrap, clamp or round to binary32 by kind; writes past the end are ignored. A union of one array kind and one typed kind indexes by its tag.
- Parameter defaults need no native ABI: callers evaluate them, except an arrow default, whose omission is an empty callable that the callee's guard replaces.

Recorded native exclusions, each refused at compile time or stopped with a message, never approximated:
- Regular expressions, and the string methods that take one.
- Exceptions (`try`/`throw`) and suspension (`async`, generators).
- Case mapping outside ASCII, which stops the program; full Unicode tables are recorded work.
- Reading an absent non-integer, non-string element (`undefined`).
- Adapting a callable anywhere but a call argument.
- A value struct owning a reference, and a product inside a tagged value.
- Reference cycles are not collected: reference counting is no cycle proof.
- Run-time strings are not reclaimed before execution ends. A loop that builds a long string therefore uses memory quadratic in its length. Reclaiming them needs callback ABI v2.
- Transcendental `Math` functions use the C library, which ECMA-262 permits ("implementation-approximated"): the last bit may differ from V8's.

Remaining 007 work, in order:
1. ~~Close the remaining port gaps~~: done except the cases above, each owned by a later milestone (008 delivery and property mangling) or by the port environment.
2. ~~Native C for the corpus groups~~: done, 72 of 72 with the exclusions above.
3. ~~Syntax not yet reached~~: done for `match`, optional chaining, object singletons holding products and destructuring (below). **Decision:** dynamic `import()` moves to 008. It needs lazy modules in the direct module graph, with namespace members resolved through the target module's interface rather than one merged scope. Those are 008's module and delivery contracts (deferred work, lazy chunks).
4. ~~Host-class inheritance and generic class inheritance~~: done (below).

### 007 progress: the last syntax and inheritance gaps

Each form is converted into existing semantic operations where they suffice. The two new operations are verified and form on both targets, or are refused natively by name.
- **`match`** is nested `Select`s over a scrutinee evaluated once. Each arm tests strict equality with its pattern (an enum discriminant, an integer, a string or a boolean), and the last arm, or `_`, is the checked fallback with no test. Only the selected arm runs. Enums are their discriminants natively.
- **Optional access** `a?.m` and `a?.[i]` evaluate the receiver once. When it is present, they read through the narrowed receiver, evaluating the index only then; otherwise the result is null. `??` gained its native form: the present left value unboxed as the result needs, or the lazily evaluated right.
- **Destructuring.** An array binding reads its position as `T?`, and a missing position is null on both targets. This fixed a latent difference: JavaScript had read a raw `undefined`, and native had read the element type's absent value. The rest is `slice` from its position. A record binding reads its key as `T?`. The record rest is a fresh null-prototype record filled by a `for…in` over the source that skips the named keys, which keeps JavaScript's own-key order.
- **Object literals holding value structs.** `object {…}` is a host object typed `JsValue`, so each entry is a `JsValue` position and a struct takes its D2 public shape: a fresh plain object per entry.
- **Generic class inheritance.** An inherited generic body sees the receiver as its own class: each class's parameters are substituted into its `extends` arguments up the chain. The class table records each class's type parameters and its base's arguments. The verifier admits upcasts of generic instances by the same substitution. Natively, a field's storage is always the declaring class's view, so a `T` field of a generic base is a tagged value that the subclass unboxes.
- **Host-class inheritance** (`class Problem extends Error`, direct extern base): the class stays a real JavaScript subclass, formed as `let a=class Problem extends Error{constructor(…){super(…);…}}`. The observable name stays exact while the binding can still be minified. Its `init` is the constructor unit. `new` is `ConstructClass`, whose first operand is the class value, so liveness sees the dependency. `super(...)` is `SuperConstruct`, after which the instance parameter is `this` and the class's own fields take their defaults, as JavaScript class fields do. The verifier checks both operations against the extern class's recorded host constructor. Host code observes the same instance as on the legacy route: `instanceof Error`, the class name, a real `stack`, and own keys. Native targets refuse it by name, as the language specifies. A host-derived class extending another host-derived class is refused for now.

Formation refusals report their span in the message but no rendered source diagnostic yet; that is 011 work. `LILSCRIPT_DEBUG_VERIFY` names the refused unit and prints the module index-to-path table.

### 007 closure

Evidence on the final 007 binary (`112bbc6a`): [census 007e](../../benchmarks/migration-results/2026-09-21-semantic-census-007e/receipt.json) passes script JS, module JS and native C 72 of 72 each, with zero miscompiles. The [port rerun](../../benchmarks/migration-results/2026-09-21-semantic-ports-007e/report.json) matches the receipt of record port for port. The full library suite passes 2,995 of 2,995 (1 ignored). Its native fixtures now also run under Clang AddressSanitizer with leak detection, beside O0, O2 and UBSan on GCC and Clang.

D3.6-D3.10, which 002 left without executable cases, each have a positive and a refusal case in `d3_clause_tests.rs`. They cover divergence (a diverging call with its result unused still diverges; recursion over constants is not evaluated while compiling), initialization order and early reads, `await` and generator interleavings (natively refused), script and module frames, and recursion depth (helper inlining refuses a recursive helper).

These cases become required 011 cells, each with the owner that closes it:
- jquery's compatibility suite: its host module `./js-host.ts` must be delivered (008).
- marked's closed-world case that expects mangled option keys (008/009 property naming).
- dynamic `import()` (008 lazy modules and delivery).
- mobx's three production-build cases, run through the port's jest mapper (harness environment; 011).
- react-markdown's sibling revision pin (port environment; 011).
- the two site-size receipts, which read committed legacy dist sizes (retired with the legacy route; 014).

## 008 Whole-Program JS and Delivery

Contracts: A3/A5/A6. Start with structured JS analysis/flow/rewrite/naming/printing and module/delivery contracts. Begin after 006; 007 is required for closure.

Root liveness in public entries and host effects. Preserve initialization, cycles, live bindings, deferred work and side-effect imports while eliminating unused modules/exports/values and safely hoisting scopes. Bindings and owned properties have identities and boundary constraints; access syntax alone grants no renaming permission.

Implement declaration, one-use, branch/sequence/logical, loop, boolean/numeric and precedence/token compaction through checked target edits. Preserve alternatives where shorter text hurts codecs. Finalize names, helpers, adapters and every supported single/preserve-modules/split/lazy mode in one delivery plan. Fixed primary-plus-one-resource output is not general chunk support. Chunk hashes/manifests must finalize deterministically and terminate.

**Exit:** independent parse/execution of emitted files, covering reflection, getters, callbacks, cycles, initialization and grammar floors. Exercise shared helpers/properties across real library and split boundaries. Deployment replay loads exactly the scored files and required resources/manifests. Score streams independently unless transport combines them. Artifacts are immutable; subsequent packaging changes require readmission/rescoring. No generated-text identity recovery returns.

### 008 progress: output compaction, batch 1

The semantic route ran no pass between formation and printing, and every value it could not fuse got a declared temporary. Batch 1 removes the waste where it is created:

- A total read of a cell that nothing writes after initialization is read again at each use instead of copied to a temporary. Demand already proves such a read cannot throw; a classic script's parameters qualify when no unit reads `arguments`.
- A single-use closure forms in place. Its consumer tree is capped at 16 target layers, so its body's entry depth is fixed before the tree exists; only a deferred closure pays that allowance.
- Private functions (unobserved name, never constructed, no own `this`/`arguments`) print as arrows, and a closure whose every use is a call keeps no exact name.
- Printing: adjacent `let`s share one declaration, single statements lose their braces, `else if` chains, `;` before `}` is implied, `a=>a*2` concise arrows, `==` between two numbers, strings or booleans, and no `|0` on intrinsics that are int32 by specification when the contract assumes pristine builtins.
- Inspection formation builds a use index, so it forms exactly what an admitted build forms.

Brotli on the semantic route, same source and config: probelil 2,775 at the start of 008, 1,919 now (default route 1,492). On the [2026-09-22 comparison](../../benchmarks/migration-results/2026-09-22-then-vs-now/README.md): markedlil 9,673, zodlil 29,796 and katexlil 62,397, against the default route's 9,360, 29,682 and 55,404; the default route itself is within 0.3% of the pre-migration compiler. The census passes 72/72/72 with zero miscompiles; zodlil (1,353) and katexlil (1,251) pass their suites; markedlil fails two output-shape assertions (the closed-world option keys already assigned in 007, and one that requires every export to be an alias). Nine search tests assert byte-exact fixtures that the new printing changed; they are re-derived at the end of the batch.

Next in 008: re-derive those fixtures, then liveness and tree shaking, declaration merging and one-use cell forwarding through checked target edits, naming, helpers and the delivery modes.

### 008 progress: integer coercions, batch 2

`|0` stays only where it can change a value. `+x|0` is `x|0` (ToInt32 applies ToNumber, with the same single coercion). Under size-first's length-to-number decision, now the contract assumption `numeric_lengths`, a host `length` is an int32 Number. An `int` cell is int32 when every write is (constants, int operations, normalized loads and host results, and parameters of directly-called-only functions whose every argument is int32). A counting loop then bounds its counter: every write in the loop is one `±1` step toward an int32 bound, the steps one iteration can take are counted (exclusive branches once, a nested loop disqualifies), and under pristine builtins a string or array length is at most 2^30. markedlil keeps 111 of 148 coercions, zodlil 257 of 458; Brotli moves -112 on zodlil, -43 on katexlil, +11 on markedlil and +6 on probelil.

Measured and not adopted, because Brotli rose while raw fell: `return c?a:b` chains (+207 across markedlil, zodlil and katexlil), `x=c?a:b` for two-armed assignments (+56), mangling a binding the naming basis spells with its function's exact name (katexlil +99 at -13,524 raw), and folding calls to constant-returning functions (+26 zodlil, +42 katexlil). Per this milestone's rule, shapes like these become codec-scored alternatives in 010 rather than defaults.

The search fixtures now choose their programs by the property each test needs: a naming crossover between codecs for `byte`, a two-scope `byte` for memory refusal, surviving locals for identical naming outputs, and a longer record key for the pooling tradeoff. The two interaction traps (A and B each lose, A+B wins) did not survive the new printing; both tests are ignored with that reason, and 010's interaction-trap task re-derives them. Library suite 2,998 passed with 3 ignored, census 72/72/72 with zero miscompiles, zodlil and katexlil pass their suites, markedlil fails its two output-shape assertions.

Next in 008: root liveness across modules, the delivery modes (preserve-modules, split and lazy with dynamic `import()`), host-module delivery for jquery's `./js-host.ts`, and deterministic chunk manifests.

### 008-D1 Multi-file delivery on the semantic route

- **Owner and files.** `structured_js/delivery.rs` (new: partition, imports/exports, per-file printing), `semantic_program` (the partition and bundle rendering), `compiler_service.rs` (bundle results) and `main.rs` (writing files and the manifest).
- **Contract.** The default route's bundle contract: `--output entry.js` writes the entry, sibling chunk files and `entry.manifest.json` (version 2: entry, mode, chunks with modules, sizes, dependencies and cache keys).
- **Model.** Module state, initialization and every function that touches either stay in the entry; a function whose free references are its own locals, hosts and other chunk functions moves to its source module's chunk. Chunk files only define functions, so even cyclic chunk imports cannot observe an uninitialized binding, and no file assigns an import. Cross-file names are the bundle's final names.
- **Modes.** `preserve-modules` gives one chunk per source module; `split` keeps a chunk only for modules with at least `shared_min_imports` importers and `min_chunk_bytes`, as the default route does. Lazy chunks follow dynamic `import()` support.
- **Assertions.** Every file parses independently, and loading the entry runs the single-file program's observations exactly. Imports never target the entry, cross-file writes are refused, manifests are deterministic across runs, and the census still passes 72/72/72.
- **Open decision.** The winner is scored as one stream and then delivered as several. The exit criterion scores streams independently, which is 010's job once bundles are search candidates.

**Status: preserve-modules and split delivered; lazy chunks next.** `--backend semantic` with `bundle.mode` `preserve-modules` or `split` writes the entry, its chunks and the version-2 manifest through the same contract as the default route. Every candidate is scored as the sum of its delivered files, so the scored bytes are exactly the delivered bytes.

- **Partition.** A root `let f=<function>` is a chunk candidate when nothing assigns it again and its subtree writes no root binding. It moves to its module's chunk when that module may carry one and everything it reads is itself chunked (greatest fixpoint). A function that calls into the entry module therefore stays in the entry, and no chunk imports the entry.
- **Names.** A chunk's file name is `chunk-<sha256(body)[..10]>-<stem>.js`. Bodies print first, then headers that import those names.
- **Split** is the default route's rule, run inside the render with every codec and render charged to the candidate's budget:
  - Keep only modules imported by at least `shared_min_imports` modules whose provisional chunk has `min_chunk_bytes`.
  - Then add, up to `max_chunks`, the chunk that lowers the bundle's deploy cost most, while one does.
  - Deploy cost is `bundle.cost` over each file's weighted codec bytes, request, depth and cache reuse. That cost function now lives on `ChunkCostConfig` and is shared by both routes. A codec with zero weight is not run.
- **Contract.** The split rule and its costs are part of the JavaScript contract only in split mode, so they change neither other modes' fingerprints nor their bytes. `UnsupportedBundleMode` is gone.
- **Evidence.** Six service tests run every delivered file under Node: each chunk loads without the entry, and the entry prints exactly what the single-file build prints. The cases cover module state, mutual recursion across chunks, a cycle back into the entry, determinism, and split's three outcomes (the chunk pays for itself, the request costs more than cache reuse saves, or the chunk is below the minimum size). Library suite 3,004 passed with 3 ignored; census 72/72/72 with zero miscompiles; probelil matches in both lanes.
- **Remaining:**
  - ~~Lazy chunks for dynamic `import()`, with preload~~: done in 008-D2 below.
  - ~~jquery's host module~~: done in 008-D3 below.
  - ~~Root liveness across modules~~: verified rather than built. Demand already removes, across modules, pure globals, allocations and functions nothing reads. It keeps every initializer effect as a bare call and runs a module imported only for its effects. The service test `liveness_across_modules_drops_unread_values_and_keeps_effects` pins it. Constant folding across the call (the default route prints `console.log(42)`) is 009's.
  - Codec search over bundle plans, which is 010's.

### 008-D2 Lazy modules and dynamic `import()`

**Status: delivered on the semantic route in all three modes.** Dynamic `import()` resolves to the same `Task<namespace>` the default route gives, and `Task` `then`, `catch` and `finally` now convert.

- **Checking.** The direct module checker admits dynamic loaders.
  - A module only `import()` reaches must be initialization-free (the linker's rule and message).
  - Namespace members resolve through the target module's interface exports, not a merged scope.
  - The initialization order is the static order from the entry, then the lazy modules in module order. Both checkers and the program verifier compute it the same way, and the verifier requires every module to be reachable through static or dynamic edges.
- **Program.**
  - New operation: `LoadModule { module, specifier }`.
  - Each module records its dynamic dependencies and its namespace: the exports some code reads through `import()`.
  - Demand keeps a namespace's members live exactly when a load of that module is live, like exports. Unread exports are removed, including from lazy chunks.
- **Output.**
  - In one file, `import()` becomes `Promise.resolve().then(()=>({answer:a}))`. The namespace is built a turn later, once every module has initialized, because our functions are `let` bindings and the default route's synchronous object would read them in their temporal dead zone.
  - A module that nothing imports statically, whose every root statement and namespace member can move, gets a lazy chunk. The chunk exports its namespace under the export names. The entry loads it with `import("./chunk-…").catch(e=>Promise.reject({specifier,message:String(e)}))`, the default route's rejection shape.
  - A lazy module that reads the entry's bindings (the `lazy-cycle` case) keeps the in-file namespace instead, so no chunk ever imports the entry.
  - Syntax targets before ES2020 keep the in-file form.
- **Delivery details.**
  - Chunk bodies print before the entry's, so the entry names lazy chunks by their digests.
  - The default route's `modulepreload` prelude follows `bundle.preload`, which the contract now carries outside single mode.
  - Split counts lazy chunks as mandatory, with the default route's `max_chunks` error.
  - The manifest marks lazy chunks `lazy`, lists dynamic dependencies and preloads, and counts both in depth and reuse.
  - Chunk files take the entry's extension (`.mjs` for an `.mjs` entry) through the new `ServiceOptions::chunk_extension`, so the scored names are the delivered names.
- **Evidence.**
  - Eleven delivery tests run each file under Node. They cover a lazy chunk serving only the members read, preload on and off, a missing chunk rejecting with `ERR_MODULE_NOT_FOUND`, the in-place fallback, the `max_chunks` refusal, the initialization-free rule, and `then`/`catch`/`finally` ordering.
  - The repository's `tests/bundles/lazy` and `lazy-cycle` fixtures print 42 in single, preserve-modules and split mode. The shipped fixture configs strip `print`, and they do so on both routes.
  - Library suite 3,009 passed with 3 ignored. Census 72/72/72 with zero miscompiles, and probelil matches in both lanes.
  - probelil, markedlil, zodlil and katexlil are byte-identical to the 008-D1 binary.

### 008-D3 Host modules travel with the output

**Status: delivered.** The default route makes jquery's output self-contained with tables keyed by helper *names* (`windowSelf`, `arrayPush`, ...). The semantic route will not match names. It carries the host module's own code instead, when a port asks for it with `bundle.host_modules = "embed"`.

- **Settings.**
  - `external` is the default, because port build scripts, katexlil's among them, post-process these imports.
  - `auto` carries the host modules when every one can be delivered, and imports them otherwise.
  - `embed` refuses the build when one cannot be delivered.
- **Stripping.** `oxc_ast` (already pinned; its `serialize` feature is now on) parses each relative `.ts`/`.js` host module. Type-only syntax is erased by the spans of its `TS*` ESTree nodes: annotations, type parameters and arguments, `as`, `satisfies`, `!`, optional marks, and type-only declarations and imports. Each erased byte becomes a space, and line breaks stay. Syntax with runtime meaning is refused: enums, parameter properties, namespaces, decorators, class member modifiers and JSX. The result must parse as JavaScript.
- **Compaction.** Comments and every space and line break not needed between two tokens are removed. Where automatic semicolon insertion ended a statement, an explicit `;` replaces the line break, so the module is one line. The compacted text must parse to the same tree as the stripped text, compared without positions.
- **Linking.**
  - The host modules the output imports, and the relative host modules those import, become one expression. It evaluates each module once, in dependency order, in its own function scope, and returns their namespaces.
  - The output binds its imports with one destructuring `let` before its first statement, where an import would have evaluated.
  - A script output runs the host code strict, as the module it was written as. A classic script may therefore use a carried module.
  - Identifiers host code reads without declaring are reserved from the output's naming.
- **What is refused.**
  - Package imports inside a host module.
  - Default and star exports, and mutable exported bindings.
  - `import.meta`.
  - Syntax newer than the target edition.
- **Bundle modes.** Only the entry reads carried bindings, so statements that read them stay in the entry.
- **Evidence.**
  - Host-module unit tests cover stripping, the refusals, compaction (`return⏎1` becomes `return;1`, and `for` heads are untouched) and linking.
  - Two service tests cover single, preserve-modules and split modes, a strict script, `auto`'s fallback on an enum and on `??` under ES2019, `embed`'s refusal, and the `external` default.
  - **jquerylil passes all 7 of its compatibility cases on the semantic route, from 0 of 1 before, because its output now loads.** Its migration patch sets `host_modules = "embed"` and teaches its build script to find the `jQuery` export whether or not it is aliased.
  - The semantic artifact is 119,472 raw and 34,346 Brotli bytes, against 27,854 Brotli for the default route's committed dist. Closing that gap belongs to 009 (host helper inlining, mangling host locals) and 013.
  - Library suite 3,016 passed with 3 ignored. Census 72/72/72 with zero miscompiles, and probelil matches in both lanes.
  - probelil, markedlil, zodlil and katexlil are byte-identical to the 008-D2 binary. None of their configs carries host modules; katexlil's font data, a `var` export, would stay an import even under `auto`.

### 008 progress: target compaction, batch 3

Three compactions ship as defaults. They run as checked edits on the finished target tree, under the target-compaction tactic.

- **One-use forwarding.**
  - `let x=v;S` becomes `S` with `v` in place of `x` when `x` is referenced exactly once, as the first thing `S` evaluates. That covers a return, an expression statement, a throw, a declaration, an `if` test, and a `for-in`/`for-of` head. The same evaluations then run in the same order, and chains collapse: `a=>{let b=a,c=b/4,d=c*2.5-1.5;return d}` prints `a=>a/4*2.5-1.5`.
  - Exempt: function and class values, which their binding names, exported and pinned names, and loop tests, which repeat.
  - Root statements merge only within one source module.
  - Every edit keeps the verifier's invariants by construction: a value moves only below a later parent in the arena, and only while its deepest point stays within the nesting limit.
- **Optional catch binding.** A catch whose exception nothing reads prints `catch{`, from ES2019.
- **Argument-less construction.** `new X()` prints `new X` wherever no call or member access follows it.

Brotli against the 008-D3 binary, same sources and configs:

| Port | Brotli | Raw |
|---|---|---|
| katexlil | −559 | −1,730 |
| markedlil | −20 | −32 |
| probelil | −16 | −102 |
| zodlil | −6 | −132 |

- **Measured and not adopted.** Moving a block's counter into its `for` head (`for(let i=0;…)`) cuts 243 raw bytes on katexlil but adds 125 Brotli, and it is neutral elsewhere. It stays behind `Module::loop_head_declarations`, off, for 010 to score per artifact.
- **Evidence.**
  - Library suite 3,018 passed with 3 ignored. Census 72/72/72 with zero miscompiles, and probelil matches in both lanes.
  - Port suites pass: katexlil 1,230/1,230 and 21/21, zodlil, and jquerylil 7/7. markedlil fails only its two known output-shape assertions.

### 008 progress: numeric and boolean spelling, batch 4

- **Booleans.** Booleans print `!0` and `!1`, as the default route prints them, with unary precedence so a member access parenthesizes them.
- **Length ranges.** Under pristine builtins a string or array length is at most 2^30, the bound counting loops already use. A length now carries that range into the numeric facts, so `a[a.length-1]` needs no `|0`.

Brotli against batch 3:

| Port | Brotli | Raw |
|---|---|---|
| zodlil | −41 | −1,375 |
| katexlil | −33 | −1,410 |
| markedlil | −20 | −422 |
| probelil | −3 | −7 |

Library suite 3,018 passed with 3 ignored. Census 72/72/72 with zero miscompiles, and probelil matches in both lanes. katexlil (1,230 and 21), zodlil and jquerylil (7) pass their suites; markedlil fails only its two known output-shape assertions.

### 008 progress: calls with an undefined receiver, batch 5

`JS.call(f, t, ...)` already printed as the plain call `f(...)` when `t` was a literal `undefined`. It now also does so when `t` calls a function whose body only returns undefined, with no parameters, no suspension and no effect, like katexlil's `undef()`. The dropped call has no effect, and a plain call's receiver is undefined too, so evaluation is unchanged. katexlil's `.call(` sites fall from 1,540 to 74.

| Port | Raw | Brotli |
|---|---|---|
| katexlil | −13,183 | −368 |
| zodlil | −41 | −16 |
| markedlil | 0 | 0 |
| probelil | 0 | 0 |

A service test runs the plain form and keeps the call when the receiver helper has an effect. Library suite 3,019 passed; census 72/72/72 with zero miscompiles; probelil matches; katexlil (1,230 and 21) and zodlil pass their suites.

### 008 progress: keyword spacing, batch 6

`return`, `throw` and `else` print a space only when the next token would otherwise continue the keyword. So `return!0`, `return(a+b)`, `throw"x"` and `else if` all print correctly.

| Port | Brotli | Raw |
|---|---|---|
| zodlil | −41 | −270 |
| markedlil | −3 | −18 |
| katexlil | −3 | −152 |
| probelil | −1 | −12 |

Library suite 3,019 passed; census 72/72/72 with zero miscompiles; probelil matches in both lanes.

**Measured and not adopted.** Printing a one-statement `if` as `c&&e;` (or `if(!c)e;` as `c||e;`) where neither side needs grouping cuts raw bytes, and the default route uses the shape. Here it costs zodlil +34 and katexlil +38 Brotli, and saves markedlil only 4. It stays behind `Module::logical_statements`, off, beside the for-head merge. Both are 010's codec-scored alternatives, and both are tested with the flag on.

### 008 exit evidence: real libraries across delivery modes

markedlil and zodlil, each built with `--backend semantic` from an unmodified scratch copy, differ from one build to the next only in `bundle.mode`.

| Library | Mode | Files | What loading the entry observed |
|---|---|---|---|
| markedlil | single | 1 | Export names and four parses covering headings, emphasis, nested lists, quotes, fenced code, links, images, tables, rules and HTML |
| markedlil | preserve-modules | 6: types, rules, str, lexer and parse chunks, plus the entry | The same bytes |
| markedlil | split | 1: no chunk pays its request at the default cost | The same bytes |
| zodlil | single | 1 | Export names, three object parses (valid, too small, invalid types) and a union parse |
| zodlil | preserve-modules | 5: fastpass, host, api and engine chunks, plus the entry | The same bytes |

The chunks share helpers and record shapes across module boundaries, and each import resolves only to another delivered file. The service tests above cover the rest of the exit list:
- Each file loads independently.
- Getters and callbacks run through the entry.
- Cycles, including a cycle back into the entry.
- Initialization order and side-effect imports.
- Determinism.
- The scored bytes are the delivered bytes.

Scoring streams independently remains with 010.

## 009 Reusable Compression Families

Contracts: A2-A7. Consume 006's interfaces and 008's target/delivery owner. Use [optimization coverage](../optimization-coverage.md) as inventory and the design's competitor mapping as questions to test, not parity evidence.

| Family group | Effects to cover through common interfaces |
|---|---|
| Semantic cleanup | Dead computations/stores, exact/range propagation, conditions, checks and common expressions with effects/identity preserved |
| Calls/control | Bounded/partial inlining, specialization/devirtualization, unused arguments/returns, sharing/outlining, merging and useful loop/branch alternatives |
| Aggregates | Scalarization, owned fields/layouts, partial escape/materialization and container/closure transport, updating all consumers/adapters |
| Data | Computation/literal alternatives, pooling, tables/packing, numeric spelling and permitted reconstruction; bounded construction and explicit startup/recurring/memory costs |
| Target/layout | Compact syntax, naming seeds, safe declaration/helper order and bounded expression alternatives |

Each family declares facts, changed domains, compatibility/conflicts, lowering, policy ownership and risk evidence. Record applicable competitor capabilities, coverage and finite gap tasks. Copying upstream code requires checking its license; matching pass names/order is unnecessary.

**Exit:** legality/refusal, direct/search veto and useful positive tests; cross-family cases for flattening/inlining, sharing/naming, packing/placement and specialization/DCE. Ablate real workloads with exact bytes and compiler/runtime costs. Remove unproductive complexity unless a bounded measured use justifies it. Retire replaced facts/mutation paths. Package matchers, independent family budgets/solvers and untracked edits fail the gate. Competitive claims wait for 013.

### 009 ablation: which families earn their bytes today (2026-09-22)

Each run switches one tactic off with `[policy.tactics] <tactic> = "off"`. Every build is the semantic route on an unmodified scratch copy (katexlil with its recorded migration patch), measured by the repository codec. Cells give the Brotli bytes added by switching the family off.

| Family off | probelil (1,905) | markedlil (9,641) | zodlil (29,580) | katexlil (61,391) |
|---|---|---|---|---|
| target compaction | +2,278 | +10,344 | +40,410 | +80,937 |
| naming search | +573 | +1,912 | +9,571 | +11,687 |
| dead-code elimination | +269 | +15 | +2,036 | +4,364 |
| scalar replacement | +13 | 0 | 0 | 0 |
| inlining (leaf helpers) | +1 | +2 | 0 | 0 |
| constant folding | 0 | 0 | +11 | 0 |
| call specialization | refused* | 0 | refused* | 0 |
| string pooling | refused* | refused* | refused* | 0 |

\* The port's config pins the tactic explicitly, so a policy override contradicts it.

**The proof-heavy families contribute almost nothing on the reference ports.** Formation compaction, naming and liveness carry the output. The default route's remaining lead comes from IR-level inlining and folding. The semantic helper family admits only pure scalar bodies (loads, stores, constants, arithmetic, selects, a tail return), so it refuses almost every real helper. Refusals logged on katexlil's search: 44 body operations (every call, intrinsic or host builtin), 33 not a private callable, 12 callable observations, 3 completion shapes. On probelil: 37 body operations and 13 initializations. katexlil's search made only 5 proposals from 1,160 proof queries.

**009's first family task:** inlining that admits real helper bodies, while keeping the frame-elision and observation proofs the helper family already owns. Next come folding and propagation on its results, then a decision on the families that stay at zero.

**First attempt, measured and reverted.** Under strict (module) execution, a host cannot observe a missing frame (`caller` is null either way). So the leaf-helper family was widened to admit bodies that call builtins, primitive intrinsics or host functions, plus intrinsic reads, templates and type tests, all with coercion behavior. Source-function calls stayed out, so an inlined leaf cannot recurse.

- katexlil's publishable proposals rose from 5 to 23, and the search combined them into 8 structures.
- A 40× larger search budget explored exactly the same 23 proposals and 8 structures.
- Not one byte moved on any reference port.
- Compile time rose from 0.8 s to 11 s on markedlil, and from 3 s to 10.5 s on zodlil.

It is reverted as unproductive complexity. The default route's inlining lead on katexlil comes from inlining a wider class than function-typed private cells, including `JsValue` variables holding functions (`defineSymbol = (...) => ...`), and from simplifying afterwards (`"M95,"+str(x)` becomes `"M95,"+x`). That needs whole-program substitution with follow-on folding, not more proposals from this family.

**Prior art** (from `finer/refs/competitor-techniques.md`, §D):
- Terser inlines small non-recursive bodies at call sites (`compress/inline.js`, gated 0–3) and collapses single-use declarators (`tighten-body.js`).
- oxc does single-use substitution only (`peephole/minimize_statements.rs:1137-1330`) and no general function inlining.
- 046 measured multi-use constant propagation as Brotli-negative on katexlil (+129), so it is not a target.

### 009 batch 1: substitution and folding on the finished program (2026-09-22)

The rules below are generic. Each carries its own soundness condition, and none consults a port. They run under target compaction, except the dead-statement rule, which runs at conversion.

- **Function names nothing can read.** A closure keeps an exact `.name` only when something can observe it. It no longer does when every use calls it, directly, through `JS.call`/`JS.apply`/`JS.construct`, or through local cells it is initialized into or stored into (`let f; f=(…)=>…`) whose every read only calls it. Reads that no demand context keeps do not count. A public export or a dynamic-import namespace member still counts as an observation. The namespace check also closes a hole in the private-function rule: a lazy chunk's `answer` now keeps its name and constructibility.
- **Unreachable statements.** Conversion stops lowering a block after a statement that returns, throws or jumps on every path, unless a later statement declares a variable an earlier closure may name. The ports' transliterated `return x; return undef();` pairs vanish.
- **Calls to functions that only return undefined** are `undefined`. Then `let x=void 0` is `let x`, `return void 0` is `return`, and a bare `return` ending a function body goes.
- **Forwarding functions.** A call to a declared function whose body passes each by-value parameter once, in order, to one host builtin and returns its result, is that builtin applied to the call's arguments. With pristine builtins that covers every host builtin. Otherwise it covers only literal and operator spellings, which evaluate nothing before their last operand. `JS.object` also needs literal keys. Substituting all of them never cost Brotli on a reference port. zodlil gains 478 bytes while its raw size grows by 2,313: the builtin spellings repeat where wrapper names did not.
- **Stores into a fresh object literal.** With pristine builtins, `let o={…};o.k=v;o["j"]=w` becomes `let o={…,k:v,j:w}`. `__proto__` stays a store. Folding is refused when a stored value mentions `o`, or when an earlier statement or hoisted function of its region does. Such code could read `o` in its temporal dead zone.
- **Liveness.** `JS.object`, `JS.array` and `JS.undefined` get literal effects instead of unknown ones, so unused namespace objects (`({default:E,SourceLocation:E});`, 37 on katexlil) are no longer evaluated. A function binding no reachable code references is dropped from the tree.

| Step | markedlil | zodlil | katexlil |
|---|---|---|---|
| Before (`764fa9cf`) | 9,641 | 29,580 | 61,391 |
| Function names nothing can read | 0 | 0 | −367 |
| Unreachable statements, undefined calls, builtin literal effects | 0 | −402 | −1,277 |
| Name proof ignores dead reads | 0 | 0 | −243 |
| Forwarding functions, object-literal stores | −9 | −489 | −638 |
| Unreferenced functions | 0 | −49 | −62 |
| **After** | **9,632** | **28,640** | **58,804** |
| Default route | 9,360 | 29,682 | 55,404 |

probelil is unchanged at 1,905. katexlil's raw size falls from 257,809 to 242,762. zodlil's rises from 123,841 to 125,222, with Brotli down 940. zodlil now beats the default route by 1,042 bytes. katexlil's gap shrinks from 10.8% to 6.1%.

**Measured and not pursued.** On katexlil, renaming the three most-used public bindings (`defineSymbol`, 650 uses; `defineMacro`; `defineFunction`) saves 10,490 raw bytes but only 44 Brotli. The wrappers their exact `.name` would need cost more than that. Terser's mangle-only pass on this output finds −453 in total: −136 from reassigning short names and about −317 from the remaining long ones. Most of those are functions held in namespace objects the port calls through (`JS.invoke(html, "buildGroup", …)`). Freeing them needs a heap proof that those objects' members are only invoked.

**Verification.** The unit suite passes (3,024 tests), as do the census (72/72/72, no miscompiles) and probelil in both lanes. The katexlil suites pass (21/21 and 1,230/1,230), and so do zodlil's. jquerylil passes 7/7. markedlil fails only its two known shape assertions. New tests:
- `undefined_calls_and_unreachable_statements_leave_no_residue`
- `a_closure_only_invoked_through_its_cell_loses_its_name`
- `forwarding_wrappers_become_their_builtins_and_stores_fold_into_the_literal`
- `a_store_whose_value_may_read_the_object_stays_a_store`

**Next.** The default route's remaining lead on katexlil shows in its shapes:
- It inlines single-expression helpers (`x.length` 186 times against our 96).
- It spells literal arrays' methods directly (`a.push(b)`).
- It forms conditional expressions (`?:` 371 times against our 55).

The first needs a target-level inliner with an arena renumbering pass, so edits can splice subtrees. The second is the default route's native-array proof (`call_array_methods_directly`). The third belongs to 010's codec alternatives.

### 009 batch 2: inlining single-expression functions (2026-09-22)

- **Inlining single-expression arrows** ([inline.rs](../../src/structured_js/inline.rs)). A `let`-bound arrow `(a,b)=>E` that nothing reassigns or exports is replaced at every call that passes its parameters, if `E` has at most six nodes or the arrow has only that one call. The call evaluates its arguments first, then `E`. So `E` must read each parameter once, in parameter order, outside any branch. Before its last parameter read, `E` may evaluate only literals, other parameters and, with pristine builtins, standard globals and their named properties. `E` creates no function, reads no `this` or `arguments`, has no `delete` and does not come from a strict body. Six nodes measured best: a limit of three leaves markedlil 97 bytes larger, and ten or twenty add nothing.
- **Arena renumbering.** Edits may now splice a subtree anywhere. `renumber` rebuilds the expression arena in postorder from the code that can run. Observed-literal alternatives follow their literals and stay sorted, which their binary search needs.
- **Frames in classic scripts.** In a sloppy script, user code run from a function (a `valueOf`, a getter, a called function) sees that frame as `arguments.callee.caller`. Removing the frame is observable there, as `script_keeps_a_coercing_private_helper_frame_and_rejects_its_inline_candidate` shows. So the inliner, and batch 1's forwarding-function substitution, remove a frame in a script only when the body runs no user code: literals, strict equality, `typeof`, `!`, literals built from those, and standard library paths. Modules run strict, and a strict frame hides its caller. Batch 1 shipped the substitution without this gate. No reference port runs as a script except probelil, whose probe does not read `caller`. The new test `a_script_keeps_the_frame_of_a_coercing_forwarding_function` covers both execution modes.
- **Declarations meet their first assignment.** `let x;…;x=v` becomes `…;let x=v` when that assignment is the first code of the region to mention `x` and `v` does not. No hoisted declaration of the region mentions it either, so no read meets the later declaration's temporal dead zone. katexlil's transliterated `JsValue x = undef(); … x = …` pairs merge wherever that holds.
- **Inert statements go.** A bare statement whose value only creates literals and functions has no effect. One example is the `({});` left where `emptyObject()` became `{}`.
- **`JS.number` of a number literal** is that literal, so the ports' `toNum(0)-toNum(1)` spelling no longer prints `+0-+1`.

| Step | probelil | markedlil | zodlil | katexlil |
|---|---|---|---|---|
| Batch 1 | 1,905 | 9,632 | 28,640 | 58,804 |
| Inlining single-expression arrows | −23 | −135 | −206 | −28 |
| Script frame gates, declaration merge, inert statements, number literals | −3 | +2 | −4 | −659 |
| **After** | **1,879** | **9,499** | **28,430** | **58,117** |
| Default route | 1,492 | 9,360 | 29,682 | 55,404 |

markedlil is now 1.5% above the default route, katexlil 4.9% and probelil 25.9%, while zodlil is 4.2% below. probelil's gap is structural. The default route inlines each once-called probe function into the script body, while in a script the frame rule keeps any body that can run user code.

**Verification.** The unit suite passes (3,025 tests, plus the frame test), as do the census (72/72/72, no miscompiles) and probelil in both lanes. The katexlil suites pass (21/21 and 1,230/1,230), and so do zodlil's. jquerylil passes 7/7. markedlil fails only its two known shape assertions.

### 009 batch 3: spellings and literal folds (2026-09-22)

Found by ablating Terser's compressor on our own output, one option at a time and leave-one-out, then keeping only what is sound for us:
- **Double negations.** `!!x` is `x` where only its truth matters: conditions, `!` operands, discarded values, and `&&`/`||` operands whose own truth is all that matters. katexlil had 476 `if(!!` and 167 `!!!`.
- **Quoted keys.** A literal string key that is not an identifier prints `">":`, the same own data property as `[">"]:`. `__proto__` and observed literals keep the computed form.
- **Numbers.** `.5`, not `0.5`, and `1e3`, not `1000`. The shortest round-trip digits, then plain or exponent form, whichever is shorter. A test checks every spelling reads back as the same double.
- **Literal arithmetic.** Number operations on literals take their result when it is no longer: exact binary64 arithmetic, and the language's int32 contract for integer operations. `JS.number` of an operand that is already a number (a literal, `+x`, or arithmetic on numbers, never a BigInt) is the operand. `toNum(0)-toNum(1)` is now `-1`.
- **`JS.add` string chains.** `JS.add` of two literals is their concatenation. A `JS.add` sum ending (or starting) with a literal takes the next literal into it, since a sum with a string is a string and concatenation associates. Typed string sums stay the string family's codec choice among computed, literal and shared spellings. An earlier version folded every literal sum at the target and removed that choice. Three string-family tests caught it, and the fold moved into `JS.add` formation.
- **Exact-name wrappers.** Where a binding, target or key would name the value, `{htmlBuilder:f}.htmlBuilder` needs no `(0,…)`. That position is never a callee, a statement start or an arrow body.
- **Dead declarations.** A `let` nothing reachable references goes when its value only creates literals and functions.

| Step | probelil | markedlil | zodlil | katexlil |
|---|---|---|---|---|
| Batch 2 | 1,879 | 9,499 | 28,430 | 58,117 |
| Double negations, quoted keys | 0 | −3 | −58 | −176 |
| Numbers, literal arithmetic, `JS.add` chains, bare name wrappers | −2 | 0 | +27 | −198 |
| **After** | **1,877** | **9,496** | **28,399** | **57,743** |
| Default route | 1,492 | 9,360 | 29,682 | 55,404 |

**Measured and not adopted.** Each saves raw bytes and loses Brotli, because the longer spelling repeats across the file:

| Change | Brotli |
|---|---|
| Loose `typeof` comparisons | markedlil +5, katexlil +6 |
| `else` removed after a `return` (95 sites) | katexlil +5 at −475 raw |
| `x.push(v)` instead of `Array.prototype.push.call(x,v)` (59 sites) | katexlil +21 at −1,239 raw |
| Terser's `join_vars`, `loops`, `conditionals`, `sequences` | negative on our output |

**Correction.** One late change in this batch went in after only targeted tests and the census: dropping any unused `let` whose value creates only literals and functions. It removed `let discardedProduct=134623` even with `dead-code-elimination = "off"`. Batch 4 gates it, and the bare-statement removal, on that permission.

**What remains.** Terser's full compressor still takes katexlil from 57,743 to 56,313. Leave-one-out puts most of it in `unused` with `reduce_vars`. That is single-use function inlining: `sqrtPath`'s one call receives each SVG path builder as an IIFE with the constant `Ma` substituted, and the definitions go. Next is target-level inlining of single-use functions, where arguments that are literals or never-written bindings may be read more than once.

**Verification.** The unit suite passes (3,027 tests), as do the census (72/72/72, no miscompiles) and probelil in both lanes. The katexlil suites pass (21/21 and 1,230/1,230), and so do zodlil's. jquerylil passes 7/7. markedlil fails only its two known shape assertions.

### 009 batch 4: arguments an inlined body may repeat (2026-09-22)

The single-expression inliner now takes bodies that read a parameter more than once, or not at all, when the argument's value cannot change. Each argument read exactly once still follows batch 2's rule: read in order, outside a branch, with only inert evaluations before the last such read. The stable arguments depend on the call:
- **Any call:** a literal, which is copied or dropped. Or a binding no other function mentions, which the call's other arguments do not mention either. No call can reach that binding and the body assigns nothing, so every read sees one value. Its first read still keeps its place, so an uninitialized binding throws where the argument would have.
- **A function's only call,** where the function then goes: additionally a parameter nothing assigns, and a root constant declared with a literal before any root statement runs code. Such an argument may be read at any point, since it is initialized and cannot change. Doing this at every call measured worse on markedlil (+20) and zodlil (+15).

| | probelil | markedlil | zodlil | katexlil |
|---|---|---|---|---|
| Batch 3 | 1,877 | 9,496 | 28,399 | 57,743 |
| **After** | **1,877** | **9,461** | **28,383** | **57,743** |
| Default route | 1,492 | 9,360 | 29,682 | 55,404 |

katexlil's SVG path builders are single-expression functions called once, from `sqrtPath`, and Terser inlines them. They pass `Ma`, a root constant. `Ma` follows root statements that call functions, so without a call-graph argument it is not provably initialized when read.

**Verification.** The unit suite passes (3,028 tests), as do the census (72/72/72, no miscompiles) and probelil in both lanes. The katexlil suites pass (21/21 and 1,230/1,230), and so do zodlil's. jquerylil passes 7/7. markedlil fails only its two known shape assertions. New test: `inlined_bodies_repeat_only_arguments_whose_value_cannot_change`.

### 009 batch 5: constructors become literals (2026-09-22)

markedlil's token constructor printed as a literal of defaults, a call to a reset method that stored the same defaults, an alias, then the constructor's own stores. The default route prints `(t,e)=>({kind:t,raw:e,…})`. Four generic edits now reach that shape:
- **Statement-level inlining.** `f(x);` becomes `f`'s statements when:
  - `f` is a `let`-bound arrow that nothing reassigns or exports;
  - this is its only call, and nothing reads `f` but that call;
  - its body is only expression statements, with no `this`, `arguments`, function creation or assignment to a parameter.

  Every argument must hold one value through the body. That means a literal, or a binding no call can reach that no other argument mentions, initialized before the call (a parameter, or declared earlier in the call's region). A classic script keeps a frame whose user code could see it. A copied body may call a function whose own inlining edited the original, so the pass runs up to three rounds.
- **In-place store replacement.** In the object-store fold, a store to a key the fresh literal already has replaces that entry where it stands. The property keeps its first position either way. The replaced value must be inert, and so must everything after it.
- **Alias elimination.** `let c=d` goes, and `c` reads as `d`, when:
  - neither is ever assigned;
  - `d` is initialized there (a parameter, or declared earlier in the region);
  - nothing earlier in the region mentions `c`.
- **Ordering.** Declarations merge (`let o;o={…}` becomes `let o={…}`) before stores fold, and forwarding runs again after a fold. Forwarding counts only references code can reach, since edits leave unreachable nodes.

| | probelil | markedlil | zodlil | katexlil |
|---|---|---|---|---|
| Batch 4 | 1,877 | 9,461 | 28,383 | 57,743 |
| **After** | **1,872** | **9,398** | **28,326** | **57,622** |
| Default route | 1,492 | 9,360 | 29,682 | 55,404 |

markedlil is now 38 bytes (0.4%) above the default route.

**Verification.** The unit suite passes (3,030 tests), as do the census (72/72/72, no miscompiles) and probelil in both lanes. The katexlil suites pass (21/21 and 1,230/1,230), and so do zodlil's. jquerylil passes 7/7. markedlil fails only its two known shape assertions. New tests:
- `a_reset_method_and_its_stores_fold_into_the_constructed_literal`
- `inlined_statements_never_repeat_an_argument_with_effects`, which covers an argument with effects and an alias whose source is assigned again.

### 009 ablation, re-run after batch 5 (2026-09-22)

The same method as the first ablation, on the batch 5 build. Cells give the Brotli bytes added by switching each family off.

| Family off | probelil (1,872) | markedlil (9,398) | zodlil (28,326) | katexlil (57,622) |
|---|---|---|---|---|
| target compaction | +2,311 | +10,587 | +41,626 | +82,338 |
| naming search | +559 | +1,882 | +10,484 | +11,595 |
| dead-code elimination | +166 | +140 | +1,641 | +3,766 |
| constant folding | 0 | 0 | +15 | +48 |
| leaf-helper inlining | 0 | +3 | 0 | 0 |
| scalar replacement | +8 | 0 | 0 | 0 |
| call specialization | refused* | 0 | refused* | 0 |
| string pooling | refused* | refused* | refused* | 0 |

\* The port's config pins the tactic.

The batch work lives in target compaction and dead-code elimination, where it belongs: it edits the finished program. The proof-heavy families still add 0–8 bytes on the reference ports. They cost no measurable compile time either: katexlil takes 3.6 s with or without them.

**Recommendation, not yet acted on:** keep them until 013 ablates the whole fleet. Four ports are too narrow to justify deleting tested subsystems whose value may lie in typed ports outside this set. If the fleet agrees they are zero, 013 removes them.

**Two gated spellings, measured.** Loop-head declarations (`for(let i=0;…)`) are never larger: katexlil −33, the others 0. They are now on under target compaction. `&&`/`||` statements for one-statement `if`s lose on three ports (markedlil +9, zodlil +67, katexlil +22) and stay off.

**Runtime.** Node 24, medians after warm-up, comparing the semantic route now, the default route and the semantic route before 009 began:

| Workload | Semantic, now | Default route | Semantic, before 009 |
|---|---|---|---|
| markedlil `parse` over the spec corpus (40 rounds) | 33.3 ms | 34.5 ms | 34.6 ms |
| katexlil `renderToString` over 8 formulas × 20 (30 rounds) | 22.0 ms | 20.0 ms | 22.1 ms |

The 009 edits leave runtime unchanged. katexlil's roughly 10% gap to the default route predates them and goes to 012's speed gates.

**Where katexlil's remaining gap is.** Terser's leave-one-out on the current output still puts most of it in `unused` and `reduce_vars` (+1,245 and +1,150 when removed). That is single-use multi-statement functions moved into their call sites as IIFEs: `sqrtPath`, `tallDelim`, the console helpers and the surrogate-pair decoder. An IIFE allocates a closure every time its enclosing code runs. That is a runtime cost 012's speed gates would have to accept, so it is not taken here.


## 010 Bounded Codec Search

Contracts: A4/A6/A7. Consume validated choices and the family registry. Naming selection is one component of semantic representation search.

One scheduler discovers qualified opportunities, combines compatible assignments and retains the admitted direct incumbent. Use bounded estimates, beams/queues, diversity and replacement/combined moves, allowing locally worse alternatives. Pruning is heuristic unless proved. Charge discovery, failed proposals, analysis, rendering and codecs; bound queues, caches and concurrent scratch.

Share unchanged work, qualify caches by source/contract/analysis bounds/recipe/target/codec where relevant and safely deduplicate exact bytes. Winners require exact requested-codec scores and complete delivery admission. Zero/tiny budgets or cancellation return an admitted incumbent when available; mandatory-baseline failure is explicit. Missing measurements are not zero.

Develop heuristics in this order, using the fast-feedback policy above:

1. Remove repeated rendering/scoring where exact identities allow reuse. Keep equal-byte recipes separate if their legality or future choices differ.
2. Pin a true structural interaction trap: A and B each lose after their own allowed naming search, while A+B wins. Add distracting opportunities and narrow beams; compare bounded search with a small exhaustive oracle. Existing raw-loser and fixed-naming interaction cases are insufficient.
3. Measure the separate costs of discovery/proofs, structural expansion, naming/literal trials and codecs. Interleave these neighborhoods so exhausting name seeds for one structure cannot starve useful structural work. Compare Brotli-only tuning with all-objective scheduling explicitly.
4. Compare the current quality/age policy with a bounded interaction lane. Prioritize combinations sharing changed use/def, capture, helper, layout or literal domains; retain structurally distinct seeds as well as good scores. Use measured budgets for the lane, not an arbitrary universal percent-loss cutoff or an eager all-pairs graph.
5. Rediscover opportunities only in domains invalidated by a checked equivalent rewrite. Current static-baseline subset discovery cannot represent newly enabled transformations; rescanning every program for every candidate is not the solution.
6. Extend legal naming neighborhoods within the same scheduler: seed styles, scope-local substitutions/swaps and relevant combined moves. Revisit names after structural changes. Do not build a second winner-only naming optimizer or treat raw length as a Brotli oracle.

An initial heuristic may remain simple. Measure quality regret against the finite oracle and bytes-versus-cost curves on frozen tuning/held-out boundaries before adding adaptive or learned ranking. A fixed number of unsuccessful proposals alone is not proof that interaction work is exhausted. No heuristic is adopted solely because it wins a hand-selected fixture.

**Exit:** tiny finite-space compatibility/winner tests against exhaustive references and interaction traps under narrow heuristic beams. Test stale caches, collision-safe deduplication, failed codecs, discovery fairness, memory refusal and parallel reservations. Declare deterministic seeds/ties/schedules and warm/cold behavior; time cutoffs report truncation. Continued compatible search cannot worsen its incumbent. Presets promising higher-effort nonregression must replay/include lower-effort winners; changing a beam/schedule proves no monotonicity. Real-library effort/objective/flag matrices and ablations show bounded scaling and replayable winners.

## 011 Public Compiler and Fleet Integration

Contracts: A1-A7. Route CLI, library APIs, build/package entrypoints and all supported JS/native delivery through the same service under explicit migration selection. Examples are clients. Complete remaining discovery-to-output resource/admission integration; verify 007's features combined with 008's delivery.

Complete every required build/test adapter and immutable case inventory. Execute maintained original suites against exact objective/profile artifacts, including downstream dependencies, initialization and browser/host behavior. Loaded bytes must match scored identities. Qualify reusable APIs separately from application specialization; subsets cannot establish whole-package support.

Cover every public flag and high-risk intersections of objective, effort, assumptions, family vetoes, runtime constraints, syntax, delivery and native settings. Record backend and resolved policy on every build. Preserve supported LSP, formatting and diagnostics when shared frontend changes affect them.

**Exit:** repository-required checks and complete required library/native suites through public entrypoints, verifying APIs, function/property observations, errors, callbacks and value/reference behavior. Missing/failed cases keep the gate open. Hidden old-route fallback, package settings, narrowed APIs or post-score minification cannot pass. Record compression losses for 013; close semantic losses here.

## 012 Compilation Speed and Resource Gates

Contracts: A1/A3/A6/A7. Apply 001's frozen quality/cost policy to paired builds from discovery to packaged/scored output. Record source/build/host/threads/cache definitions, wall/user/system time, peak RSS, copies/allocations, analyses, renders/codecs and time to first valid artifact. Separate installation, tests and independent replay from compiler timing.

Alternate repeated pairs under the sample/noise rules. Compare cold/repeated builds at equivalent contracts and output-quality targets. Improve measured redundant work through existing owners: demand-driven facts, sparse invalidation, shared choices/prepared targets, deduplicated artifacts and bounded concurrency. New caches/layers require measured benefit and bounded lifetimes.

**Exit:** no required row violates compile-time/RSS/runtime envelopes; representative fast/default speed targets pass at declared quality. Report median/p95, uncertainty and inconclusive rows. Exercise large graphs, no-op/local/public edits and whole libraries. Verify ceilings/cancellation/cleanup, distinguishing charged memory from RSS, soft deadlines from process limits and compiler from generated-program memory. Microbenchmarks/fewer codecs alone cannot establish speed. Architectural bottlenecks reopen their contract; broader limits require recorded policy changes.

### 012-P1 Semantic Phase Telemetry

The [phase-timing receipt](../../benchmarks/migration-results/2026-09-20-semantic-phase-timing/README.md) extends the existing opt-in timing owner with demand, formation, verification, edition, naming-basis, name allocation, printing and separate canonical gzip/Brotli buckets. Stack guards record attempts/refusal/unwind; native and raw-only requests do not invoke encoders. No semantic, policy, budget, cache or default change is introduced; schedule 22 remains. Counts are process-global and durations are accumulated elapsed scopes, not CPU or additive wall totals. Profiling overhead can affect deadlines, so diagnostic comparisons use deterministic work/probe caps.

There are 186 distinct accepted debug checks and 186 final release checks. Initial test import, native-export fixture and raw-bookkeeping assertion failures remain preserved; only the final test assertion changes when reusing 185 unaffected debug passes. Five alternating off/on pairs on three existing small cases preserve all deterministic outputs/search/resources. Structural search's median Brotli time is 14.636 ms, versus demand 0.318, formation 0.297 and naming 0.127 ms, at unchanged 243/167/143-byte winners. All 32 previous search rows and 24 oracle records remain exactly equal on the final release-profile test binary. Root verifies 981 inputs and 151 outputs; no independent agent review. The input digest is `742f203a45dfd5483ff2774291e29563abf9b5adb5b9e39c955bb8eb380d8dc8`. This is not shipped-CLI, held-out library, speed/RSS improvement or milestone qualification. It argues against a naming cache based on this tiny fixture, not against naming optimization generally; profile larger existing semantic workloads before changing cost policy. The earlier emission-heavy Marked observation concerns a different, legacy route.

### 012-P2 Production CLI Cost And Probe Budgets

The [production CLI study](../../benchmarks/migration-results/2026-09-20-semantic-cli-cost/README.md) uses the unchanged 12-module integration source/host fixture on the current optimized release. The fresh public cohort passes 89 library checks, one CLI check and original JS/native consumers; all 15 compared deliveries match C7. Combined with the previous 186 checks on the identical library-test binary, 242 distinct library checks are accepted. Older broader cohorts are not silently carried forward. Five timing-off/on pairs preserve exact artifacts, scores, search and resource accounting. The Brotli-selected output independently measures 17,476 raw / 3,834 gzip / 3,503 Brotli bytes. Median enabled time is 1,877.934 ms in 97 Brotli encodes, versus 19.909 ms formation and 6.047 ms naming. This is production-slice evidence, not whole Marked or the separate emission-heavy legacy path.

Three rounds over ten explicit configurations compare immediate/staged scoring at 8/24/48/96/192 optional probes, holding proposals at 96 and all other policy/contract fields equal. At 24 probes, staged scoring delivers the exact full-run artifact in 521.818 ms versus 1,944.863 ms at the original cap; eight probes cost 202.235 ms and one additional Brotli byte. Immediate scoring remains at 3,512 bytes at 24/48 probes. At 96/192 caps, only 96 optional probes execute before the proposal limit. Staged exploration spends more formation/work at tight probe budgets. These are configured-workload tradeoffs, not a compiler algorithm improvement, universal cap or proof that a plateau is safe to stop. All 42 accepted outputs, including warmups, pass unchanged host observations and independent codecs. Root reconciles 608 outputs, five binaries and the unchanged 981 inputs; the initial benchmark JSON-parser failure is preserved. No independent agent review or milestone closure is claimed. Next compare interaction retention and held-out quality-per-effort before changing defaults; do not substitute a naming cache or blanket early-stop rule for that evidence.

## 013 Compression Qualification

Contracts: objective, A2/A4-A7. Generate, test and independently measure raw-, gzip- and Brotli-selected release artifacts for every frozen competitive cell. Qualify applicable pinned competitor recipes under the same public/runtime contract. Missing/failed expected competitors require investigation; neither they nor losing workloads silently disappear.

For each boundary/profile/codec, require:

```text
size(lilscript_artifact_selected_for_codec)
    <= min(size(eligible_competitor_artifact_for_codec))
```

Use complete delivery and pinned codecs. Runtime eligibility on both sides binds to exact artifact, host, workload and startup/recurring/memory limits. Unknown required evidence blocks qualification. Diagnostic/related rows still meet their separately frozen behavior/quality requirements.

Group losses by lost facts, unavailable families, incompatible layouts, syntax/names/order, search allocation or packaging. Add bounded child tasks here with reproduction, owner, negative behavior cases and cost limit. Fix general mechanisms; recheck interactions and held-out cases. Update baselines deliberately as a new comparison contract, never selectively per losing row.

**Exit:** every required cell passes behavior, eligibility, per-codec no-loss and cost gates on the same compiler inputs. Independently replay recipes/hashes; report per-row bytes, wins/ties/losses and costs. Remaining losses keep the gate open. Measure strict-win percentages without inventing the threshold or claiming "almost always" is settled. Architecture completion alone cannot close a competitive loss.

## 014 Retirement and Final Certification

Contracts: A1-A7 and the objective. Make the service the normal route for every supported source/target/delivery mode. Complete declared configuration compatibility with actionable diagnostics. Remove obsolete optimizer/emitter/search owners, duplicate facts, generated-text semantic recovery, temporary adapters/selectors and development bypasses. Retain necessary native lowering and independent verification with explicit consumers.

Update language/configuration/current-architecture docs to actual behavior. Audit runtime routes as well as names: a new facade over old owners is not retirement. Trace a real library and the integrated 006 consumer through production. Native shares meaning and performs no JS codec search.

**Exit:** rebuild final source/dependency/configuration identities and rerun required repository, original library/native, per-codec competitor, effort/flag and compiler/runtime gates after deletion. Validate receipts and replay winners. Stale pre-retirement evidence, missing cases/logs, partial/noisy rows and support/admission bypasses cannot pass. Publish separate conclusions for architecture, correctness, cost and competitive size, with measured strict-win rates and scoped limitations. Keep the completed record here; do not create another migration tree.

## Work Records and Evidence

Before implementation, define a bounded task under its numbered section: ID, owner, source/content identity, files/interfaces, prerequisite receipts, assertions, runnable commands/resource limits and open decisions. Add finite child tasks there, not in another directory. Read only common rules, that section and relevant A/D contracts; aim for at most 2,500 words of planning context before focused source. One owner edits a task's files at a time; this protocol does not itself authorize spawning agents or starting old cloud jobs.

Receipts live under `benchmarks/migration-results/<run-id>/` when implementation begins. Record assertions/prerequisites; source/build/dependencies; workload/API/test inventory; configuration/policy; commands/exit codes/executed cases; generated/tested hashes; codec settings/scores; applicable wall/CPU/RSS/runtime evidence; logs and limitations. Bulky payloads may use durable content-addressed storage. A path, historical count or prose claim is not evidence.

Use explicit dependency manifests and hash installed code as well as lockfiles. Exclude receipts, progress, logs and unrelated docs from compiler-input digests so evidence cannot invalidate itself; hash acceptance contracts and payloads separately. Relevant source/policy/dependency/acceptance changes invalidate consumers. Mark affected checked milestones `stale` and uncheck them. Preserve failed/aborted receipts. Independent unchanged components may reuse qualified receipts only with dependency identities recorded.

A reviewer independently checks patches, assertions and receipt identities before marking verified; implementation prose alone is not proof. Keep handoffs compact: outcome, changed owners/files, commands/results, receipt identity, remaining gaps, invalidated evidence and next action. Final certification always uses the final production build. Documentation consolidation closes no compiler gate.

## Source Disposition

External archive root established during consolidation: `/home/azureuser/lilscript-planning-archive/`. `2026-09-18-reset/` held earlier plans/boards/designs and home copies; `2026-09-19-consolidation/before/` held exact documents removed or edited here, including the fourteen step files. These are historical records, not instructions to resume work, and are not colocated with this plan.

**Archive availability unresolved:** [September 19 audit](../../benchmarks/migration-results/2026-09-19-archive-audit/receipt.json) confirms successful preservation checks this morning, but the archive root is now missing and no relocation was found. Earlier partial backups and tracked Git history survive; they are not substitutes for the missing latest archives. The owner has been asked whether it moved. This preservation obligation remains open independently of compiler implementation.

| Retired source family | Retained here | Superseded |
|---|---|---|
| `docs/knowledge/migration/`, compression subplan, board and roadmap (September 18 archive) | Provenance, ABI/behavior admission, per-boundary comparisons, reusable fixes and deletion: 001/008/011/013/014 | Competing phase/board queues, five-fork completion scope and old continuation instructions |
| Root `migration/` target-tree plan (September 18 archive) | Lost-fact diagnosis, hygienic bindings/grammar, regressions and delete-versus-port review: 004/008/009 | Universal byte-identity gate, deterministic Brotli "noise", old fold counts/pins and cloud directives |
| Root architecture proposals and `migration plan.md` (September 18 archive) | Complete delivery, fair baselines, speed and reference retirement: 001/006/012/014 | Parallel target architecture, mandatory legacy-to-new adapters and strict-win-everywhere wording |
| `lilscript-finer-structured/` plans/designs and home copies (September 18 archive) | Value/reference lessons, choices, bounded facts/edits/search, native cases and historical receipts: 002/004/006/007/010 | Thirteen-step coordinator, prescribed layering, example-driver completion credit and old authorization text |
| Former `docs/migration/001-014` packets (September 19 archive) | Design-aligned requirements, stable IDs, dependencies and negative gates above | Separate checklists, repeated protocols and further planning packets |
| `docs/knowledge/research/aligned-mangling/PLAN.md` (September 19 archive) | Identity-before-renaming cases, reachability, codec ablations and negative research: 004/008/010/013 | Separate naming lane, historical live-bug status and unqualified naming/tie heuristics |
| `labs/vue-client/web/size-migration-plan.html` (September 19 archive) | Dynamic/getter boundaries, retained identity, fixed calls, reflection and packaging cases: 002/007/008/013 | Package-specific sequence, source-shape prescriptions and dated sizes as current acceptance |

Research, owner briefs, regression fixtures and evidence remain useful in their homes. They cannot assign active tasks or override the design. Normal navigation leads here, not to archived plans. Do not duplicate objective or architecture into another coordinator.

## Next Action

Latest sparse-narrowing inputs pass all 289 affected debug checks and the scoped 003-R1 release checkpoint: 303 distinct library checks, one CLI test and original consumers, with 15 unchanged JS/C deliveries. It follows condition-narrowing admission (284), binary continuations (277), Analyzer vectors (258 final), declaration vectors (348), constructor metadata ownership (298), unsafe assignment-sink retirement (611), canonical class members (409), parser lookahead (264), lexical work (246), canonical binding ownership (393) and module interface/schedule admission (298). Schedule 21 is current. Guard-free and fixed sparse-guard prefixes avoid repeated empty traversal, with current-scope lookup and diagnostics preserved, but the binary frame grows 64 to 80 bytes on the measured debug build. Dense guards and general traversal can still repeat work; no real-workload speed/RSS/size claim follows. The selected Mdast-to-Hast check also passes on the preserved September 20 release, with all five delivered files unchanged; no further library sweep is needed for this checkpoint. Use that explicit parent/hash for subsequent source-built work. General parser traversal/recursion/stack depth, escaping diagnostics, checker maps/nested payloads and remaining metadata ownership stay open.

001-MH-R1's changed-source Mdast replay is verified: both original builds, type checking, all 152 identities and separate package dry-run pass, with exact loaded/scored artifacts and stable inputs. Its original failure remains evidence, not an eligible size baseline. 001-RR verifies the original 21-case/type/package source-built boundary, and 001-Q1 shares the existing qualification implementation across data-only clients with 37 focused checks. 001-JQ freezes eight passing existing-dist identities, including live callback mutation/identity, but its broader original consumer fails in `Deferred.pipe` on those same bytes. The generic dynamic captured-arguments reductions pass on the current compiler, leaving historical pass/full-source attribution open; do not apply a speculative repair or rerun the unchanged failing artifact. Source-build configuration compatibility and jQuery's different delivery shape remain prerequisites. Continue remaining feature/caller and original-library inventory through existing owners. Remark, Mdast and Remark-Rehype still need broader CJS/installed-package/site/UMD coverage; Marked has 46 required identities beyond its retained-open supplement, and Probe has twelve incomplete matrix rows. Do not translate historical flags for a pass or repeat the full profile sweep per compiler edit.

001-MM freezes all 1,963 original Micromark identities with stable retained-distribution loads; its four-invocation source build and historical `name_ordering` setting remain explicit prerequisites. 001-MO freezes eleven original non-browser Motion identities from `/tmp/motionlil-cost-audit-20260911/current`, with dependency recovery explicit; ten separate geometry observations pass against upstream/retained candidate. Both inventories pass the existing consumer without a library rerun. Preserve Motion's unexecuted browser/type obligations, missing Playwright and nine-invocation source-build boundary. These caller observations strengthen 002; the explicit-boundary owner question remains unanswered rather than implicitly approved.

Existing 004-P1/P2 and scoped 006 witnesses remain useful, but prerequisites and D2/D5 still prevent closure. Resolve Motion/Micromark public semantics, the finite acceptance/cost policy and archive availability. Broad emission cost and measured quality per effort must guide search work; pooling/naming diagnostics do not justify more default Brotli effort. 006-P4 now quantifies a real staged-scoring tradeoff: tight-probe Brotli improves while rendering/work grows, and extra probes cannot repair an evicted combination. 006-P5 makes reclamation admission precise without changing that finite search outcome; it does not reduce measured emission cost. Carry those costs into held-out measurements before changing defaults; preserve snapshot-qualified proof ownership when adding equivalent-rewrite rediscovery. These bounded receipts authorize neither broad language migration nor milestone closure; final public/library qualification remains required.

012-P1 exposes real semantic demand/formation/naming/printing and canonical codec durations through the existing opt-in telemetry. 012-P2 now confirms codec dominance on the current production CLI's unchanged 12-module fixture and measures a useful staged-scoring quality/cost curve. Keep that evidence distinct from whole libraries and large legacy Marked. Use the same preserved production binary on frozen independent semantic workloads and compare bounded interaction retention before adopting default budgets or adaptive ranking. Local plateaus do not prove combinations exhausted. Preserve exact quality/contract comparisons, failed evidence and independent raw/gzip/Brotli objectives; the current CLI study optimizes Brotli only.

001-GF freezes all 22 original GFM identities and its unchanged `check:types` prerequisite, with exact retained-distribution loads and protected fixture expectations. 001-Q2 guards fixture pins and shares complete dependency snapshots. 001-Q3 now consumes the exact original pinned prerequisite, with eleven passing focused checks and no Rust rebuild. GFM's `terminal_cleanup_chain` and `wide_single_use_collapse` come from sibling `migration/target-tree`; current-family equivalence is not established. Define their legal intents through common checked families and explicit configuration compatibility before original source qualification. Do not silently alias/drop flags, restore duplicate optimization owners or rename scripts for a pass. 003-C8 removes owned type-argument copies and passes 260 affected checks, but nested type/map accounting and comprehensive traversal remain open. Continue remaining original-library inventories and frontend ownership work independently; no broad fleet rerun or extra default Brotli effort is justified by these receipts.
