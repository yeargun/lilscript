# Migration progress

The one-compiler migration as a to-do list. [plan.md](plan.md) says what each task is and the order the work goes in. [history.md](history.md) records what each batch landed, measured and left open.

**Updated 2026-09-28**, at batch B1 on `finer/059-idiom-directed-naming`. Every batch branch is merged there.

## Where we are

- **Done:**
  - M0 and M1: there has been one compiler since 2026-09-24.
  - The interim release.
  - Batches I, T, D, N, A1, F1, F2, F3 and B1, and M3.3a.
  - Three of the tools: M2.13, M2.5 and M2.14.
  - Of the plan's 13 steps, steps 1, 4 and 5 are done and steps 2 and 3 are partly done.
- **Next on the critical path:** step 6 continues with batch B2 (level 0 without a codec, codec settings as objective configuration, deterministic parallel scoring, the count gates), then M3.1's first slice and M3.4. Batch B1 landed the counted walk: one list from A0, the proxy judge, schedule v1, passes to the fixed point and restarts from level 14; level-13 CPU fell to ×0.58 on katexlil and ×0.08 on jquerylil, and level 13 no longer reaches the beam's structural recipes (ledgered to M7.9, M7.5's rest and M9.1's rest).
- **Also open, off the critical path:**
  - Step 2: the layout commit, then M3.3b with M3.8a, then motionlil compiler-written (M12.2).
  - Step 3: M2.10, M2.11, M2.12 and M2.15.
- **Largest gaps:**
  - **Program rules are young.** F1 and F2 added DCE, folding and removal-only inlining on the Program IR: production module lanes −1,174 Brotli since A1, but `comparison/apps` is still 926 Brotli against 834 for Closure ADVANCED and 558 for the old route, because the ratchet's script lanes cannot seal their roots while every competitor treats top-level bindings as private: Y5.
  - **Compile time.** After B1, level 13 is below the frozen pre-M1 time on jquerylil (2.0 s against 28.8 s) and motionlil (5.3 s against 14.3 s), and above it on katexlil (10.4 s against 4.62 s) and the small ports, whose time is frontend, rules and formation. M3.5's exit needs katexlil at 6.0 s.
  - **Runtime.** katexlil's steady state in Node is 1.047× upstream KaTeX (after A1). M2.12's runtime ledger will track every port.
- **Owner rulings pending:** Y1–Y7 (architecture §21.1). None of them blocks the critical path.
- **How work runs:** one building, testing or measuring job at a time on this host (plan rule 5), with the full verification after each batch rather than each change (rule 8).
- **Owner decisions of 2026-09-28** ([brief](../../finer/intent/2026-09-28.md)):
  - Amendment AM1: budgets are counts, and the work-unit tariff and its machinery are dropped.
  - Law L22: decide statically first.
  - Implement the whole plan.
