# Migration plan: smaller programs under the chosen objective

Replanned with the owner on 2026-09-29. This is the active design and implementation plan. [index.md](index.md) is the working checklist; [history.md](history.md) records changes and measurements. [coverage.md](coverage.md) assigns the unfinished work from the previous plan. The previous design, plans and batch records are in [old-history](../old-history/README.md).

This plan continues the existing compiler. It does not start another compiler route or require a new IR. Statements below describe the target unless explicitly marked **implemented**; [configuration.md](../configuration.md), source and tests describe the interface available today.

## 1. Objective and boundaries

For each supported application or library boundary, produce the smallest correct **delivered artifact under the selected objective**, within the chosen compilation effort and declared runtime requirements. JavaScript is the primary target. The same checked program must also support native execution through shared semantic optimization.

Raw, gzip and Brotli are separate objectives. Each has its own incumbent, policy and result. A multi-objective request returns separately optimized artifacts; it does not average the scores or claim one file is optimal for all three. Codec version, quality, window and mode are part of the objective.

The cost is the final output the consumer receives: names, helpers, wrappers, imports, exports and required data included. Compress each independently served file with the configured codec, not an artificial concatenation. Multi-entry and lazy delivery use a declared, frozen loading model; charge shared files once within a load and report package bytes separately. Explicit request/depth costs are reported separately from byte counts. Source maps and other optional files are counted in the delivery profiles that actually serve them.

Correct behavior, public ABI, evaluation order, exceptions, identity, initialization and permitted reflection constrain the alternatives. Runtime parity with the original library remains the default product requirement, measured on declared workloads including startup and relevant memory behavior. Explicitly permitted runtime tradeoffs must be recorded; increasing effort must not silently grant them.

**The optimum is bounded by available knowledge, candidates and compilation work.** Exact compression determines the score of a candidate, not the global optimum. More effort should explore more useful alternatives while retaining the best eligible result. There is no promise to find the smallest possible program or beat every arbitrary JavaScript input.

The qualification goal remains ambitious and concrete: match or beat the strongest eligible pinned toolchain, including Closure ADVANCED, under each objective on the maintained and held-out boundaries, with meaningful wins and no hidden losses. Totals guide development; final qualification also exposes every individual cell.

## 2. Architecture and ownership

| Layer | Owns | Required boundary |
|---|---|---|
| Configuration and checking | Resolved policy, types, identities, exports, reflection and target capabilities | Resolve once; unknown keys fail; assumptions are explicit |
| Typed Program IR | Operations, regions, values, cells, calls and source meaning | Stable identities; one checked edit/invalidation protocol |
| Shared facts | Calls, effects, values, initialization, escape, fields and uses | Sound answers or explicit unknown/truncated results; no guessing from emitted text |
| Semantic transformations | Folding, removal, propagation, specialization, inlining and scalar replacement | Preserve meaning; update dependent facts and reconsider affected work |
| Target formation | JavaScript/native representation, runtime helpers, target legality and provenance | Consume shared facts; avoid emitting ceremony only to rediscover its redundancy |
| Representation and naming | Layout, sharing, spelling, data encoding, symbol allocation and delivery placement | Registered legal alternatives, permissions, estimates and coupling |
| Admission and publication | Verification, complete rendered bytes, exact objective score and receipts | Scored bytes equal delivered bytes; all routes enforce the same policy |

Facts belong to their producer and travel through typed identities and annotations. A fact is fully integrated when its consumers use it and the replaced analyses are removed. Existing transitional passes remain until their replacements preserve coverage and pass the relevant checks; replacement and deletion land together.

Use dirty units and dependency invalidation rather than repeating every analysis after each move. Recursive call groups and dataflow joins need sound convergence rules. Analysis limits yield conservative unknown facts; they never legalize an otherwise unproved rewrite. Keep counters for work that can grow with calls, fields, contexts or candidate combinations.

## 3. Configuration is a product contract

