# Compression migration

Status: design-first subplan for compression work derived from the Closure
comparison at commit `73eee24` and the 2026-08-29 LilScript tree. Parent:
[planned migration](planned-migration.md). Architecture:
[planned architecture](../compilation/planned-architecture.md). Research:
[`differences/`](../../../differences/index.md). Live status:
[board ledger](board/LEDGER.md).

This document refines only phases 3-6 of the canonical migration. Phases 0,
0.5, 1, and 2 in `planned-migration.md` are prerequisites, not work to rename or
repeat here. If this file conflicts with the canonical migration, planned
architecture, language contract, or ledger, those authorities win.

## Goal

Make LilScript programs smaller because source semantics and typed analysis
prove representations legal before JavaScript is chosen, then compare legal
complete artifacts under the selected `raw`, `gzip`, or `brotli` objective.

Closure is a source of candidate generators, safety counterexamples, and pass
interactions. It is not the architecture or cost model to reproduce. LilScript
retains typed ownership/effect/escape/identity facts, a known-valid incumbent,
bounded deterministic work, exact selected-codec scoring, and explicit ABI and
host boundaries.

The canonical phase-6 completion criterion is the only completion criterion for
this subplan. The candidate lists below are evidence-gated opportunities, not a
promise to implement every Closure feature. An opportunity that has not met its
admission gate remains research and does not block release. Once admitted to the
board, it ends as landed, rejected with retained evidence, or ineligible with a
reusable proof reason.

## Invariants

1. Language semantics, explicit source intent, host assumptions, identity, and
   ABI are hard constraints. Codec bytes cannot make a rewrite legal.
2. One immutable compilation contract is constructed before profitability
   work. The objective may select a private representation but may not alter
   source typing, effects, logical values, public names, callable behavior,
   descriptors, module behavior, or host ABI.
3. Raw, gzip, and Brotli compiles of the same program and build profile must
   derive the same semantic and expected-ABI fingerprint. Each final artifact
   must independently match it.
4. New source syntax states reusable semantics. It never names a package,
   minifier pattern, codec, or JavaScript spelling, and its optimizer-disabled
   meaning is complete.
5. Typed IR and conservative analyses authorize transformations. Missing proof
   rejects an alternative; it never silently enables pure getters, pristine
   prototypes, non-proxy values, or whole-world host assumptions.
6. JavaScript-specific contractions operate on the minimal hygienic target-JS
   tree. Final-byte parsing checks the result but is not the source of binding,
   property, or ABI identity.
7. Every profitability family retains and validates its configured incumbent.
   Mandatory correctness, ABI, explicit lowering, and unsafe-policy decisions
   are not scored alternatives.
8. Local raw estimates and codec predictors may schedule work. Only the pinned
   scorer over the complete `Artifact` or declared `ArtifactSet` selects a
   winner.
9. Search is bounded and reports evaluated, rejected, unvisited, and starved
   work plus its stop reason. Production reports `best-observed`, not a global
   optimum.
10. No package/path matcher, port-shaped fold, host trampoline, hidden
    post-minifier, or source construct whose purpose is to mimic Terser output.
11. Compile time, peak memory, startup, allocation, and runtime costs remain
    reported tradeoffs. More candidates or infrastructure are not inherently
    better.

## Implemented Baseline

Do not schedule these as missing language work:

- `struct`, identity-free `class`, `enum` and exhaustive scalar `match`,
  expression `if`, closed `object O`, null-prototype `Record<T>`, and
  `export constructor` already provide distinct semantic contracts;
- array and `Record<T>` destructuring, including rest, already have parser,
  semantic, typed-IR lowering, and contract documentation;
- ordinary `object{...}` already means a JavaScript ordinary-prototype
  `JsValue`; it is not a typed homogeneous dictionary and currently rejects
  spread;
- typed IR carries operation provenance, source `|0` obligations,
  owner-qualified field slots, class identity, capture identity, effects, and
  escape facts;
- the compression IR already implements path-sensitive constant propagation,
  pure-expression superoptimization, partial-escape allocation sinking,
  `map().map()` pipeline fusion, repeated-region outlining, and two bounded
  private-function merging forms;
- reachability and inlining already converge together before compression and
  run again after compression, with unused parameters/returns and scalar
  simplification revisited; the compiler also scores an all-compress-off
  contrast, selected fusion/outlining/merging contrasts, and one bounded
  outline/inlining/helper interaction;
- closed-record projection already folds known field/missing reads, constant
  `JSON.stringify`, and `Object.keys(record).join(...)` through an explicit key
  array; remaining record-observer work must not reimplement those cases;
- the compilation contract and source-derived ABI manifest are partial seeds,
  not an observed final-artifact ABI gate;