- **Batch records:** [M1](history.md#2026-09-24-m1-one-compiler), [I](history.md#2026-09-24-batch-i-effects-and-initialization-order), [release](history.md#2026-09-24-green-ci-and-the-interim-release), [T](history.md#2026-09-24-batch-t-the-terminal-challenger-stage), [D](history.md#2026-09-27-batch-d-choices-and-data-tables), [N](history.md#2026-09-27-batch-n-nominal-identity), [design v1](history.md#2026-09-27-design-version-1-and-the-plans-revision), [M3.3a](history.md#2026-09-27-m33a-the-delivery-contract-first-slice), [A1](history.md#2026-09-27-batch-a1-canonical-formation-without-tree-analysis), [tools](history.md#2026-09-27-the-tools-first-part-m213-m25-and-m214), [F1](history.md#2026-09-28-batch-f1-the-floor-slice-first-part-edit-kernel-dce-exact-values-folding), [F2](history.md#2026-09-28-batch-f2-the-floor-slice-second-part-graft-removal-only-inlining-typed-defaults), [F3](history.md#2026-09-28-batch-f3-the-floor-slice-third-part-parameters-and-returns-the-tree-twins), [B1](history.md#2026-09-28-batch-b1-the-counted-walk-m35-first-part).

## Steps

The plan's [order of work](plan.md#order-of-work). The critical path runs through steps 4 → 5 → 6 → 7 → 8 → 9 → 10 → 11 → 13 ([plan](plan.md#effort-and-critical-path)).

- [x] **1** katexlil released with its font metrics in LilScript: 2026-09-27
- [ ] **2** Delivery contract and motionlil. Done: M3.3a. Open: the layout commit (architecture §15), then M3.3b with M3.8a, then motionlil (M3.3e, M12.2)
- [ ] **3** Tools. Done: M2.5, M2.13, M2.14. Open: M2.10, M2.11, M2.12, M2.15, and M2.7; M2.4 starts here and then runs ahead of each fact
- [x] **4** Batch A1, canonical formation without tree analysis: 2026-09-27
- [x] **5** Floor slice: M5.1, M6.4a, M7.8a, M7.3, M7.5a: 2026-09-28 (batches F1–F3). Carried: M5.1's `UseIndex` and `drop_unreferenced_functions`, M6.4a's finite sets, the tree twins (measured, kept)
- [ ] **6** Budget model, counted (AM1): M3.1 (first slice), M3.4, M3.5; then M3.2, M3.9, M5.6, M3.7 as needed. *Partly:* B1 (M3.5's walk, 2026-09-28). **In progress**
- [ ] **7** Language slice 1: M4.2 with M10.2, then M10.4, M10.7, M10.16
- [ ] **8** The core: M4.6, M10.13, M10.9, M10.15, M10.11 (`debug` class), M10.14 (checker half); M10.12 after Y1
- [ ] **9** Machinery: M4.3, M4.4, M5.2, M5.5, M5.3a, M8.2 A2, M8.3 (first half)
- [ ] **10** Facts and the rest of the floor: M6.4b, M6.6, M6.7, M6.8, then M7.1, M7.4, M7.5, M7.6, M7.7, M7.8, M7.9, M8.5, M7.10, with M5.3b; M5.7, M8.3 (second half); M4.5, M11.1, M11.11, and M11.5 right after M7
- [ ] **11** One choice system: M10.3, M9.1's rest, M9.2's gzip row, M9.3 per site, M9.5, M9.6, M9.7, M9.8's rest, M9.9–M9.12, M10.19, M3.3c with M3.8b
- [ ] **12** The rest of M10: M10.1, M10.8, M10.5, M10.6, M10.18, M10.10, M10.17, M10.14's constructibility (after Y3), M10.11's termination (after Y4) and pins
- [ ] **13** Qualification and native: M12.2 for the other ports, M3.3d, M11.3, M11.4, M11.6–M11.10, M11.12, M12.3–M12.5; M12.6 throughout

## Tasks

`[x]` done · `[ ]` open · *partly*: some of it landed, and the note says what · the step number says when an open task is due.

### M0 Record and freeze: done 2026-09-23 · [plan](plan.md#m0-record-and-freeze)

- [x] M0 Record and freeze

### M1 One compiler: done 2026-09-24 · [plan](plan.md#m1-one-compiler-the-old-route-leaves-the-product)

- [x] M1.1 Move shared pieces
- [x] M1.2 Tools on the one compiler (LSP, playground, lint)
- [x] M1.3 CLI
- [x] M1.4 Configuration
- [x] M1.5 Tests: 288 harvested regression cases
- [x] M1.6 Delete the old route
- [x] M1.7 Names and docs
- [x] M1.8 Verify and gate the deletion
- [x] M1.9 Correctness debts: 4 of 6 fixed; the rest are under "Carried items" below

### M2 Verification ladder, baseline, interim release, benchmark contract: 8 of 15 · [plan](plan.md#m2-verification-ladder-baseline-and-interim-release)

- [x] M2.1 Green CI: 2026-09-24
- [x] M2.2 Case runner: with M1
- [ ] M2.3 Oracles. *Partly:* `print` is never stripped (A1). Interpreter-generated oracles are still open
- [ ] M2.4 Interpreter extension: step 3, then ahead of each fact
- [x] M2.5 Admission parse: 2026-09-27
- [x] M2.6 Port runner and ledger: with M1
- [ ] M2.7 Differential: step 3
- [x] M2.8 Baseline on one binary: 2026-09-24, for the 13 goal ports
- [x] M2.9 Interim release: 2026-09-24 ([report](../reports/2026-09-24-release.md)); motionlil on 2026-09-27
- [ ] M2.10 Benchmark contract: step 3
- [ ] M2.11 Held-out libraries: step 3
- [ ] M2.12 Perf runner, runtime ledger, counters, micro perf gates: step 3
- [x] M2.13 Generic corpus ratchet: 2026-09-27
- [x] M2.14 Frozen compile-time baselines: 2026-09-27 (`tests/compile-time/frozen-2026-09-27.json`)
- [ ] M2.15 Calibration corpus: step 3 (M3.5 no longer waits for it, AM1)

### M3 Configuration, public API, delivery contract, budget model: 1 of 13 · [plan](plan.md#m3-honest-configuration-one-public-api-delivery-contract-the-budget-model)

- [ ] M3.1 Schema v3. *Partly:* `[target.javascript] format` (`esm` only) and `[delivery]` with the `[bundle]` translator came with M3.3a. The first slice is due at step 6, `format` with M3.3b
- [ ] M3.2 Family registry: step 6
- [x] M3.3a Delivery, first slice (several entries, preserve-modules, lazy files, plan verifier, manifest v3): 2026-09-27
- [ ] M3.3b Formats: step 2. Work in progress is saved as `~/lilscript-work/portwork/m3.3b-wip.patch`
- [ ] M3.3c Facts and choices; its exit gates the consumer lanes: step 11
- [ ] M3.3d Lazy effects and cycles: step 13
- [ ] M3.3e Ports, with M12.2: step 2 for motionlil
- [ ] M3.4 Public API and shared formation: step 6
- [ ] M3.5 The budget model, counted (AM1): step 6. *Partly (B1): the proxy judge, schedule v1, one list walked in passes from A0, restarts and the beam in the tail, receipts, `scripts/monotone.mjs`. Open: level 0 without a codec, codec settings, `-j` scoring, count gates, the replay check, the audit lane, the time exit*
- M3.6 Codec pool: merged into M3.5
- [ ] M3.7 Environment variables: step 6
- [ ] M3.8a Consumer-shakeable delivery, first half: step 2, with M3.3b
- [ ] M3.8b Consumer-shakeable delivery, second half: step 11
- [ ] M3.9 Caches and the decision lock: step 6

### M4 Checker identities and checker-owned facts: 1 of 6 · [plan](plan.md#m4-checker-identities-and-checker-owned-facts)

- [x] M4.1 Nominal identity: 2026-09-27 (batch N)
- [ ] M4.2 The dynamic type, with M10.2: step 7
- [ ] M4.3 Checker facts transported. *Partly:* `assigned` split into `reassigned` and `observable_before_initialization`, with `ReadInitialization` (batch I). The rest is step 9
- [ ] M4.4 Node ids: step 9
- [ ] M4.5 Contracts and capabilities at check time: step 10
- [ ] M4.6 Operation catalog: step 8

### M5 The machinery: 1 of 8 · [plan](plan.md#m5-the-machinery)

- [ ] M5.1 Program edit kernel, DCE on both targets: step 5. *F1: DCE on both targets and the kernel's first part; F2: the graft. Open: signature changes (F3), `UseIndex`, deleting `drop_unreferenced_functions`*
- [ ] M5.2 Tree annotations and journal (absorbs M8.1): step 9
- [ ] M5.3a Scheduler: step 9
- [ ] M5.3b Transitional rules deleted: steps 10–12, each with its replacement
- [x] M5.4 Monotone selection and the terminal slot: 2026-09-24 (batch T). Monotone across levels closes with M3.5
- [ ] M5.5 Dataflow and views: step 9
- [ ] M5.6 Resource accounting: counts (AM1), retiring the allocation-exact ledgers; independent of M3.5
- [ ] M5.7 Incremental tail: step 10

### M6 The fact spine: 0 of 9 · [plan](plan.md#m6-the-fact-spine)

A fact counts as landed only once its re-derivations are deleted (plan rule 2).

- [ ] M6.1 Call graph and function facts. *Computed, not consumed* (batch I): lands when M5.2 deletes the re-derivations, at step 9
- [ ] M6.2 Effects. *Computed, not consumed* (batch I): lands with M5.2, at step 9
- [ ] M6.3 The `pure` contract. *Computed, not consumed* (batch I): lands with M5.2, at step 9
- [ ] M6.4a Values, exact tier: step 5. *F1: the exact tier. Open: finite sets, `simplify::known`'s constant cases*
- [ ] M6.4b Values, range tier: step 10
- [ ] M6.5 Initialization order. *Computed, not consumed* (batch I): lands with M5.2, at step 9
- [ ] M6.6 Escape and uniqueness: step 10
- [ ] M6.7 Field facts: step 10
- [ ] M6.8 Frequency: step 10

### M7 Program rules, the floor: 4 of 12 · [plan](plan.md#m7-program-rules-the-floor)

- [ ] M7.1 Removal: step 10
- [x] M7.2 Discarded effect-free calls: 2026-09-24 (batch I)
- [x] M7.3 Parameters and returns: 2026-09-28 (batches F2, F3). *Signatures change per coupling class. The three default passes stay: measured, they still act on script roots and callee-built defaults (Y5)*
- [ ] M7.4 Root constants, defines, forwarding: step 10
- [x] M7.5a Removal-only inlining: 2026-09-28 (batch F2), first version: one exit, no function created inside, one module. *Tail returns, closures inside bodies and cross-module bodies go to M7.5; the tree inliners stay until then*
- [ ] M7.5 Inlining, the rest: step 10
- [ ] M7.6 Namespaces and emulated methods: step 10
- [ ] M7.7 Fields: step 10
- [x] M7.8a Literal and branch folding: 2026-09-28 (batch F1)
- [ ] M7.8 Folding, the rest: step 10
- [ ] M7.9 Scalar replacement and store-copy elision: step 10
- [ ] M7.10 Identical units: step 10, after M8.5

### M8 Canonical formation and the pure printer: 0 of 6 · [plan](plan.md#m8-canonical-formation-and-the-pure-printer)

- M8.1 Formation writes annotations: merged into M5.2 and batch A1 (A1's part, import identity, is done)
- [ ] M8.2 Canonical forms. *Partly:* batch A1 was done on 2026-09-27. A2 is at step 9, and the rest lands with each replacement
- [ ] M8.3 Pure printer: first half at step 9, second half at step 10
- [ ] M8.4 Host modules: step 10
- [ ] M8.5 Runtime helpers as prelude code: step 10
- [ ] M8.6 Source maps: step 13
- [ ] M8.7 Port-shaped rules: continuous through steps 9–11; the NO3 allowlist must end empty

### M9 One choice system: 0 of 11 · [plan](plan.md#m9-one-choice-system-choices-naming-layouts-and-data)

- [ ] M9.1 Choice interface. *Partly:* the first slice (choice kernel, with data as its first family) came in batch D. The rest is step 11
- [ ] M9.2 Objective as a judge. *Partly:* the terminal families (batch T). gzip's own prior row is step 11
- [ ] M9.3 Spelling families. *Partly:* the first family as module-wide booleans (batch T). Per-site spellings follow M8.3, at step 11
- M9.4 Estimator and finalists: merged into M3.5
- [ ] M9.5 Naming. *Partly:* the seed runs at every level (A1). The rest is step 11
- [ ] M9.6 Property names: step 11
- [ ] M9.7 Layouts: step 11
- [ ] M9.8 Data. *Partly:* tables as a codec-judged choice (batch D). The rest follows M10.3, at step 11
- [ ] M9.9 Function folding: step 11
- [ ] M9.10 Calibration: step 11
- [ ] M9.11 Order and locality: step 11
- [ ] M9.12 The diagnosis's codec-judged items: step 11

### M10 The final language: 0 of 19 · [plan](plan.md#m10-the-final-language-r1r18)

- [ ] M10.2 The dynamic type (R12), with M4.2: step 7
- [ ] M10.4 Receivers and variadics (R7): step 7
- [ ] M10.7 Identity tests (R13): step 7
- [ ] M10.16 Iteration (R14): step 7
- [ ] M10.13 Definite assignment (R3), two batches: step 8
- [ ] M10.9 Absence and integers (R2, R11), two batches: step 8
- [ ] M10.15 Typed intrinsics (R10): step 8
- [ ] M10.11 Effects and pins (R15, R9): the `debug` class at step 8; termination (after Y4) and pins at step 12
- [ ] M10.14 Reflection (R6): the checker half at step 8; constructibility (after Y3) at step 12
- [ ] M10.12 Trusted crossings (R1): step 8, after Y1
- [ ] M10.3 Const data (R9): step 11
- [ ] M10.19 Specialization over const data (R9): step 11
- [ ] M10.1 Shapes (R5): step 12
- [ ] M10.8 Record spread and records (R5): step 12
- [ ] M10.5 Sealed virtuals and sum types (R8): step 12
- [ ] M10.6 ABI-valued enums (R8): step 12
- [ ] M10.18 Value structs (R4); `ref` removal after Y2: step 12
- [ ] M10.10 Sealed modules (R16): step 12
- [ ] M10.17 Host catalog and generics (R17, R18): step 12

### M11 Native: 1 of 12 · [plan](plan.md#m11-native-the-whole-language-directly-executable)

- [ ] M11.1 Toolchain owner: step 10
- [x] M11.2 Plain arithmetic: 2026-09-27 (batch A1)
- [ ] M11.3 Externs per target: step 13
- [ ] M11.4 Portable records: step 13
- [ ] M11.5 Native from the optimized program: step 10, right after M7
- [ ] M11.6 Exceptions, suspension, regex, strings: step 13
- [ ] M11.7 Runtime and symbols: step 13
- [ ] M11.8 Native objective and the library ABI: step 13, before M11's exit
- [ ] M11.9 Profiles: step 13
- [ ] M11.10 Host API: step 13
- [ ] M11.11 Native-complete definition: step 10, after M4.5
- [ ] M11.12 Cycles: step 13

### M12 Qualification and publication: 0 of 6 · [plan](plan.md#m12-qualification-and-publication)

- [ ] M12.1 Ports own their sources. *Partly:* each port's rewrite has been in its own repository since M2.8. `finer/port-migrations/` still holds patches, per-port keys leave the configs with M3.5, and every `assume_*` still needs a recorded reason
- [ ] M12.2 No post-minifiers: motionlil at step 2, the other ports at step 13
- [ ] M12.3 Every library wins: step 13
- [ ] M12.4 Rebuild and publish: step 13
- [ ] M12.5 Receipts: step 13
- [ ] M12.6 Runtime parity and residual pairing: continuous since A1 (katexlil's idioms can start)

## Carried items

Open items that the batch records left, grouped by the task that owns them. An item leaves this list when its owner lands it.

- **M3.3b:**
  - delivery cases in the case runner, with the script and CJS lanes;
  - the manifest's module names, `[path]` escaping, stale v2 manifests and `bundle.cost`;
  - `bare` for script builds;
  - comparing each delivered file with its part of the tree (from M2.5).
- **M3.3c:**
  - interfaces escape at the one `END` (R1, R2);
  - `RootRow::completes`;
  - `single` with several entries delivers definitions only another entry needs.
- **M3.3e:** the scratch build of motionlil's ten entries as one program was never run.
- **M3.4:** the LSP and `lilscript-lint` check one root.
- **M3.5:**
  - level 0 still measures exactly;
  - monotone selection between two search-enabled levels;
  - the data estimator is raw bytes under every objective.
- **M3.9:** each render re-scores unchanged files.
- **M5.6:** `entry_graph` and the cycle refusal allocate outside `AllocationBudget`.
- **M8.5 and M3.3:** table decoders are declared at the first module's root. They move to prelude code (M8.5) and are placed per chunk (M3.3).
- **M8.7:** NO3's second half, thresholds without a policy source, is not checked yet.
- **M9.1 and M9.5:** the search scores a render the terminal stage then re-spells, so a tie between naming plans can land on the worse delivered text; `Style`'s derived order keeps `Global` on a tie (batch F1: two cases +6 and +7 Brotli).
- **M7.9:** a parameter written through a field inlines as a cell (`{let e=d;…}`, +9 raw), and a struct copy for an argument makes formation hold a waiting operand (+6 raw) (batch F2).
- **M7.5's rest:** tail-return bodies need a result declared without a value; closures created inside a body; bodies across modules (batch F2).
- **M9.3 and M10.4:**
  - mobxlil +72 Brotli after A1, law P1's price;
  - callbacks that are not private still get the shared factory;
  - a rest list read with `.length` reads `arguments`.
- **M4 and M10:** four ignored production tests carried from M1: `??=` on a place (the ratchet ledger names M10.9), generic methods, an explicit `JS.undefined()` argument taking the default, and a detached `charCodeAt`.
- **M10.14:** the two wrapper-name cases get rewritten to what the reflected-set contract guarantees.
- **M12.2:**
  - the nominal rename reverts (`~/lilscript-work/portwork/nominal/`) land with each port's next release;
  - mobxlil's development bundle needs its host globals;
  - two site receipts need refreshing.
- **Architecture amendment (§22):** the text "today the default strips it" is stale since A1.
- **Owed by batch A1:** the case runner's CPU-time pair (rule 3).
- **Y5:** the ratchet's `comparison/cases` compile as scripts with global roots, while every competitor recipe treats top-level bindings as private; `functions/nested-local` reaches its bar only as a module (batch F1).
- **No owner yet. Each needs a task, or an owner ruling:**
  - structs crossing to the host or through generics are refused: an `extern` taking a struct, a struct union widened to `JsValue`, and a generic function taking a struct. The case ledger still names M1.9, a closed task, and the plan names no later one;
  - M4.1's gaps:
    - generic classes are kept;
    - D2 adapters for a published constructor's struct parameters;
    - importing a name that exports both a type and a constructor binds only the type;
  - `pool_strings` and `pack_string_arrays` ignore their permissions and packing's startup risk;
  - katexlil's font-metrics change keeps `src/fontMetricsData.js` for two of its scripts (batch D).
- **Ledger rows owned by tasks marked done:**
  - M1.5: one case, a name-keyed `objectHasOwn` prelude;
  - M2.6: zodlil's ReDoS test, which fails on host timing and is intermittent;
  - M2.8: mobxlil's upstream-test path mapping.

## Working on the migration

- **Starting a task.** Read this file first. Then read the plan's [rules](plan.md#rules-for-every-phase), the task's phase section, and the architecture sections that section cites. history.md and record-2026-09.md are records, not instructions: open one only when the task continues a batch.
- **Landing a batch.** In one commit:
  - tick its tasks here, with the date (and *partly* when only part landed);
  - move "Next" in "Where we are" if the step changed;
  - move carried items;
  - add its record to [history.md](history.md): what landed, evidence, and what stays open, each with its owner.

  Numbers and prose go in the record, never here or in the plan.
- **Changing the plan.** Re-order "Order of work" with a note in history.md, or amend the architecture (§22). Task ids never change.
