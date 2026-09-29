# Migration progress

The one-compiler migration as a checklist. [plan.md](plan.md) says what each task is and the order of work. [history.md](history.md) holds the batch records: evidence, numbers, and what each batch left open.

`[x]` done · `[~]` partly done, and the note says what is open · `[ ]` open, with the step it is due in.

## Now

- **Branch:** `finer/059-idiom-directed-naming`. The last landed batch is Y5a/b (private application roots and frames), on 2026-09-30.
- **Steps:**
  - done: 1, 4, 5, 7, 8 and 9, and step 6's critical path;
  - partly done: 2 and 3;
  - under way: 10.
- **Ratchet baseline:** Y5, accepted under the owner's totals rule.

## Next steps

1. **Step 10, structural rules first.** Shrink operations, bindings and functions; do not chase `|0` under codecs.
   - M7.8's rest: known-method folds from the catalog, evaluation of small pure functions called with constant arguments, and string-literal sums. This is where Closure ADVANCED leads on apps and algorithms.
   - M6.6 (escape), then M7.9 (scalar replacement, and store copies that move rather than copy). This is flattening, and it deletes the tuple rebuild per field write.
   - M7.5's rest: tail returns, closures created inside bodies, and bodies across modules.
   - Then M7.1, M7.7, M7.6, M6.7, M6.8, M8.5 and M7.10, with M5.3b deleting each transitional rule as its replacement lands.
2. **Compile time.** V2's markedlil trace adds a walk from the structural-search winner; both it and the level-0 walk reach the same score. The extra `int32_hints` trials explain only part of the work. Reduce repeated formation and judging through M5.7 (the incremental tail), preserving the general search contract; the trace and costs are in Y5's history record.
3. **Off the critical path:**
   - step 2: the layout commit, M3.3b with M3.8a, then motionlil compiler-written;
   - step 3's tools: M2.10, M2.11, M2.12 and M2.15;
   - M3.4.

Step 10 is done when `comparison/apps` and `comparison/algorithms` no longer lose to Closure ADVANCED.

## Owner rulings in force

- **One heavy job at a time.** Only one building, testing or measuring job runs on this host at once. Test about every 20 minutes of work, or after a critical part.
- **Batch discipline.** Each batch opens with a pre-registration in history.md. Its changes (C1, C2, …) land one commit each, and one verification chain runs per batch.
- **Totals decide (2026-09-29).** A batch is judged by its totals over the ratchet, the cases and the fleet. Growths of a few bytes here and there do not block a batch whose totals shrink, and extra search stays only if it pays for its CPU.
- **`|0` (2026-09-29).** Under gzip and Brotli, `|0` is not worth effort. Range-proven elision matters only for the raw objective.
- **Amendments and laws:**
  - AM1: budgets are counts.
  - AM2: size comes first from the default level up.
  - AM3: compile time has no target in seconds; it is judged by how it scales and by what it buys.
  - L22: decide statically first.
- **Y rulings** (architecture §21.1): Y1 answered yes (2026-09-28), Y5 answered yes (2026-09-29). Y2–Y4, Y6 and Y7 are pending, and none of them blocks the critical path.

## Steps

The plan's [order of work](plan.md#order-of-work). The critical path is 4 → 5 → 6 → 7 → 8 → 9 → 10 → 11 → 13.

- [x] **1** katexlil released with its font metrics in LilScript
- [~] **2** Delivery contract and motionlil. Done: M3.3a. Open: the layout commit, M3.3b with M3.8a, motionlil (M3.3e, M12.2)
- [~] **3** Tools. Done: M2.5, M2.13, M2.14. Open: M2.7, M2.10, M2.11, M2.12, M2.15; M2.4 runs ahead of each fact
- [x] **4** Canonical formation without tree analysis (A1)
- [x] **5** Floor slice: M5.1's core, M6.4a, M7.8a, M7.3, M7.5a
- [~] **6** Budget model, counted. Critical path done (M3.5's walk, M3.1's first slice). Open: M3.4, `-j` scoring after M5.6, M3.2, M3.9, M3.7
- [x] **7** Language slice 1: M4.2 with M10.2, M10.4, M10.7, M10.16
- [x] **8** The core: M4.6, M10.13, M10.9, M10.15, M10.11's `debug` class, M10.14's checker half, M10.12
- [x] **9** Machinery: M4.3, M4.4, M5.2, M5.5, M5.3a, M8.2's A2, M8.3's first half (their rests are carried below)
- [ ] **10** Facts and the rest of the floor: M6.4b (partly), M6.6, M6.7, M6.8, M7.1, M7.4, M7.5, M7.6, M7.7, M7.8, M7.9, M8.5, M7.10, with M5.3b; M5.7, M8.3's second half; M4.5, M11.1, M11.11, and M11.5 right after M7
- [ ] **11** One choice system: M10.3, M9.1's rest, M9.2's gzip row, M9.3 per site, M9.5–M9.8's rests, M9.9–M9.12, M10.19, M3.3c with M3.8b
- [ ] **12** The rest of M10: M10.1, M10.8, M10.5, M10.6, M10.18, M10.10, M10.17, M10.14's constructibility (Y3), M10.11's termination (Y4) and pins
- [ ] **13** Qualification and native: M12.2 for the other ports, M3.3d, M11.3, M11.4, M11.6–M11.10, M11.12, M12.3–M12.5; M12.6 throughout