The discovered file remains `lilscript.toml`; `--config path/to/config.toml` selects an explicit file. Configuration must control behavior predictably, and `--print-policy` must explain the effective request, its provenance, permissions, work limits and compatibility warnings.

This is a **currently accepted configuration shape**, not the full planned schema:

```toml
[objective]
codecs = ["brotli"]

[objective.brotli]
quality = 11
window = 22
mode = "generic"

[objective.gzip]
level = 9
window = 15

[effort]
level = 13

[policy.tactics]
inlining = "auto"
string-pooling = "off"
```

The parser admits any nonempty, duplicate-free objective set. CLI and Rust builds independently resolve and search the requested codecs over one checked program, with all mandatory baselines admitted before optional work. The CLI requires an explicit output directory for several results and writes complete codec subdirectories with a combined policy/hash manifest. Single-objective delivery remains compatible. Terminal string pooling/packing and fixed target-rule vetoes are enforced; C1 has qualified current producer/risk enforcement and compatibility. A parsed setting is not evidence that every path obeys it.

| Axis | Meaning and target behavior |
|---|---|
| Program contract | Application/library boundary, script/module semantics, public names, output format, ECMAScript floor, development checks and explicit host assumptions. These decide legality, independently of effort. Complete the current CLI/TOML mapping rather than introduce duplicate controls |
| Objective | `raw`, `gzip` or `brotli`, with actual encoder settings. Distinguish optimizing emitted text from optionally writing compressed delivery files |
| Effort | Keep the existing numeric interface, default 13. Lower levels favor compilation speed; default and higher levels favor smaller output through more exploration. It controls work, not source meaning |
| Family permissions | Retain `[policy.tactics]` as the canonical interface and generate its reference from the registry. Split umbrella tactics into meaningful behaviors as their producers migrate |
| Runtime requirements | A documented performance policy, completed with C2, covering steady state, startup and relevant memory constraints. Risk permissions are explicit and do not assert measured parity |
| Resources | Hard memory/deadline limits are distinct from normal search budgets. Exhausted search returns the best admitted incumbent; an inability to satisfy a hard limit fails clearly, without silently changing the requested contract |
| Execution and reuse | Threads and transparent caches may change cost, not bytes. Any optional decision lock that intentionally supplies a prior result is explicit and fingerprinted |

For every optional family, `off` is a hard veto, `on` permits competition without forcing a representation, and `auto` uses the objective's policy within the permitted runtime risk. The veto applies to initial formation, cleanup, search, replay, caches and every requested objective. Mandatory language lowering and validation remain mandatory; an optional optimization cannot bypass its veto by being renamed cleanup.

One registry supplies family identity, availability, prerequisites, legality, risk, policy setting, invalidation and receipt names. Reject or clearly warn about unsupported controls. Names with no producer may remain only as diagnosed compatibility entries, never as advertised implemented optimizations; new families land with their producers. Test disabling a family on a program that would otherwise use it, including interactions with other families and cached candidates.

Configuration precedence must be documented and tested from file discovery through explicit configuration and CLI overrides. Contradictory aliases fail instead of depending on load order. Fingerprint every byte-affecting setting, semantic assumption and schedule/codec version. Receipts distinguish a disabled family, an inapplicable family, a rejected candidate and a budget that never reached the candidate.

