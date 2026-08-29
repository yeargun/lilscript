# Planned compiler migration

Status: canonical execution plan from the 2026-08-29 checkout. Architecture:
[planned architecture](../compilation/planned-architecture.md). Implemented
behavior: [current architecture](../compilation/current-architecture.md). Live
task status: [ledger](board/LEDGER.md).

This plan starts after `arch-02`, `arch-03`, `arch-04`, `arch-06`, and `arch-07`
landed. `arch-05` remains active. Older 07 prose is rationale and sequencing
history where it conflicts with the ledger; it is not permission to redo landed
work.

The Closure-informed, language-and-compiler compression work is decomposed in
the [compression migration](compression-migration.md). That document is a
subplan for phases 3-6 here; it may refine work units and test cadence but may
not bypass this plan's evidence, legality, incumbent, or release gates.

## Rules of execution

1. Keep every green behavior gate green. Correctness, source intent, host
   assumptions, and ABI block size ranking.
2. Freeze and retain the current legal incumbent before adding an alternative.
   If a supposedly broader search cannot reproduce it, stop and fix that first.
3. Measure complete emitted artifacts with `lilscript-codec`. Raw heuristics may
   order work but cannot declare gzip/Brotli wins.
4. Use one objective artifact per selected metric. A gzip improvement cannot
   offset a Brotli regression in a Brotli build.
5. Report bounded work and omissions. Never call production beam search globally
   optimal.
6. Add language or analysis facts that are reusable across ports. Never add a
   package-name matcher, a port-shaped target fold, post-minification, or a
   default-on `assume_pure_property_reads` equivalent.
7. Add infrastructure only when a minimized case or a fingerprinted corpus run
   shows the existing mechanism cannot express, validate, or reach a legal win.
8. Record compile time and peak memory with size. More search or more knobs is
   not an automatic improvement.

## Starting checkpoint

Live implementation, gate counts, and provisional size deltas live only in
[`docs/current-status.md`](../../current-status.md). Source/tests remain behavior
authority; tracked fingerprinted reports remain numerical authority; the
[ledger](board/LEDGER.md) remains task authority.

Do not reschedule already implemented contract/provenance, registry, reversible
prior, class/closure/property, expression-language, reserve, or scored-peephole
work. The current architecture inventory is
[current architecture](../compilation/current-architecture.md).

## Phase 0: Make evidence and recipes replayable

Dependencies: none. Correctness work may continue independently, but no size
regression recovery or product claim advances without the relevant evidence row.

Work:

1. Extend the tracked large-library schema before collecting new numbers. Add
   source/finalizer/artifact hashes, complete normalized contract/objective,
   selected recipe, explain output, stop reason, family counts, wall/user/system
   time, and peak RSS. Exclude timestamps and timings from the deterministic
   evidence projection.
2. Serialize and replay a complete selected recipe: IR options, emission options,
   target/printer choices, context identity, parent lineage, compiler/config/
   source/codec fingerprints. A historical byte artifact is not a recipe.
3. Separate direct compiler-output lanes from downstream deployment lanes such
   as Vite/Oxc, banners, facades, or bundlers. Only direct output versus an
   independently authored eligible JS baseline supports a compiler claim.
4. Add an independently selected JS baseline frontier. A comparison between two
   LilScript revisions proves regression/recovery, not competitive superiority.
5. Pin the frozen pre-change and current compiler/source/config/harness for every
   maintained Motion, Marked, MobX, and jQuery boundary. Never inherit old
   `dist/` output after a build failure.
6. Run behavior/API gates, then record exact selected-metric bytes and optional
   cross-metric diagnostics plus resource measurements.

Exit criteria:

- `node --test comparison/large-libraries/contract.test.mjs` and
  `node comparison/large-libraries/run.mjs --check` pass;
- the generated report reproduces the provisional deltas in current status or
  records and explains corrected values;
- every measured row names its semantic boundary and selected objective;
- selected recipes replay byte-identically;
- deterministic projections and promised artifacts match across two runs;
- no current green behavior count decreases.

Refuse or roll back when:

- an artifact, config, source tree, compiler, codec, or harness is unhashed;
- a compile failure inherits an old `dist/` artifact;
- a cross-metric diagnostic is used to pass a selected-metric regression;
- the before and after runs do not use the same pinned source and boundary.

## Phase 0.5: Strengthen legality and ABI gates

Dependencies: phase 0 recipe/evidence shape. Complete before restoring an old
smaller artifact whose current legality is not already proven.

Work:

1. Separate compilation world, artifact format, and public boundary roots.
2. Freeze an expected ABI from the linked typed program before optimization and
   derive an observed ABI witness from each final artifact/module set.
3. Cover emitted export names, live bindings/module identity, callable kind and
   default-sensitive arity, constructibility, constructor/prototype topology,
   owner-qualified public fields, descriptors/order where promised, foreign
   imports, and host names only as maintained cases require.
4. Validate typed legality before printing. Before any exact codec call,
   independently parse final bytes and validate syntax floor, every binding or
   declared external, property category, module links, observed ABI, and live
   obligation witnesses. Behavioral equivalence remains a test-suite claim.
5. Add cross-objective ESM consumer fixtures for namespace keys, live bindings,
   callable identity, constructors, descriptors, inheritance, and field order.

Exit criteria:

- the configured incumbent passes the same gates as challengers;
- expected and observed ABI fingerprints match for raw/gzip/Brotli builds;
- no scored artifact relies on an unresolved identifier or unclassified
  property access;
- ordinary host `extern` names remain exact unless a separately declared
  coordinated foreign ABI owns both sides.

## Phase 1: Recover large-library incumbents

Dependencies: phase 0 and the applicable phase 0.5 gates. This phase precedes
new optimizer infrastructure.

Priority order:

1. MobX `production-min`, while retaining the regular MobX improvement.
2. Regressing Motion lab/export/animate/mini boundaries, while retaining the
   `animateMini` result.
3. Marked's selected Brotli package artifact. Its gzip diagnostic cannot offset
   a Brotli loss.
4. jQuery, which is unchanged against the frozen compiler but still loses its
   maintained public-library comparison. Do not delay regressions 1-3 to chase
   that older gap.

For each loss:

1. Diff normalized contract, objective, `--explain json`, selected
   `OptimizationOptions`, `IrJsOptions`, terminal decisions, stop reason, and
   final bytes between the two pinned compilers.
2. Identify the first decision that made the old legal artifact unreachable or
   outranked. Use existing hard-offs and isolated family tests before adding an
   instrument or knob.
3. Replay the old representation through current legality and ABI validation.
   If it remains legal, make it a retained incumbent or
   explicit alternative in the existing registry. If it is no longer legal,
   prove a correctness/ABI reason; size does not overrule that reason.
4. Fix one generic family at a time and rerun the affected library plus Rust,
   canonical, and codec gates. Preserve losing alternatives as regression
   fixtures where they explain codec interaction.

Exit criteria:

- no listed `size-first` shipping artifact regresses its selected metric against
  the frozen pre-change compiler, unless that older artifact fails the current
  semantic/ABI contract and is therefore recorded as ineligible rather than
  restored;
- all improvements are produced by the LilScript compiler itself;
- each fix has a minimized non-port-named test and a complete-artifact ablation;
- compile time and peak memory are no worse without an explicitly approved,
  measured size trade.

Rollback conditions:

- any semantic/API/identity gate fails;
- an incumbent disappears, a hard correctness pass becomes optional, or an
  unsafe assumption becomes implicit;
- a fix helps only a package-name/source-shape matcher;
- a broader budget merely hides a starvation defect or materially raises cost
  without recovering selected bytes.

## Phase 2: Consolidate contract, decisions, and evaluation

Dependencies: phase 1. Goal: remove duplicate policy without changing the
reachable legal artifact set.

Work:

1. Normalize config once into the existing compilation contract and objective.
   Pass those values through IR preparation, optimization, emission, terminal
   contraction, and bundle planning. Lower layers stop reading raw config to
   reinterpret ABI or unsafe policy.