## Tasks

### M0–M1 Record and freeze; one compiler: done

- [x] M0 Record and freeze
- [x] M1.1–M1.9 One compiler: the old route, its flag and its tests are gone. M1.9's two open correctness debts are carried below

### M2 Verification ladder, baseline, benchmark contract

- [x] M2.1 Green CI
- [x] M2.2 Case runner
- [~] M2.3 Oracles. Open: interpreter-generated oracles
- [ ] M2.4 Interpreter extension: step 3, then ahead of each fact
- [x] M2.5 Admission parse
- [x] M2.6 Port runner and ledger
- [ ] M2.7 Differential: step 3
- [x] M2.8 Baseline on one binary
- [x] M2.9 Interim release
- [ ] M2.10 Benchmark contract: step 3
- [ ] M2.11 Held-out libraries: step 3
- [ ] M2.12 Perf runner, runtime ledger, counters, micro perf gates: step 3
- [x] M2.13 Generic corpus ratchet
- [x] M2.14 Frozen compile-time baselines
- [ ] M2.15 Calibration corpus: step 3

### M3 Configuration, public API, delivery contract, budget model

- [~] M3.1 Schema v3. Open: `format` (M3.3b), `[resources]` (M5.6), `[performance]` (M2.12)
- [ ] M3.2 Family registry: step 6
- [x] M3.3a Delivery, first slice
- [ ] M3.3b Formats: step 2 (work in progress: `~/lilscript-work/portwork/m3.3b-wip.patch`)
- [ ] M3.3c Facts and choices: step 11
- [ ] M3.3d Lazy effects and cycles: step 13
- [ ] M3.3e Ports, with M12.2: step 2 for motionlil
- [ ] M3.4 Public API and shared formation: step 6
- [~] M3.5 The budget model, counted. Open: `-j` scoring (after M5.6)
- [ ] M3.7 Environment variables: step 6
- [ ] M3.8a Consumer-shakeable delivery, first half: step 2
- [ ] M3.8b Consumer-shakeable delivery, second half: step 11
- [ ] M3.9 Caches and the decision lock: step 6
- M3.6 merged into M3.5

### M4 Checker identities and checker-owned facts

- [x] M4.1 Nominal identity
- [~] M4.2 The dynamic type. Open: type parameters by id, interned types (with M4.4)
- [~] M4.3 Checker facts transported. Open: parameter defaults on declarations
- [~] M4.4 Node ids. Open: statement ids, type parameters by id, interned types
- [ ] M4.5 Contracts and capabilities at check time: step 10
- [~] M4.6 Operation catalog. Open: one operation identity in the IR (M5.2), `hasOwnProperty.call` and effect-free constructions (M10.17)

### M5 The machinery

- [~] M5.1 Program edit kernel, DCE on both targets. Open: `UseIndex`, deleting `drop_unreferenced_functions`
- [~] M5.2 Tree annotations and journal. Open: transfers for rule-created bindings and functions, and the deletions they allow (`quiet.rs` first)
- [~] M5.3a Scheduler. Open: the program rules' SCC order with a dirty worklist; classifying the remaining rules
- [ ] M5.3b Transitional rules deleted: steps 10–12, each with its replacement
- [x] M5.4 Monotone selection and the terminal slot
- [x] M5.5 Dataflow and views
- [ ] M5.6 Resource accounting (counts)
- [ ] M5.7 Incremental tail: step 10

### M6 The fact spine

A fact is landed only when its re-derivations are deleted (plan rule 2).