- the decision registry is a census with a small migrated `DecisionSpec`
  subset, while some optimizer probes and terminal acceptance remain imperative;
- generated JavaScript is still string-produced and reparsed by target folds;
  there is no hygienic target-JS tree yet.

Ports use the implemented forms before requesting syntax. A mechanical
`JsValue` translation is evidence to classify, not permission for a peephole.

## Canonical Sequence

| Canonical phase | Compression responsibility | Exit before advancing |
|---|---|---|
| 0: evidence | Freeze compiler/source/config/codec/harness/artifact fingerprints and replayable selected recipes for MotionLil, MarkedLil, MobXLil, jQueryLil, and SolidLil. | The five-fork canonical phase-0 evidence and replay gates pass. |
| 0.5: legality and ABI | Freeze expected ABI; validate final bytes for syntax, bindings, property categories, module links, ABI, and lowering obligations before scoring. Disable any terminal rewrite that cannot pass. | One validator path accepts incumbent and challengers; objective builds share the expected contract and the five-fork checkpoint is green. |
| 1: incumbent recovery | Recover each still-legal maintained-library incumbent before adding optimization infrastructure. | Every listed regression is restored or proven ineligible under current semantics/ABI, with no new selected-metric regression in any of the five forks. |
| 2: decision consolidation | Move existing imperative probes into registry-owned recipes and one acceptance path without changing the reachable legal artifact set. | Consolidation is byte-identical where promised, every family retains its incumbent, and the five-fork checkpoint is green. |
| 3: target JS | Introduce the minimal hygienic target representation and migrate identity-sensitive text rewrites incrementally. | No migrated family recovers identity from spelling; each replacement passes behavior, objective, five-fork, and resource gates. |
| 4: reusable proofs | Close classified language/proof gaps with complete semantics and direct baseline lowering. | The feature is reusable, its ABI is fixed, optimization remains optional, and all five forks retain their supported boundaries. |
| 5: measured search | Add only alternatives or scheduling work justified by a minimized miss or fingerprinted corpus result. | The measured miss is reached within an accepted resource budget or the attempt is retained as negative evidence; the five-fork checkpoint has no selected-metric regression. |
| 6: retirement and release | Delete only superseded paths and certify corpus-scoped claims. | The canonical phase-6 gates and the full five-fork release matrix pass. |

Phase 2 precedes phase 3. Dirty-region scheduling, outer fixed points, property
coloring, and new syntax are not phase-2 consolidation work.

Before phase 3 begins, these phase-0.5 correctness units close known current
hazards. They are not compression opportunities and cannot be delayed behind a
size win:

| Unit | Semantic layer | Retained incumbent | Objective decision | Safety proof | Verification boundary |
|---|---|---|---|---|---|
| V-01 final-artifact admission | Printed JavaScript and declared `ArtifactSet` | Current configured bytes, independently parsed | Mandatory validation before any codec call; no scored alternative | Every emitted binding/external, property category, module edge, ABI element, syntax feature, and live lowering obligation has an expected identity and observed witness | Malformed, unresolved, miscategorized, ABI-drifting, or obligation-dropping bytes are rejected before scorer instrumentation records a call |
| V-02 terminal rename closure | Generated-JS bindings until target identity replaces them | The unremapped artifact and every already-valid resolved remap | Exact selected-codec ranking only after admission | Every rewritten occurrence resolves to the same declaration; whole-artifact remaps require total resolution; fixed descendant function/class names are reserved; name generation returns exhaustion instead of indexing beyond its alphabet | Adversarial nested/destructured/default/catch/class scopes, free globals, shorthand/labels/templates, more than two-character name capacity, and final binding resolution |
| V-03 ordinary-object construction | Typed IR when ownership is known; otherwise generated-JS assignment semantics | Sequential writes to the fresh ordinary object | Literal/`Object.assign` forms may compete only after proof; unsafe policy is fixed by the contract | Compiler-owned hook-free representation, or explicit pristine-prototype assumption, proves inherited setters and `__proto__` cannot distinguish definition from assignment | Prototype getter/setter mutation, duplicate keys, computed effects, self-reference, escape, and final property-category validation |

These are applications of canonical phase 0.5, not a parallel "C0" migration.

## Language Feature Admission

Provisional spelling is chosen only after the semantic RFC is accepted. Every
language proposal must contain all of these sections and gates:

1. **Semantics:** complete optimizer-disabled behavior, evaluation order,
   effects, identity, mutation, presence, reflection, and error behavior.
2. **Static proof:** the exact type/ownership/escape/effect facts exposed to IR,
   including conservative invalidation.