2. Move existing phase-order and compress-pass probes out of imperative setup in
   [`optimize_and_select_javascript_inner`](../../../src/compiler.rs#L1488-L1651)
   into registry-owned recipes. Do not add new alternatives in the move.
3. Give every profitability recipe an incumbent, legal alternatives, proof
   predicate, validator, work class, and explain name. Keep mandatory passes and
   ABI/unsafe/explicit-lowering decisions outside profitability search.
4. Route ordinary leaves, terminal challengers, and canonical/search-off
   peephole through one acceptance primitive: materialize,
   validate, exact-score, apply objective guards, compare with incumbent.
5. Extend the existing selection report only with missing facts: stable recipe
   identity/parent, evaluated/rejected counts by family, validation reason, and
   compiler/source/config/codec fingerprints.

Exit criteria:

- every current candidate is reachable from one registry entry or explicitly
  classified mandatory/ABI/unsafe/intent;
- `IrJsOptions` and `OptimizationOptions` are execution plans, not competing
  policy sources;
- search-on, search-off, and terminal acceptance use the same rank and
  validation semantics;
- output is byte-identical where the phase claims only consolidation, otherwise
  every changed selected artifact passes phase 0 and phase 1 gates.

Refusal condition: do not introduce a general `ChoiceGraph`, solver, persistent
IR, Pareto archive, or new config DSL to perform this consolidation. Add one only
after a measured case demonstrates that registry recipes and the bounded
coordinator cannot express the required interaction.

## Phase 3: Introduce the minimal hygienic target JS tree

Dependencies: phase 2 and green identity gates. This is the remaining `arch-05`
work.

Work:

1. Represent only constructs emitted by `src/codegen_ir_js.rs`, with resolved
   binding/external/global references, property categories, function/call kind,
   allocation/capture identity, scopes, expression/statement kind, precedence,
   ordered effect barriers, module edges, exports, syntax-floor requirements,
   and lowering obligations.
2. First lower one retained incumbent to the tree and print byte-identically,
   emitting a binding/property/ABI/obligation witness. Keep an independent
   standards-grade parser as the final-byte validator.
3. Move identity-sensitive families first: naming/remapping, declaration
   rewrites, copies, class/prototype contraction, and statement/expression
   placement. Each move gets binding-aware tests and a complete-artifact
   on/off comparison.
4. Attach `PreserveJavaScriptBitOrZero` to the target operation so unrelated
   contractions no longer need the current whole-candidate peephole skip.
5. Delete each text implementation only after every gate passes. Do not migrate
   a fold merely because it exists.

Exit criteria:

- production never reparses generated text to discover binding or owned-property
  identity;
- final bytes are independently parsed/resolved and checked against intended
  identities, ABI, syntax floor, and live obligations before scoring;
- named classes originate in typed IR/target lowering, not class-table recovery;
- no production path calls `repair_late_javascript_candidate` or a text-mutating
  peephole family whose target equivalent has landed;
- the full phase 0/1 gate set remains green and resource budgets remain accepted.

Rollback conditions:

- the new printer cannot reproduce an incumbent;
- a transform needs textual spelling to infer semantic identity;
- a valid-but-wrong-nearer binding can reach codec scoring;
- target-tree complexity grows to arbitrary JavaScript parsing without a
  maintained emitter case requiring it.

## Phase 4: Close library losses with reusable proofs

Dependencies: phases 0.5-2. Target-identity compiler changes depend on phase 3.
Port source work using shipped language features may proceed in parallel.

Classify every loss using
[failure triage](../verification/failure-triage.md). Use existing language
features before proposing new syntax:

- Motion: attribute each boundary independently. Replace internal `JsValue`
  option/keyframe bags only where an owned optional/structural proof preserves
  Motion semantics. Do not infer hook-free reads or narrow the public DOM API.
- MobX: explain why regular output improves while `production-min` regresses.
  Preserve Proxy, Reflect, accessor, descriptor, and constructor behavior.
  A production-only spelling difference is not permission for pure-getter
  assumptions.
- Marked: treat a selected-metric regression as red despite a cross-metric gain.
  Broaden ownership/no-hook proof only from compiler-owned, non-escaped
  allocations; otherwise retain the incumbent.
- jQuery: use shipped `object{...}`, expression-if/scalar match, constructor
  export, and wider sound array-ness facts before proposing syntax. Its unchanged
  migration delta is not a competitive win. Do not revive post-hoc `if` to
  ternary contraction; the negative result is recorded in
  [jquery-01](board/notes/jquery-01.md).

A new language feature requires semantics, optimizer envelope, unsupported
target behavior, ABI effect, reusable examples from more than one port or a
clear general language contract, and paired cases before compiler optimization.
Host-callable typed methods, accessors, structural optional bags, richer match,
and object spread remain proposals until such evidence exists.

Exit criteria:

- the targeted port no longer needs a library-specific wrapper or unsafe flag
  for the proven internal case;
- dissolve/keep and relevant target spellings remain scored alternatives;
- every supported boundary passes semantics/API and is no larger than the best
  eligible pinned JS toolchain for each selected metric;
- no claim depends on post-minified LilScript output.

## Phase 5: Improve bounded search only where evidence shows a miss

Dependencies: phase 2; target-aware families also depend on phase 3.

Work:

1. Use the report to distinguish an absent legal alternative from a starved or
   poorly ordered existing family.
2. Add a joint family only for a measured non-monotone interaction, following
   `function-spelling-stable-local-names` and `pure-helper-dense-tables`. Keep
   the incumbent in the joint set.
3. Add exact enumeration only for a small declared finite domain whose complete
   artifacts can all be validated and codec-scored within budget.
4. Retain a best-so-far archive and make selected recipes replayable. For paths
   moved to an append-only logical schedule, require larger budgets to consume a
   superset of smaller work. Report other paths as non-monotone rather than
   claiming the guarantee.
5. Publish active, attempted, validated, scored, retained, unvisited, and starved
   counts by family plus the stop reason. `portfolio-exhausted` remains
   `best-observed`, not optimal.
6. Use exact selected-codec scoring for acceptance. Use structural strata,
   cached hashes, and measured cheap predictors for scheduling; exact alternate
   codecs are final diagnostics unless a diversity experiment proves their cost
   worthwhile.

Exit criteria:

- no maintained large artifact silently misses representation, call-graph,
  control-flow, naming, pooling/layout, or terminal families;
- every new search unit buys a measured selected-objective opportunity or is
  removed;
- production compile-time and memory limits are enforced, and release/deep
  search are explicitly different profiles.

Refusal condition: no full Cartesian product, universal exact solver, learned
score as final authority, or unbounded Brotli probing.

Chunk planning remains separate until a maintained split-delivery corpus defines
initial-load, lazy-route, repeat-visit, total-transfer, request, and cache costs.

## Phase 6: Delete superseded paths and certify release

Dependencies: phases 0-5.

Delete only after no production caller remains:

- migrated text-mutating peephole and repair paths;
- imperative decision sites duplicated by the registry;
- raw-config policy reads below normalization;
- legacy `src/codegen_js.rs` entry points after supported callers/tests use the
  typed IR path.

Retain the independent final-byte parser/resolver, exact codec implementation,
typed CFG/SSA optimizer, native backend, configured incumbent, and historical
negative evidence.

Final exit criteria:

1. Every gate listed in [`docs/current-status.md`](../../current-status.md)
   remains green or advances through an explicitly reviewed contract update.
2. Raw/gzip/Brotli builds pass the same ABI and semantic fixtures.
3. Fingerprinted large-library evidence has no selected-metric regression from
   the frozen pre-change compiler.
4. Every declared supported and semantically equivalent maintained boundary is
   no larger than its best eligible pinned JS baseline in each selected metric.
5. The report says `best-observed` and names unexplored work unless a finite
   subdomain was actually exhausted.
6. Compile-time, peak-memory, startup, and runtime tradeoffs are published with
   the size result.

## Per-change gate order

Run the narrowest semantic test first, then broaden without skipping layers:

1. Targeted Rust test, including the named registry/contract/identity tests
   referenced above.
2. `cargo test --release --all-targets`.
3. `node comparison/cases/run.mjs --canonical-only`.
4. `node --test benchmarks/codec-contract.test.mjs`.
5. The affected library gate and fingerprinted before/after row.
6. `scripts/release-check.sh` before release.

If a long external corpus is unavailable, the change does not graduate past the
last completed phase. It may remain behind an off-by-default experimental family;
absence of evidence is not a pass.