- [~] M6.1 Call graph and function facts. Consumed by the effects, the ranges and the program rules. Open: deleting the tree's re-derivations
- [~] M6.2 Effects. Consumed on the tree's evaluation-behaviour column. Open: the same deletions
- [ ] M6.3 The `pure` contract. Computed, not consumed
- [~] M6.4a Values, exact tier. Open: finite sets, `simplify::known`'s constant cases
- [~] M6.4b Values, range tier. Landed: ranges with joined parameters; `javascript_int32.rs` deleted. Open: primitive classes, deleting formation's own number sources, `raw_domains.rs` and the binding-class derivation
- [~] M6.5 Initialization order. Consumed on the tree. Open: deleting `quiet.rs`'s order and `root_constants.rs`'s own proof
- [ ] M6.6 Escape and uniqueness: step 10
- [ ] M6.7 Field facts: step 10
- [ ] M6.8 Frequency: step 10

### M7 Program rules: the floor

- [ ] M7.1 Removal: step 10
- [x] M7.2 Discarded effect-free calls
- [~] M7.3 Parameters and returns. Open: callee-built defaults and deletion of the three tree passes (M5.3b)
- [~] M7.4 Root constants, defines, forwarding. Landed: scalar constants by rule, string constants as a codec-judged family. Open: defines, deleting `eliminate_aliases` and `forward_root_constants`
- [x] M7.5a Removal-only inlining
- [ ] M7.5 Inlining, the rest: step 10
- [ ] M7.6 Namespaces and emulated methods: step 10
- [ ] M7.7 Fields: step 10
- [x] M7.8a Literal and branch folding, dead code after folding
- [ ] M7.8 Folding, the rest: step 10
- [ ] M7.9 Scalar replacement and store-copy elision: step 10
- [ ] M7.10 Identical units: step 10, after M8.5

### M8 Canonical formation and the pure printer

- M8.1 merged into M5.2 and A1
- [~] M8.2 Canonical forms. Open: `fold_logical_*` as formation; the rest lands with each replacement
- [~] M8.3 Pure printer. Open: loop heads and logical statements as per-site spellings (with M9.3); the second half at step 10
- [ ] M8.4 Host modules: step 10
- [ ] M8.5 Runtime helpers as prelude code: step 10
- [ ] M8.6 Source maps: step 13
- [ ] M8.7 Port-shaped rules: continuous; the NO3 allowlist must end empty

### M9 One choice system

- [~] M9.1 Choice interface. Open: the rest, at step 11
- [~] M9.2 Objective as a judge. Open: gzip's own prior row
- [~] M9.3 Spelling families. Open: per-site spellings
- [~] M9.5 Naming. Open: the rest, at step 11
- [ ] M9.6 Property names: step 11
- [ ] M9.7 Layouts: step 11
- [~] M9.8 Data. Open: the rest, after M10.3
- [ ] M9.9 Function folding: step 11
- [ ] M9.10 Calibration: step 11
- [ ] M9.11 Order and locality: step 11
- [ ] M9.12 The diagnosis's codec-judged items: step 11
- M9.4 merged into M3.5

### M10 The final language

- [x] M10.2 The dynamic type (R12)
- [x] M10.4 Receivers and variadics (R7)
- [x] M10.7 Identity tests (R13)
- [x] M10.12 Trusted crossings (R1)
- [x] M10.15 Typed intrinsics (R10)
- [x] M10.16 Iteration (R14)
- [~] M10.9 Absence and integers (R2, R11). Open: R2's second batch (normalize at crossings, the refusals), `a.get(i)`
- [~] M10.11 Effects and pins (R15, R9). Done: the `debug` class. Open: termination (Y4), pins (step 12)
- [~] M10.13 Definite assignment (R3). Open: the refusal, with each port's release (M12.4)
- [~] M10.14 Reflection (R6). Done: the checker half. Open: constructibility (Y3), step 12
- [ ] M10.3 Const data (R9): step 11
- [ ] M10.19 Specialization over const data: step 11
- [ ] M10.1 Shapes (R5): step 12
- [ ] M10.8 Record spread and records (R5): step 12
- [ ] M10.5 Sealed virtuals and sum types (R8): step 12
- [ ] M10.6 ABI-valued enums (R8): step 12
- [ ] M10.18 Value structs (R4); `ref` removal after Y2: step 12
- [ ] M10.10 Sealed modules (R16): step 12
- [ ] M10.17 Host catalog and generics (R17, R18): step 12