3. **ABI:** closed-application, reusable-library, extern/host, and reflection
   behavior; the same contract must hold for every transfer objective.
4. **Lowering:** direct unoptimized JavaScript, target-IR obligations, and
   either equivalent native lowering or a specified compile-time rejection.
5. **Optimization envelope:** incumbent plus legal private alternatives; no
   optimization may define the construct's meaning.
6. **Tests:** parser and formatter, positive and negative semantic cases, typed
   lowering, optimizer-off JavaScript behavior, every supported backend,
   differential coverage where applicable, ABI fixtures, and raw/gzip/Brotli
   cross-objective API fixtures.
7. **Need:** two independent consumers or one unavoidable general language
   contract, with a measured boundary showing why existing forms are
   insufficient.

The current proposal backlog is semantic, not syntactic:

| Capability | Required semantics and proof | ABI and lowering constraints | Required adversarial tests |
|---|---|---|---|
| Typed ordinary-prototype homogeneous dictionary | Dynamic string keys, homogeneous values, `%Object.prototype%`, own versus inherited lookup, `__proto__`, insertion order, and getter/setter/proxy effects are explicit. No read is hook-free merely because the value is typed. | It remains distinct from null-prototype `Record<T>` and fixed-field aggregates. Direct JS lowering preserves ordinary object operations; native either implements the same contract or rejects it. Observable keys are never objective-renamed. | Prototype getters/setters, prototype mutation, missing keys, `__proto__`, enumeration order, spread/copy order, escape, and host calls. |
| Closed optional data aggregate | A finite field set with absence distinct from every field value, declared mutation, allocation identity, and no proxy/accessor behavior unless a separate contract says otherwise. | Private layout may vary; a public named layout, key order, or descriptor promise is ABI and remains stable across objectives. Baseline lowering materializes the direct named shape before scalar/positional alternatives are admitted. | Presence versus null/undefined boundaries, field order, partial construction, copies, mutation, escape, identity observation, and public round trips. |
| Structural and guarded match extension | The scrutinee is evaluated once; field reads, bindings, guard evaluation, arm order, scope, and exhaustiveness are specified. Existing array/record destructuring is reused rather than reintroduced. | Pattern syntax cannot narrow a host or public contract. Baseline lowering is ordinary typed control flow; compact target forms are later alternatives. | Effectful scrutinee/guards, failed arms, missing fields, binding scope, exhaustive and wildcard cases, and hook-bearing boundaries. |
| Host-callable typed method value | Receiver type, argument/rest evaluation, call kind, arity, name, constructibility, `arguments`, identity, and escape are language facts rather than a `JsValue` adapter convention. | Published receiver/callable behavior is ABI. Baseline JS is a direct ordinary function with the specified receiver behavior; fusion is private and requires identity/escape proof. Native behavior is explicit support or rejection. | Detached calls, `.call`/`.apply`, rest, omitted arguments, reflection, constructor attempts, escaping callbacks, and receiver-sensitive effects. |
| Typed accessor | Getter/setter invocation, receiver, descriptor flags, definition order, identity, effects, and exceptions are observable semantics. | Public descriptors and names are ABI. Direct JS lowering uses the newest legal accessor form or `defineProperty` without changing behavior; native support or rejection is explicit. | Read/write order, inherited accessors, descriptors, exceptions, extraction, reflection, and public consumers. |
| Owned reflected member identity | Reflection names a typed owner/member identity, not an arbitrary source spelling. If exposed as a string, that spelling is observable ABI and cannot vary by objective. An opaque member token may vary privately only when it cannot be converted, enumerated, or cross an uncoordinated boundary. | Do not add an ordinary `property_name(Type.field)` string whose result changes with mangling. Lowering either fixes an ABI string or carries an owner-qualified token through owned operations. | String observation attempts, dynamic access, enumeration, foreign consumers, owner conflicts, inheritance, serialization, and all three objectives. |

Accessor, Proxy, `Reflect`, dynamic descriptors, arbitrary host objects, and raw
property strings remain explicit boundaries. A feature is not accepted merely
because it makes one port shorter.

## Build-Time Domains

Build-time semantics belong to the immutable compilation contract or to a
separately reviewed language RFC; they are not codec-selected optimizer facts.

- A primitive define has one declared type, value source, default/error policy,
  and build-profile identity. Raw/gzip/Brotli compiles of that profile receive
  the same value before reachability.
- A runtime toggle remains the same logical Boolean. Packing is private only
  when all storage and reads are compiler-owned and no reference, enumeration,
  or host ABI exposes the representation.