Every optimization control follows the [configuration documentation guideline](../configuration.md#guideline-for-optimization-controls): describe its purpose and useful situation, accepted values/default, effort gate, availability and prerequisites, size/compilation/runtime tradeoffs, veto/override behavior and versioned evidence. Keep semantic assumptions distinct from cost controls. Do not introduce an unexplained switch or imply that permitting an alternative forces a win.

**Effort defaults:** retain 13 as the size-focused default. Useful exploration with substantial cost and consistently tiny measured returns belongs at 14 or 15 by default, with explicit opt-in/out where appropriate. Apply that decision per measured strategy; do not remove useful exploration merely because it is expensive or infer a global bytes-per-second threshold from one workload. The deferred naming tail now follows this rule at 14, with an explicit level-13 opt-in.

**Compatibility:** explicit policy version 3 separates runtime permission from effort. Omitted versions and version 2 retain the diagnosed level-16 startup grant; migration deliberately selects version 3 and explicitly permits any accepted reconstruction cost. Retain useful existing aliases, reject contradictions, and change schemas with a tested migration. Per-library optimizer thresholds and strategy scripts are not the default configuration surface: general heuristics belong in calibrated, versioned compiler policy.

## 4. Proofs, defaults and choices

Keep three questions separate:

1. **Is the transformation legal?** Prove it from the language, boundaries and facts. Compression measurements cannot answer this.
2. **Is it a good default?** Establish this with calibration and regression evidence under all objectives. A raw-length bound or removal of an operation is not a proof of compressed-size improvement.
3. **Is this candidate better for this request?** Compare complete admitted artifacts with the configured objective when the representation warrants exploration.

Compute exact semantic facts even when the best printed representation is uncertain. Knowing a call's result may remove a branch without forcing every use to expand into a large literal. Destructive canonicalization must not erase valuable representation alternatives before their owning family can consider them.

Use inexpensive, well-supported defaults for ordinary simplifications. Do not make every arithmetic fold a codec trial. Register optional rewrites, measure their effects, and move materially objective-sensitive or interacting representations into families. Group such decisions by meaningful sites or shared structure so the search remains bounded. A calibration result supports a default; it does not establish a universal sign theorem.

Known-method and pure-function evaluation must implement the language's exact arithmetic, strings, exceptions and evaluation order. Purity alone does not prove termination or engine-independent floating-point results. Bound recursive evaluation and specialization; unsupported or exhausted evaluation leaves the original operation available.

## 5. Mangling and compressible code generation

Naming is part of representation selection. It must see the bindings and helpers that survive optimization and the scopes and files that will actually print.

| Objective | Candidate priorities to calibrate |
|---|---|
| Raw | Name length weighted by emitted uses; interference-aware short-name reuse; compact syntax; net helper, literal and property costs |
| Gzip | Repeated token sequences, reusable names, declaration/function/data order within legal initialization groups, locality within the configured window |
| Brotli | Repeated structure and spelling, scope reuse, alphabets and token contexts, literals versus references, shared versus specialized text under the configured encoder |

These are candidate-generation considerations, not hardcoded claims that a spelling or alphabet always wins. Gzip gets its own measured policy instead of inheriting Brotli's. All legal representations remain eligible under every objective.

**Lexical names:** one hygienic allocator with an explicit interference model, including captures and shadowing. Reuse names across non-interfering scopes. Offer frequency-based short-name assignment and stable printed-order assignment; measure reusable spelling across structurally similar functions. Local moves should preserve unrelated names where possible. Global frequency rankings and alphabet changes are explicit joint moves because they can rename the whole artifact.

**Properties:** rename by private field identity and proved observability. Respect extern/catalog names, exported shapes, computed accesses, enumeration, serialization, inheritance and reflection. Reuse a property name across unrelated types only when those fields cannot be confused at an access. Unknown dynamic observations preserve names. Open-world libraries still have private implementation fields; public property mangling requires an explicitly compatible boundary.

**Coupling:** assess names together with affected layouts, helper sharing, literal placement and spelling. A name-plan tie is decided on final delivered bytes with a deterministic tie rule. Never accept a gain measured before a later global rename changes the candidate.

Use the existing [mangling research](../knowledge/research/README.md) as hypotheses and negative evidence. Some experiments used illegal mutations or the deleted compiler: re-establish legality and measure current outputs before adopting a heuristic. Dictionary words, fixed favorite letters, padding and entropy estimates are not correctness or size guarantees.

## 6. Bounded search and compilation cost

One candidate mechanism must ultimately carry structural representations, inlining/sharing, data encodings, naming, spelling and placement. Each family declares its sites, legal alternatives, affected dependencies, runtime risk, estimates and useful joint moves. This replaces parallel bespoke search paths incrementally, preserving their measured opportunities.

For each objective and resolved policy:

1. Check once; compute facts and run the permitted default simplifications. Form a legal baseline using that objective's calibrated defaults and naming seed.
2. Admit the baseline and retain it as an incumbent. Level 0 requires no codec trials for selection; explicitly requested size reporting may still compress once.
3. Rank legal opportunities by estimated benefit and work, using the names, helpers and delivery boundaries they affect. Use deterministic tie rules and versioned policy.
4. Apply a proposed move to shared state; update affected facts, simplify the dirty units, form, name and render the actual candidate.
5. Use cheap estimates or a proxy to avoid clearly unpromising exact evaluations. Calibrate false negatives on a no-prune lane. An estimate or proxy never promotes the incumbent.
6. Admit and score the complete candidate with the exact configured objective. Replace the incumbent only on improvement; equal scores use a stable tie policy.
7. Spend additional effort on site refinement, useful joint moves and bounded alternative starts. At higher effort, a bounded exploratory state may be temporarily worse to reach a better combination; the best admitted incumbent remains protected.
8. Stop when the counted work budget or move space is exhausted. Deliver the best admitted artifact and its receipt, with no unscored transformation afterward.

Independent one-change improvements are insufficient for all programs. Examples of joint opportunities are inline → propagate constants → delete the callee; layout → remove allocations → rename fields; and pool literals → include a decoder → choose its names. Joint moves must encompass the resulting cleanup and runtime costs.

For a fixed compiler and schedule, with the same contract, permissions and objective settings, higher effort should extend the lower effort's deterministic work and retain its result, so it cannot lose through budget rescheduling. If a new strategy cannot share that prefix, offer its result alongside the retained incumbent. Changes between compiler versions are governed by the batch gates, including the owner's allowance for small local heuristic regressions when totals improve. This distinction avoids promising global optimality.

Keep budgets understandable: proposal/analysis work and exact judgments, with counted resource limits. Bound combinatorial expansion, recursive evaluation and retained candidates. Report analysis, formation, naming, printing, proxy, exact-codec time and cache hits separately. Do not impose one seconds target on differently sized programs.

Reuse facts and target units by revision. Memoize compression by final byte digest plus encoder identity/settings; reuse unchanged files within a delivery candidate. Include policy, analysis bounds and semantic identities in earlier-stage caches and revalidate candidate eligibility. Cache hits preserve logical work accounting and candidate order, so a warm cache cannot secretly buy a different search at the same effort; physical savings are reported separately. Equal compressed sizes do not imply equivalent candidate states. Sharing work across objective requests must not share their winner or use an incompatible score.

Avoid adding search before giving it useful alternatives. Improve estimates and reuse before buying more identical evaluations. Calibrate priors and estimates by family, structural site class, objective and relevant encoder/size regime, never by library identity. Measure where extra effort earns bytes across generic programs and validate on held-out workloads. Cross-objective seed borrowing and more elaborate search remain measured experiments, not mandatory extra production passes.

## 7. Work packages and dependencies

These are completion milestones. Work on one milestone until its deliverable and completion evidence are satisfied, then advance to the next. Several implementation commits may belong to one milestone; they do not justify opening another. Existing partial implementations are inputs to their milestone, not a reason to keep interleaving packages. The detailed legacy ownership is in [coverage.md](coverage.md).

### C. Controls and evidence

| Task | Deliverable | Completion evidence |
|---|---|---|
| <a id="c1"></a>C1 | Family registry, complete permissions, coherent TOML/CLI precedence, meaningful receipts and compatibility migration | Veto cases cover direct, searched and cached output; no silently ineffective setting; schema/reference agree |
| <a id="c2"></a>C2 | Independent oracles and feature coverage; pinned fair benchmark contracts; runtime counters and paired perf runner | Same behavior oracle on both sides; runtime controls and sample spread retained; new optimization has the needed oracle before landing |
| <a id="c3"></a>C3 | Medium-size generic calibration set, protected held-out set, per-objective baselines and compile-work reports | Leave-one-out validation, no-prune audit, recorded prior exposure, no tuning on held-out failures |

### S. Semantic leverage

| Task | Deliverable | Completion evidence |
|---|---|---|
| <a id="s1"></a>S1 | Known-method folds, bounded evaluation of pure constant calls, value propagation, defaults/returns and dead-work cleanup | Exact edge-case oracles; calls/branches/bindings removed on generic cases; all three objectives reported |
| <a id="s2"></a>S2 | Escape, uniqueness, field facts, scalar replacement, store-copy elision and private namespace/object flattening | Aliases, identity, effects and value-copy semantics preserved; fewer allocations/stores; relevant runtime lanes pass |
| <a id="s3"></a>S3 | Remaining inlining, devirtualization, specialization and identical-unit opportunities | Cross-module calls, nested closures and tail returns covered; duplicating/sharing alternatives feed Q1 rather than an unconditional raw-size rule |
| <a id="s4"></a>S4 | Remaining checker identities, fact transport, operation catalog and language contracts that expose safe optimizations | Facts consumed by both target layers where applicable; obsolete derivations removed; boundary and capability cases cover each added feature |

S1 starts with the existing facts. S2 builds the ownership/field facts its transformations need. S3 uses those facts and Q1 for objective-sensitive alternatives. S4 is delivered with its consumers, including shapes, records, sealed calls, enums, const data and host/generic support; it is not an excuse to postpone immediately usable work.

### G. Naming and JavaScript generation

| Task | Deliverable | Completion evidence |
|---|---|---|
| <a id="g1"></a>G1 | Scope/interference allocator, local reuse, printed-order/frequency seeds and joint alphabet moves | Hygiene and capture cases; final-byte comparisons per objective; unrelated scopes remain stable under local seeds |
| <a id="g2"></a>G2 | Private property renaming and safe name reuse across types | Reflection/extern/computed-access cases; matched open and closed benchmark lanes; public names preserved |
| <a id="g3"></a>G3 | Per-site spelling choices, legal declaration/function order, literal placement and a structural printer | Syntax/edition and initialization checks; gzip-specific locality trials; interactions with naming judged together |

G1 can proceed on the current target tree. G2 depends on the reflected set and C2's boundary contract, not on eliminating every Closure loss. G3 uses Q1's incremental family interface. No naming or property milestone waits for a structural-only compiler to beat Closure first.

### Q. Choices, data and efficient exploration

| Task | Deliverable | Completion evidence |
|---|---|---|
| <a id="q1"></a>Q1 | One representation-family interface with per-site and joint moves; migrate structural/layout/inline/shared-helper opportunities | Old opportunity inventory preserved; one admission path; bespoke mechanisms deleted with tested replacements |
| <a id="q2"></a>Q2 | Dirty scheduling, shared formation, exact-score memo, resource accounting and transparent caches | Repeated-state and unchanged-file work avoided; output identity with caches/threads varied; scaling counts and retained bytes reported |
| <a id="q3"></a>Q3 | Independent raw/gzip/Brotli policies, calibrated estimates/proxies and useful effort tiers | C3 calibration; bounded exploration; protected incumbents across levels; bytes earned per added judgment and CPU cost |
| <a id="q4"></a>Q4 | Const data, helper ownership, pooling/encoding/layout choices and bounded specialization over static schemas | Full decoder/helper bytes counted; data exactness checked; startup, memory and hot-path costs measured; all family permissions honored |

Q2 starts from the repeated formation/scoring already observed in markedlil. Q1 migrates a family when its next useful optimization needs the common interface. Q3 does not require every Q1 family to exist first. Q4 needs the relevant const/identity facts from S4 and runtime coverage from C2. More complex search must demonstrate extra opportunities and a useful tradeoff.

### D. Delivery and integration

| Task | Deliverable | Completion evidence |
|---|---|---|
| <a id="d1"></a>D1 | ESM/CJS/IIFE/UMD/bare formats, private application wrapping, delivery contracts and manifest cleanup | Y5's IIFE default completed; script strictness kept distinct; exports, cycles and host-entry cases pass |
| <a id="d2"></a>D2 | Consumer-shakeable libraries, multi-entry/lazy placement, host modules and source maps | Correct initialization and dependency closure; consumer bundles measured; every scored file matches its delivered tree |
| <a id="d3"></a>D3 | Shared build/check API, independent multi-objective results and port build migration | CLI/LSP/lint share the intended module graph; compiler-written delivery, no post-minifier; source/config/artifact receipts reproduce |

D1 begins from the existing delivery work. D2's placement choices use Q1 when available; basic correctness and formats do not wait for search. D3 migrates ports as their required formats become available, rather than leaving all delivery evidence until the end.

### V. Qualification and release

| Task | Deliverable | Completion evidence |
|---|---|---|
| <a id="v1"></a>V1 | Close the size, correctness and runtime gaps on maintained and held-out boundaries | Separate raw/gzip/Brotli matrices against eligible competitors, including Closure ADVANCED; each residual has an owning task until closed |
| <a id="v2"></a>V2 | Reproducible releases and evidence consumed by packages/sites | Pinned compiler/source/config/dependencies; clean rebuild; current receipts and supported-language/format declarations |

V1 runs throughout S/G/Q/D and is the final JavaScript qualification milestone. Each loss is classified by missing semantic work, representation, naming, delivery, runtime or benchmark mismatch. Correct mismatches without counting reduced behavior as a compiler win. V2 follows qualification of the artifacts being published.

### N. Native continuation

| Task | Deliverable | Completion evidence |
|---|---|---|
| <a id="n1"></a>N1 | Shared optimized program feeding native, one toolchain owner, explicit capability/ABI matrix | Same semantic cases across targets; no hidden native-only refusal after checking; diagnostics retain source spans |
| <a id="n2"></a>N2 | Remaining native language/runtime/host support and performance qualification | Portable records, externs, exceptions/suspension/regex/strings, memory cycles and ABI cases; sanitizer and runtime comparisons |

Native remains an owned commitment. Its completion does not gate JavaScript naming or compression work. Shared semantic changes cover both supported targets as they land; native-complete and JavaScript-qualified are separate statuses until both are achieved.

## 8. Working order

Complete milestones in this dependency order:

1. **C1 → C2 → C3:** close configuration and permission enforcement, independent evidence, then calibration infrastructure.
2. **S1 → S2 → S3:** finish folding, object optimization and call optimization, including each one's required shared facts and removal of its replaced passes.
3. **Q1 → G1 → G2 → G3 → S4:** finish the representation interface, lexical names, property names and target spelling, then close the remaining shared contracts/fact-consumer audit. S4's deletions depend on those consumers; completing all of S4 first would invert that dependency.
4. **Q2 → Q3 → Q4:** complete reuse/accounting, calibrate independent objective/effort schedules, then finish data/helper representations.
5. **D1 → D2 → D3:** finish formats, consumer delivery and shared build/port integration.
6. **N1 → N2 → V1 → V2:** complete native integration and coverage, close remaining qualification gaps, then qualify reproducible releases.

Only one milestone is active. Record its finite acceptance checklist before changing implementation. A prerequisite discovered within a milestone belongs to that milestone's completion work; do not use it to begin several more packages. Newly implemented families must register their controls as part of their own milestone; this does not keep C1 permanently open. Future language or product proposals without an approved contract are not silent completion requirements: resolve them explicitly in the plan under the existing semantics.

Prefer a measured missing capability over another layer of orchestration. Change scope or order only when the existing requirement conflicts with the product objective or a real dependency requires it, and record the reason. Do not delete a requirement merely because implementation is difficult.

## 9. Verification and acceptance

| Concern | Acceptance rule |
|---|---|
| Correctness | Independent oracles, relevant unit/case/port suites, ABI/observability cases and output admission. No new unaccounted wrong program; expected failures have explicit owners and never count as wins |
| Configuration | Requested permissions, contract and codec settings hold on every path, including replay and caches; receipts explain actual behavior |
| Size per batch | Report aggregate deltas separately for raw, gzip and Brotli on frozen matched inputs. Small local growth may accompany smaller totals, as the owner ruled. Never hide one objective's regression in a blended score; investigate material individual regressions and record the decision |
| Default and higher effort | Protect size quality. A rare small heuristic regression between compiler versions is explicit and assessed with the totals; it cannot justify silently removing useful search just to make compilation faster |
| Runtime | Paired fresh-process measurements with controls and retained samples for affected workloads; warm-up, steady state, startup and memory as relevant. Static risk classification is a screen, not proof of parity |
| Compilation | Report counts and phase CPU costs on identical input/policy, with repeated alternating pairs for timing claims. Check scaling and bytes bought by extra work; no universal seconds target |
| Generalization | Calibrate on generic training programs; validate on independent programs. Inspecting a held-out failure moves it to development and requires a replacement held-out case |
| Delivery | Actual compiler-written artifacts, no post-minifier. Same consumer loading model, dependencies and API surface on both sides; exact receipt identities |

Compression is deterministic for fixed bytes, encoder and settings. Record exact deltas. Source/name perturbations measure robustness of a heuristic; they are not encoder measurement noise. Do not import the old universal 100/150/400-byte thresholds as laws. Keep win labels and historical thresholds versioned and separate from exact comparisons; C3 defines any replacement from evidence before changing published verdicts.

C2 pins toolchains and an eligible recipe grid for Closure ADVANCED, Terser, Oxc, SWC, Rolldown, esbuild and upstream artifacts. Match language guarantees, externs, reflection and exported functionality. Report open libraries, closed applications and representative consumer bundles separately. A tool that cannot support the boundary is inapplicable, with a reason, not a zero-byte bar.

Use one heavy building/testing/measuring job at a time on this host. Implement a coherent group of changes before building and testing; do not run tests after every edit. Batch the relevant oracles and regressions at implementation checkpoints, and run the full fleet and held-out qualification at milestone completion. A suspected soundness defect can justify an earlier focused check. Broaden or repeat checks only for new changes, failures or unresolved evidence.

Before implementation, record the batch's numbered changes, predicted size/work/runtime effects and verification scope. Keep changes attributable in separate commits. When a result surprises, use those changes or a real family veto to isolate it. Record the actual compiler/config/source identities, findings and unfinished work once in history.

A task is complete when its behavior works through public entry points, its permissions and interactions are covered, its replacement removes obsolete code, and its result is measured with the appropriate gates. Parsing a flag, computing an unused fact or winning one hand-picked file is partial progress.

## 10. Language compatibility and plan maintenance

The approved trusted-crossing contract (Y1), private application roots/frames (Y5a/b) and default application IIFE (Y5c) remain the starting point. This planning revision does not silently change `ref`, exported function constructibility or whether `pure` promises termination; the old Y2/Y3/Y4 decisions stay explicit in S4. Optimize under existing semantics while the affected language change is specified separately.

Keep language rule identities and old task identifiers usable in references. [coverage.md](coverage.md) maps unfinished M-tasks and carried defects to these work packages; historical numbering is not a second execution order. Current language/configuration documentation must distinguish accepted behavior from planned behavior.

Only the checklist holds live progress. The plan holds decisions, dependencies and acceptance; history holds evidence. Amend this plan directly when the owner changes the goal or new evidence changes a mechanism, with a short record of why. Archived instructions, schedules and absolute performance claims are historical evidence, not an additional authority over this plan.