### M11 Native

- [ ] M11.1 Toolchain owner: step 10
- [x] M11.2 Plain arithmetic
- [ ] M11.3 Externs per target: step 13
- [ ] M11.4 Portable records: step 13
- [ ] M11.5 Native from the optimized program: step 10, right after M7
- [ ] M11.6 Exceptions, suspension, regex, strings: step 13
- [ ] M11.7 Runtime and symbols: step 13
- [ ] M11.8 Native objective and the library ABI: step 13
- [ ] M11.9 Profiles: step 13
- [ ] M11.10 Host API: step 13
- [ ] M11.11 Native-complete definition: step 10, after M4.5
- [ ] M11.12 Cycles: step 13

### M12 Qualification and publication

- [~] M12.1 Ports own their sources. Open: `finer/port-migrations/` patches, per-port config keys, a recorded reason for every `assume_*`
- [ ] M12.2 No post-minifiers: motionlil at step 2, the others at step 13
- [ ] M12.3 Every library wins: step 13
- [ ] M12.4 Rebuild and publish: step 13
- [ ] M12.5 Receipts: step 13
- [ ] M12.6 Runtime parity and residual pairing: continuous

## Carried items

Open items the batches left, by owning task. An item leaves when its owner lands it.

- **Y5c / M3.3b:**
  - the IIFE default for application scripts;
  - delivery cases (script and CJS lanes) in the case runner;
  - `bare` for scripts;
  - the manifest's module names, `[path]` escaping, stale v2 manifests and `bundle.cost`;
  - comparing each delivered file with its part of the tree.
- **M3.3c:**
  - interfaces escaping at the one `END`;
  - `RootRow::completes`;
  - `single` with several entries.
- **M3.3e:** build motionlil's ten entries as one program.
- **M3.4:** the LSP and `lilscript-lint` check one root.
- **M3.5:**
  - level 0 still measures exactly;
  - monotone selection between two search levels;
  - the data estimator is raw bytes under every objective.
- **M3.9:** each render re-scores unchanged files.
- **M5.6:** `entry_graph` and the cycle refusal allocate outside `AllocationBudget`.
- **M8.5 / M3.3:** table decoders sit at the first module's root.
- **M8.7:** NO3's second half, thresholds without a policy source.
- **M9.1 / M9.5:** a naming-plan tie can land on the worse delivered text.
- **M7.9:**
  - a parameter written through a field inlines as a cell;
  - a struct copy passed as an argument holds a waiting operand.
- **M7.5:**
  - tail-return bodies;
  - closures created inside a body;
  - bodies across modules.
- **M9.3 / M10.4:**
  - mobxlil's law-P1 price;
  - non-private callbacks get the shared factory;
  - a rest list read with `.length` reads `arguments`.
- **M4 / M10:** four ignored production tests:
  - `??=` on a place;
  - generic methods;
  - an explicit `JS.undefined()` argument;
  - a detached `charCodeAt`.
- **M10.14:** the two wrapper-name cases, rewritten to the reflected-set contract.
- **M12.2:**
  - the nominal rename reverts (`~/lilscript-work/portwork/nominal/`);
  - mobxlil's development bundle and its host globals;
  - four site receipts.
- **Architecture §22:** "today the default strips it" is stale.
- **A1:** the case runner's CPU-time pair.
- **No owner yet:**
  - structs crossing to the host or through generics are refused (an `extern` taking a struct, a struct union widened to `JsValue`, a generic function taking a struct);
  - M4.1's gaps: generic classes, D2 adapters for a published constructor's struct parameters, and an import of a name that is both a type and a constructor;
  - `pool_strings` and `pack_string_arrays` ignore their permissions;
  - katexlil keeps `src/fontMetricsData.js` for two scripts.
- **Ledger rows owned by done tasks:**
  - M1.5's `objectHasOwn` prelude case;
  - M2.6's intermittent zodlil ReDoS test;
  - M2.8's mobxlil test-path mapping.

## Working on the migration

- **Starting a task:** read this file, then the plan's [rules](plan.md#rules-for-every-phase) and the task's phase section. The records in history.md and record-2026-09.md are not instructions; open one only when a task continues a batch.
- **Landing a batch:** in one commit, tick its tasks here, update "Now" and "Next steps", move the carried items, and add its record to history.md. Numbers and prose go in the record, never here or in the plan.
- **Changing the plan:** re-order "Order of work" with a note in history.md, or amend the architecture (§22). Task ids never change.