- A generated ID or coordinated CSS/DOM/protocol name is an observable string
  when it crosses that boundary. Its mapping is then contract/ABI data and is
  identical across objectives. Objective-specific encoding is allowed only for
  an opaque internal identity with compiler-owned producers and consumers.
- Replacing diagnostic text with a code is a semantic change, not minification.
  It requires an explicit product profile in the contract, preserves placeholder
  evaluation order, and declares any decoder map as part of the artifact set.
  The selected codec cannot choose whether the text is lossy.

No arbitrary string scanning, name-suffix convention, or package policy may
create one of these domains.

## Phase 3: Minimal Target-JS Slice

Dependencies: every canonical phase-0 through phase-2 exit gate.

Work:

| Unit | Semantic layer | Retained incumbent | Objective decision | Safety proof | Verification boundary |
|---|---|---|---|---|---|
| TJS-01 minimal tree and printer | Hygienic target JS for constructs emitted by `src/codegen_ir_js.rs` only | One current string-emitter recipe reproduced byte-for-byte | Mandatory infrastructure; no byte-changing choice | Nodes carry binding/external/global identity, property category, function/call/receiver kind, allocation/capture identity, ordered effect barriers, module edges, syntax floor, source range, ABI elements, and live lowering obligations | Byte identity, witness completeness, source-map identity, independent parse/resolve, ABI, obligation, and resource checks |
| TJS-02 naming and declarations | Hygienic target bindings and scopes | Existing emitter naming plus each still-valid parsed-text naming/declaration candidate | Old and migrated forms compete under the exact selected codec until retirement | A rename changes spelling only; declaration kind/placement preserves scope, TDZ, hoisting, capture, function name, and ABI | Binding adversaries, source maps, final-byte resolution, behavior/API, selected objective, and compile-resource budget |
| TJS-03 contractions and placement | Target expressions, statements, classes, definitions, and assignments | Current emitted form plus current valid text challenger | Each family is independently scored against its incumbent | Typed/target effects, identity, receiver, allocation multiplicity, property-definition kind, precedence, and control completion authorize the rewrite | Family semantic negatives, final syntax/property/obligation witness, source maps, exact objective, startup/runtime guards, and resources |
| TJS-04 provenance report | Target identities through printed byte ranges and artifact/chunk ownership | Deterministic output with no persisted rename map | Reporting is mandatory and non-semantic; reuse of an old spelling is later a scored proposal, never an ABI command | Many-to-many symbol/property identity records owner and field slot, source spelling/ranges, emitted spelling, pinned/candidate reason, and artifact | Deterministic serialization, round-trip identity checks, stack/source-map fixtures, and no public/host name change across objectives |

Keep the independent standards-grade parser/resolver as a final-byte check, not
as production identity recovery. Migrate naming/remapping, declaration rewrites,
copies, class/prototype contraction, and statement/expression placement one
family at a time; do not build an arbitrary ECMAScript frontend.

Exit:

- every migrated transform consumes typed/target identity rather than spelling;
- the configured incumbent remains reachable and independently valid;
- target nodes cover only compiler-emitted JavaScript, not arbitrary ECMAScript;
- each retired text path has behavior, ABI, selected-objective, source-map, and
  resource evidence.

## Phase 4: Reusable Proofs

Port source work using shipped language features may proceed after canonical
phase 2. A feature whose implementation needs target identity also depends on
phase 3.

For one admitted feature at a time:

| Unit | Semantic layer | Retained incumbent | Objective decision | Safety proof | Verification boundary |
|---|---|---|---|---|---|
| P4-01 reusable language/proof feature | Source semantics, type system, typed IR, boundary ABI, and direct backend lowering | Existing shipped language forms and the classified port boundary; unsupported use remains rejected | Semantics and ABI are mandatory and objective-independent; only later private representations are exact-scored | The accepted RFC supplies the complete admission template above and a conservative proof with named invalidation | Parser/formatter/type tests, optimizer-off JS, native support or rejection, semantic negatives, differential cases, ABI and cross-objective fixtures, then one generic port slice |

1. Land the semantic and ABI RFC, including the full admission template above.
2. Implement parser/formatter/type behavior and optimizer-disabled lowering.
3. Establish direct JavaScript behavior and native support or rejection.
4. Add negative, differential, ABI, and cross-objective tests.
5. Only then add retained representation alternatives and exact scoring.
6. Demonstrate two generic consumers before using a fork as the deciding case.
7. Migrate one classified fork slice and retain source, behavior, ABI, artifact,
   recipe, and resource evidence.

Exit: the feature removes proven internal `JsValue` glue without narrowing the
public contract, works outside the motivating port, and has no
objective-dependent semantics or ABI.

## Phase 5: Measured Candidate Backlog

Each item below first needs a minimized legal challenger that current candidate
generation cannot express or a fingerprinted report that shows a reachable
family is starved. The board entry names the incumbent, proof, validator,
selected metric, work budget, affected boundary, and expected resource cost.

The source audit narrows the candidate backlog below. Existing path-sensitive
propagation, expression superoptimization, pipeline fusion, escape sinking,
region outlining, parameterized private-function merging, closed-record key
join/constant-JSON projection, and pre/post-compression inlining fixed points are
implementation inventory, not new transforms under another name.

| Unit | Semantic layer and dependency | Retained incumbent | Objective decision | Safety proof | Verification boundary |
|---|---|---|---|---|---|
| P5-01 condition and exit forms | Typed CFG facts plus hygienic target control/expression nodes; depends on TJS-03 | Current typed emitter and all still-valid targeted condition/exit families | Exact selected-codec score of complete bytes; local punctuation cost only schedules forms | Positive/negative forms preserve Boolean type, NaN and coercion behavior, effects, throws/finally, labels, TDZ, branch completion, and evaluation cardinality; BigInt is covered when supported | Closure-derived adversarial condition/switch/exit corpus, typed negatives, final target witness, behavior, startup guards, and exact objective |
| P5-02 known-method constants | Typed IR; no target-tree dependency | Existing intrinsic call; current implemented folds remain unchanged | Folded and unfolded artifacts compete whenever output can grow | Exact UTF-16 semantics and bounded compile/output growth for currently unfused `trim`/`trimStart`/`trimEnd`, `slice`, bounded `split`, literal `replace`, and complete/partial `Array.join` | Per-method positive/negative tests including surrogates, empty delimiters, limits, replacement tokens, holes/nullish values, then behavior and exact objective |
| P5-03 remaining construction/observers | Typed IR for arrays and null-prototype `Record<T>`; target JS only for ordinary-object spelling | Sequential writes/current literal plus existing record field, key-join, and constant-JSON projection | Every new literal or scalar observation is scored against the retained form | Ownership, dominance, escape, holes, insertion order, prototype setters, accessors, spread, self-reference, and evaluation order; only remaining record observers such as proven `RecordValues`/`RecordHasOwn` are eligible | Prototype-mutation adversaries, CFG/dominance cases, observer ordering, optimizer-off behavior, native parity where applicable, and exact objective |
| P5-04 measured call/schedule interaction | Typed CFG/SSA after phase-2 recipe consolidation | Current pre/post-compression schedule, all-compress-off contrast, existing inlining/specialization/helper variants, and existing outline/fusion/merging contrasts | Add a cycle/order only for a minimized miss and exact-score the complete artifact | Existing direct-call, effect, escape, identity, growth, and lowering proofs; no uncosted long-constant duplication | Recipe replay shows the new order reaches a legal artifact no current recipe reaches; semantic/API, objective, compile-time, memory, and starvation evidence. Do not add a generic outer fixed point: reachability/inlining already converges twice |
| P5-05 dirty target scheduling | Hygienic target regions after TJS-03 | Current fixed rounds and bounded terminal beam | Scheduling changes reachability only; selected candidates still use exact complete-artifact scoring | Explicit family dependency/invalidation edges, no-change termination, and hard per-region/global visit and probe caps | A minimized miss or rescan profile, deterministic visit/rewrite/probe counts, identical legal candidate results at equivalent work, and resource bounds |
| P5-06 receiver-conflict properties | Owner-qualified typed fields plus target property identities | Global property allocation and implemented inheritance-component owner-scoped allocation | Greedy, DSATUR, bounded recoloring, and no-change forms are exact-scored | Runtime receiver sets conflict conservatively; unknown/reflected use invalidates only proven-safe clusters; public, record, host, and ABI names remain stable | Inheritance/union/unknown/reflection conflicts, provenance for every renamed materialization, cross-objective ABI, behavior, objective, and resources |
| P5-07 namespaces and repeated prefixes | Typed ownership plus hygienic target definition/call nodes | Uncollapsed namespace and repeated full prefix | Collapse/no-collapse and alias/no-alias are complete-artifact candidates with setup charged to every artifact | Dominance, one definition, no incompatible escape/delete/spread/accessor/receiver observation, no prototype replacement, and dependency-safe placement | Receiver-order and prototype adversaries, source maps/provenance, module behavior, exact objective, startup, and resources |
| P5-08 modules and delivery | Linked module graph and declared `ArtifactSet`; waits for a maintained split workload | Current single path and separate greedy split/preserve-module planners | Declared deployment objective ranks whole sets; startup and aggregate transfer remain separately visible | Dynamic targets arise only from reachable importers; declaration SCC/method motion preserves dependencies, live bindings, ABI, enumeration, and initialization order | No-move/stub/proven-no-stub fixtures, dynamic-import pruning, API/behavior, chunk graph, deterministic manifests, unexplored work, and resource evidence |
| P5-09 build and replaceable domains | Immutable compilation contract, then typed IR before reachability; a new source surface depends on P4-01 | Runtime source values or declared profile constants and direct strings/booleans | Codec never selects logical values or lossy policy; only legal private encodings are exact-scored | Declared typed value source/error policy, explicit ID/message domain, placeholder evaluation order, boundary ownership, and deterministic map membership | Same profile values and ABI across objectives, debug/lossy mode tests, map round trips, behavior, artifact-set objective, and DCE/specialization evidence |
| P5-10 stable naming hints | TJS-04 identity/provenance records | Deterministic fresh allocation with no input map | Map reuse is a scored spelling proposal and never a public ABI requirement | Input entries bind semantic symbol/property identities, cannot rename pinned public/host keys, and fail closed on stale/ambiguous identities | Deterministic many-to-many serialization, stale-map rejection, small-edit/cache diagnostics, source maps, cross-objective ABI, and exact objective |

Phase-5 exit for any admitted candidate:

- the challenger is legal, independently validated, reachable, and replayable;
- the retained incumbent can still win;
- the selected complete-artifact metric improves or the attempt is recorded as
  negative evidence;
- compile-time and memory stay within the declared profile;
- no port name, source path, unsafe default, or post-minifier is involved.

## Acceptance And Objective Isolation

All IR, emission, target, terminal, and artifact-set challengers use the
canonical acceptance order:

1. Materialize a stable recipe.
2. Validate typed legality, effects, identity, ABI, and lowering obligations.
3. Lower to target JS and print complete bytes.
4. Independently parse and resolve final bytes; validate syntax floor,
   bindings/externals, property categories, module links, observed ABI, and
   obligation witnesses.
5. Score the complete artifact or declared artifact set with the pinned selected
   codec.
6. Apply the configured transfer/runtime priority and guards.
7. Compare with the independently validated incumbent.

No exact codec call occurs before validation. Final-byte checks do not prove
general behavioral equivalence; typed legality and behavior/API suites supply
that evidence.

For each feature or family that can affect a reusable boundary, compile the same
source and build profile once per raw/gzip/Brotli objective and require:

- identical semantic and expected-ABI fingerprints before optimization;
- observed ABI matching that expected ABI for every artifact/module set;
- the same behavior/API fixtures passing for every objective;
- any differing bytes to be explained only by private representation choices.

## Verification And Progressive Rollout

The canonical migration's per-change order remains authoritative. A green Rust
or micro gate is necessary but cannot graduate a change that affects emitted
JavaScript without the proportional fork gates below. A corpus cell is one
`(fork, supported boundary, artifact class, objective)` tuple; aggregate bytes,
another objective, or another fork cannot compensate for a red cell.

### Current harness reality

The first implementation checkpoint is corpus readiness, before V-01 or new
compression work. The tracked matrix now pins six boundaries across SolidLil,
MotionLil, MarkedLil, MobXLil, and jQueryLil. The first Motion direct-output lane
and a separate true MobXLil `config/production.min.toml` lane are present. The
remaining Motion direct entry boundaries and a newer pinned compiler pair remain
open; both historical checkpoint builds time out on the newly pinned large
sources. Until those gaps close, the matrix is a useful fail-closed preflight,
not the complete five-fork checkpoint.

The phase-0 harness must pin all five source trees and keep direct compiler output,
packaging/deployment output, and diagnostics as distinct artifact classes:

| Fork | Current build and semantic commands to preserve in the pinned archive | Gate interpretation and gap to close |
|---|---|---|
| MotionLil | `MOTIONLIL_LILSCRIPT_BIN=... MOTIONLIL_BUILD_MODE=production node scripts/build.mjs`; `npm test`; `npm run check:types`; `npm run check:pack` | The build replaces `dist/`, compiles entries concurrently, then bundles and Terser-minifies package files. Add direct-output lanes for every maintained `mini`, `animateMini`, `animate`, `animate+stagger`, lab, and export boundary; package output is a separate deployment lane. `npm run test:size` uses Node codecs and is diagnostic. |
| MarkedLil | `LILSCRIPT_COMPILER=... node scripts/build.mjs --compile`; `npm test`; `npm run check:pack` | Keep raw, gzip, and Brotli objective artifacts separate and keep the closed-key build diagnostic. Omitting `--compile` can reuse `dist/marked.raw.js`, so it is forbidden in a gate. Test the published ESM/CJS/UMD API and official parse corpus, not only the parse-only diagnostic. |
| MobXLil | `LILSCRIPT_COMPILER=... node scripts/build.mjs`; `npm test`; `node scripts/package-smoke.mjs`; `npm run test:types` | Track regular ESM and true `production-min` as separate rows. A clean `--prod` build may synthesize the min path from regular ESM and is not proof of `config/production.min.toml`. The build also writes `src/dev-flag.lil`; each variant therefore needs its own archive. Run the upstream/differential semantics and package smoke for the shipping variants. |
| jQueryLil | `LILSCRIPT_COMPILER=... node scripts/build.mjs --compile`; `npm test`; `npm run check:pack` | Preserve the reusable ESM/CJS/UMD facade and DOM/Deferred compatibility boundary. A no-`--compile` build can consume stale `dist/jquery.raw.js` and is forbidden. Vite app rows are deployment evidence, not substitutes for the public-library row. |
| SolidLil | `SOLIDLIL_LILSCRIPT_BIN=... SOLIDLIL_BUILD_MODE=production node scripts/build.mjs`; `npm test`; `npm run check:types`; `npm run check:pack` | Track core, web, full, and declared import surfaces separately. The package build bundles and Terser-minifies compiler output, and `npm run test:size` uses Node codecs; both are deployment/diagnostic lanes unless a direct-output canonical row is recorded. |

These commands are an inventory, not permission to build in a sibling working
tree. The canonical runner exports the pinned Git tree, verifies the lockfile,
entry, config, compiler, codec, and harness hashes, deletes every declared output,
installs with the pinned lockfile, and writes only inside that archive. A missing
artifact, timeout, compile failure, skipped required suite, or stale `dist/` makes
the cell ineligible.

### Gate levels

Apply these levels progressively; do not defer all real-library feedback to phase
6:

| Level | When it runs | Required evidence |
|---|---|---|
| G0 targeted | Every change | The narrow semantic/adversarial test, validator rejection case, incumbent replay, and family on/off ablation. Language changes also run parser/formatter/type/lowering, optimizer-off JavaScript, native support or specified rejection, differential, ABI, and cross-objective fixtures. |
| G1 affected boundary | Before a byte-changing change leaves its branch | Every affected fork boundary is rebuilt from a pinned clean archive for every declared objective; its semantic/API command passes on the exact artifact and the selected metric is no worse than the frozen legal incumbent. |
| G2 five-fork checkpoint | At phase-0 corpus readiness; after each V-01/V-02/V-03 unit, phase-1 recovery, and phase-2 registry batch; after TJS-01 and each independently migrated TJS-02/TJS-03 family batch; after TJS-04, each P4-01 feature, each admitted P5 unit, and every phase-6 deletion | MotionLil, MarkedLil, MobXLil, jQueryLil, and SolidLil all run their maintained boundaries. Existing red comparisons to an independent JS baseline may remain visible during recovery, but no cell may worsen against its frozen legal LilScript incumbent. |
| G3 phase/release | At every canonical phase exit and before release | Full Rust/canonical/codec/release gates, the complete five-fork matrix against both pinned compiler revisions, eligible independent JavaScript frontiers, deterministic replay, ABI equality across objectives, and sequential resource measurements. Release additionally requires every declared supported boundary to meet the canonical phase-6 JavaScript-baseline criterion. |

A small commit inside a major unit may stop at G0/G1, but the unit remains open
until G2 passes. If a fork is unaffected, byte-identical replay may satisfy its G1
cell; G2 still rebuilds and tests it. If a required external suite is unavailable,
the unit remains open or behind an off-by-default experiment.

The common in-repository gates are:

```sh
cargo test --release --all-targets
node comparison/cases/run.mjs --canonical-only
node --test benchmarks/codec-contract.test.mjs
node scripts/board.mjs check
node scripts/check-doc-links.mjs
node --test comparison/large-libraries/contract.test.mjs
node comparison/large-libraries/run.mjs --check
```

After the phase-0 matrix contains all five forks and all required rows, G2/G3 also
run:

```sh
node comparison/large-libraries/run.mjs --check-inputs
node comparison/large-libraries/run.mjs --run --compiler both --output "$RESULT"
```

Gate invocations use the checked-in zero-byte regression policy and never pass
`--max-regression`. A nonzero override is an explicitly labelled diagnostic; it
cannot make a regression pass or graduate work.

### Isolation and concurrency

Read-only hash/schema checks and independent semantic preflights may run in
parallel only when each job has its own exported source tree, `node_modules`, npm
cache, Cargo target directory, `dist`, `.tmp`, `reports`, retained artifacts, and
result path. No two invocations of a sibling build script share a checkout:
MotionLil and SolidLil replace `dist`, MarkedLil and jQueryLil can reuse raw output,
and MobXLil mutates both `dist` and `src/dev-flag.lil`. Internal parallelism owned
by one build script is acceptable because that invocation owns its archive.

Parallel preflight output is disposable and is never promoted into authoritative
evidence. The authoritative coordinator rebuilds from immutable inputs and runs
one compiler/fork/boundary/objective cell at a time on a quiescent host, in a
fixed recorded order. Exact selected-codec measurement and wall/user/system time
and peak RSS are sequential; startup/runtime measurements use their separately
declared protocol. The report records contention, unavailable counters, stop
reason, evaluated/rejected/unvisited/starved work, and every artifact hash.

### Promotion and rollback

For each major unit, use this rollout:

1. Land the G0 rejection tests and freeze the old selected recipe and artifact.
2. Run one smallest affected fork boundary as a canary, then every affected
   boundary at G1. Do not choose only the fork where the change wins.
3. Run G2 for all five forks and review every cell, skip count, ABI fingerprint,
   selected recipe, stop reason, and resource delta.
4. Keep the change only if the retained incumbent remains reachable, all semantic
   cells pass, and no selected-metric cell regresses. Then run G3 at phase exit.

Immediately roll back or leave the family off by default when a formerly green
semantic/API/ABI/identity/obligation cell fails, a required test becomes skipped,
an output is stale or unpinned, the incumbent disappears, one selected-metric
cell grows by even one byte, determinism fails, or the declared compile-time/peak-
memory/runtime guard is exceeded. A cross-metric, aggregate, or other-fork win is
not a waiver.

## Regression Protocol

Preserve the exact source archive, compiler and codec binaries, normalized
contract, selected recipe, explain report, output bytes, semantic output, and
resource record before investigating. Re-run the failing cell twice in isolation,
then identify the first divergent decision with existing hard-offs and family
ablations. Minimize semantic and codec contexts separately so the semantic case
keeps the behavior fault and the size case keeps the selected-codec context.

Classify before changing compiler or port source:

| Classification | Required action |
|---|---|
| Invalid/stale comparison or nondeterminism | Fix the generic harness/provenance fault and invalidate affected measurements; never inherit an artifact or bless one run. |
| Semantic, ABI, identity, binding, effect-order, or obligation defect | Stop size ranking, fix the responsible generic semantic/lowering/validation layer, and retain an adversarial non-package-named regression. |
| Lost legal incumbent | Replay it through current validation and restore it as the family incumbent or reachable registry alternative. If it is now illegal, retain the precise reusable ineligibility proof instead of restoring bytes. |
| Admission/validation defect | Repair the shared proof or validator before widening search. Every incumbent and challenger uses the same path. |
| Starvation/scheduling miss | Retain a small exhaustive oracle and fix generic scheduling within a measured budget; a larger beam is not a substitute for attribution. |
| Missing generic transform | Add only a reusable proof-backed candidate with a minimized package-neutral case, retained incumbent, exact complete-artifact score, and resource guard. |
| Missing reusable language proof | Follow P4-01 and the language-admission template; do not encode the motivating package, path, or JavaScript spelling. |
| JS-shaped or mechanical port | Rewrite the fork idiomatically with existing language forms, preserving its public contract and before/after attribution. Do not add compiler glue to reward the translation. |
| Legitimate dynamic/public boundary or honest objective trade | Keep the boundary stable and record the candidate as ineligible or losing. Size-first does not accept the trade; another priority may report it under an explicit policy. |
| Resource regression | Remove, bound, or reschedule the generic work; do not hide it by excluding the expensive fork or by measuring concurrently. |

Every fix ends with the minimized regression, G1 for every affected boundary, and
G2 before the work unit closes. Never introduce a package/path matcher, hidden
threshold, post-minifier, unsafe host assumption, reduced suite, or aggregate
allowance. Existing language forms and idiomatic source work are preferred; new
language semantics require independent consumers and objective-independent ABI.

## Release Exit

Phase 6 deletes a text rewrite, duplicate policy path, or legacy entry point only
after no supported caller remains and its replacement passes behavior, ABI,
selected-objective, source-map, and resource gates. Retain the independent
final-byte validator, pinned codecs, configured incumbent, typed CFG/SSA and
native paths, and historical negative evidence.

Release certification is exactly the canonical phase-6 exit: all maintained
semantic/API gates pass; raw/gzip/Brotli preserve one contract; fingerprinted
selected-metric evidence has no legal-incumbent regression; corpus-scoped claims
compare with eligible pinned JavaScript baselines; unexplored work is reported;
and compile-time, memory, startup, and runtime tradeoffs are published.
