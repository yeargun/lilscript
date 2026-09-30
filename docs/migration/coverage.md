# Coverage of unfinished work

This is an ownership map, not another task order or progress board. The [checklist](index.md) owns progress and the [plan](plan.md#7-work-packages-and-dependencies) defines the work packages. It accounts for all 95 unfinished M-tasks in the [archived checklist](../old-history/migration/index.md), followed by every carried-item entry. Completed tasks stay in the archive; their remaining defects are assigned below.

Old IDs remain valid historical aliases, including IDs in test ledgers, language clauses and source comments. Each row has a primary owner; that owner coordinates any supporting work package. Close the residual under its new owner when implementation and evidence land. Old step numbers are intentionally removed.

## Legacy task ownership

| Legacy ID | Owner | Remaining scope |
|---|---|---|
| M2.3 | [C2](plan.md#c2) | interpreter-generated oracles |
| M2.4 | [C2](plan.md#c2) | Interpreter extension |
| M2.7 | [C2](plan.md#c2) | Differential |
| M2.10 | [C2](plan.md#c2) | Benchmark contract |
| M2.11 | [C3](plan.md#c3) | Held-out libraries |
| M2.12 | [C2](plan.md#c2) | Perf runner, runtime ledger, counters, micro perf gates |
| M2.15 | [C3](plan.md#c3) | Calibration corpus |
| M3.1 | [C1](plan.md#c1) | `format` (M3.3b), `[resources]` (M5.6), `[performance]` (M2.12) |
| M3.2 | [C1](plan.md#c1) | Family registry |
| M3.3b | [D1](plan.md#d1) | Formats (work in progress: `~/lilscript-work/portwork/m3.3b-wip.patch`) |
| M3.3c | [D2](plan.md#d2) | Facts and choices |
| M3.3d | [D2](plan.md#d2) | Lazy effects and cycles |
| M3.3e | [D3](plan.md#d3) | Ports, with M12.2 for motionlil |
| M3.4 | [D3](plan.md#d3) | Public API and shared formation |
| M3.5 | [Q3](plan.md#q3) | `-j` scoring (after M5.6) |
| M3.7 | [C1](plan.md#c1) | Environment variables |
| M3.8a | [D2](plan.md#d2) | Consumer-shakeable delivery, first half |
| M3.8b | [D2](plan.md#d2) | Consumer-shakeable delivery, second half |
| M3.9 | [Q2](plan.md#q2) | Caches and the decision lock |
| M4.2 | [S4](plan.md#s4) | type parameters by id, interned types (with M4.4) |
| M4.3 | [S4](plan.md#s4) | parameter defaults on declarations |
| M4.4 | [S4](plan.md#s4) | statement ids, type parameters by id, interned types |
| M4.5 | [S4](plan.md#s4) | Contracts and capabilities at check time |
| M4.6 | [S4](plan.md#s4) | one operation identity in the IR (M5.2), `hasOwnProperty.call` and effect-free constructions (M10.17) |
| M5.1 | [Q2](plan.md#q2) | `UseIndex`, deleting `drop_unreferenced_functions` |
| M5.2 | [S4](plan.md#s4) | transfers for rule-created bindings and functions, and the deletions they allow (`quiet.rs` first) |
| M5.3a | [Q2](plan.md#q2) | the program rules' SCC order with a dirty worklist; classifying the remaining rules |
| M5.3b | [S4](plan.md#s4) | Transitional rules deleted |
| M5.6 | [Q2](plan.md#q2) | Resource accounting (counts) |
| M5.7 | [Q2](plan.md#q2) | Incremental tail |
| M6.1 | [S4](plan.md#s4) | deleting the tree's re-derivations |
| M6.2 | [S4](plan.md#s4) | the same deletions |
| M6.3 | [S1](plan.md#s1) | The `pure` contract. Computed, not consumed |
| M6.4a | [S1](plan.md#s1) | finite sets, `simplify::known`'s constant cases |
| M6.4b | [S1](plan.md#s1) | primitive classes, deleting formation's own number sources, `raw_domains.rs` and the binding-class derivation |
| M6.5 | [S4](plan.md#s4) | deleting `quiet.rs`'s order and `root_constants.rs`'s own proof |
| M6.6 | [S2](plan.md#s2) | Escape and uniqueness |
| M6.7 | [S2](plan.md#s2) | Field facts |
| M6.8 | [S3](plan.md#s3) | Frequency |
| M7.1 | [S1](plan.md#s1) | Removal |
| M7.3 | [S1](plan.md#s1) | callee-built defaults and deletion of the three tree passes (M5.3b) |
| M7.4 | [S1](plan.md#s1) | defines, deleting `eliminate_aliases` and `forward_root_constants` |
| M7.5 | [S3](plan.md#s3) | Inlining, the rest |
| M7.6 | [S2](plan.md#s2) | Namespaces and emulated methods |
| M7.7 | [S2](plan.md#s2) | Fields |
| M7.8 | [S1](plan.md#s1) | Folding, the rest |
| M7.9 | [S2](plan.md#s2) | Scalar replacement and store-copy elision |
| M7.10 | [S3](plan.md#s3) | Identical units, after M8.5 |
| M8.2 | [G3](plan.md#g3) | `fold_logical_*` as formation; the rest lands with each replacement |
| M8.3 | [G3](plan.md#g3) | loop heads and logical statements as per-site spellings (with M9.3); the second half |
| M8.4 | [D2](plan.md#d2) | Host modules |
| M8.5 | [Q4](plan.md#q4) | Runtime helpers as prelude code |
| M8.6 | [D2](plan.md#d2) | Source maps |
| M8.7 | [C3](plan.md#c3) | Port-shaped rules: continuous; the NO3 allowlist must end empty |
| M9.1 | [Q1](plan.md#q1) | the rest |
| M9.2 | [Q3](plan.md#q3) | gzip's own prior row |
| M9.3 | [G3](plan.md#g3) | per-site spellings |
| M9.5 | [G1](plan.md#g1) | the rest |
| M9.6 | [G2](plan.md#g2) | Property names |
| M9.7 | [Q1](plan.md#q1) | Layouts |
| M9.8 | [Q4](plan.md#q4) | the rest, after M10.3 |
| M9.9 | [S3](plan.md#s3) | Function folding |
| M9.10 | [Q3](plan.md#q3) | Calibration |
| M9.11 | [G3](plan.md#g3) | Order and locality |
| M9.12 | [G3](plan.md#g3) | The diagnosis's codec-judged items |
| M10.9 | [S4](plan.md#s4) | R2's second batch (normalize at crossings, the refusals), `a.get(i)` |
| M10.11 | [S4](plan.md#s4) | termination (Y4), pins |
| M10.13 | [S4](plan.md#s4) | the refusal, with each port's release (M12.4) |
| M10.14 | [S4](plan.md#s4) | constructibility (Y3) |
| M10.3 | [Q4](plan.md#q4) | Const data (R9) |
| M10.19 | [Q4](plan.md#q4) | Specialization over const data |
| M10.1 | [S4](plan.md#s4) | Shapes (R5) |
| M10.8 | [S4](plan.md#s4) | Record spread and records (R5) |
| M10.5 | [S4](plan.md#s4) | Sealed virtuals and sum types (R8) |
| M10.6 | [S4](plan.md#s4) | ABI-valued enums (R8) |
| M10.18 | [S4](plan.md#s4) | Value structs (R4); `ref` removal after Y2 |
| M10.10 | [S4](plan.md#s4) | Sealed modules (R16) |
| M10.17 | [S4](plan.md#s4) | Host catalog and generics (R17, R18) |
| M11.1 | [N1](plan.md#n1) | Toolchain owner |
| M11.3 | [N2](plan.md#n2) | Externs per target |
| M11.4 | [N2](plan.md#n2) | Portable records |
| M11.5 | [N1](plan.md#n1) | Native from the optimized program |
| M11.6 | [N2](plan.md#n2) | Exceptions, suspension, regex, strings |
| M11.7 | [N2](plan.md#n2) | Runtime and symbols |
| M11.8 | [N2](plan.md#n2) | Native objective and the library ABI |
| M11.9 | [N2](plan.md#n2) | Profiles |
| M11.10 | [N2](plan.md#n2) | Host API |
| M11.11 | [N1](plan.md#n1) | Native-complete definition, after M4.5 |
| M11.12 | [N2](plan.md#n2) | Cycles |
| M12.1 | [D3](plan.md#d3) | `finer/port-migrations/` patches, per-port config keys, a recorded reason for every `assume_*` |
| M12.2 | [D3](plan.md#d3) | No post-minifiers: motionlil, the others |
| M12.3 | [V1](plan.md#v1) | Every library wins |
| M12.4 | [V2](plan.md#v2) | Rebuild and publish |
| M12.5 | [V2](plan.md#v2) | Receipts |
| M12.6 | [V1](plan.md#v1) | Runtime parity and residual pairing: continuous |

S4 coordinates removal of duplicate fact derivations with their actual S/G/Q consumers. S2 owns namespace flattening in M7.6 and coordinates method devirtualization with S3. S3 supplies frequency facts to runtime classification and Q3. Native capability clauses in M4.5 are shared with N1. M10.18 keeps the current `ref` contract until an explicit language change; S2 can improve value-copy implementation beforehand.

## Carried-item ownership

The wording in this table identifies the previous checklist item; it is not a claim that a historical failure was reproduced during this documentation revision. Verify against the current case/ledger before changing code.

| Previous owner | Owner now | Item |
|---|---|---|
| Y5c / M3.3b | [D1](plan.md#d1) | the IIFE default for application scripts; |
| Y5c / M3.3b | [D1](plan.md#d1) | delivery cases (script and CJS lanes) in the case runner; |
| Y5c / M3.3b | [D1](plan.md#d1) | `bare` for scripts; |
| Y5c / M3.3b | [D1](plan.md#d1) | the manifest's module names, `[path]` escaping, stale v2 manifests and `bundle.cost`; |
| Y5c / M3.3b | [D1](plan.md#d1) | comparing each delivered file with its part of the tree. |
| M3.3c | [D2](plan.md#d2) | interfaces escaping at the one `END`; |
| M3.3c | [D2](plan.md#d2) | `RootRow::completes`; |
| M3.3c | [D2](plan.md#d2) | `single` with several entries. |
| M3.3e | [D3](plan.md#d3) | build motionlil's ten entries as one program. |
| M3.4 | [D3](plan.md#d3) | the LSP and `lilscript-lint` check one root. |
| M3.5 | [Q3](plan.md#q3) | level 0 still measures exactly; |
| M3.5 | [Q3](plan.md#q3) | monotone selection between two search levels; |
| M3.5 | [Q3](plan.md#q3) | the data estimator is raw bytes under every objective. |
| M3.9 | [Q2](plan.md#q2) | each render re-scores unchanged files. |
| M5.6 | [Q2](plan.md#q2) | `entry_graph` and the cycle refusal allocate outside `AllocationBudget`. |
| M8.5 / M3.3 | [Q4](plan.md#q4) | table decoders sit at the first module's root. |
| M8.7 | [C3](plan.md#c3) | NO3's second half, thresholds without a policy source. |
| M9.1 / M9.5 | [G1](plan.md#g1) | a naming-plan tie can land on the worse delivered text. |
| M7.9 | [S2](plan.md#s2) | a parameter written through a field inlines as a cell; |
| M7.9 | [S2](plan.md#s2) | a struct copy passed as an argument holds a waiting operand. |
| M7.5 | [S3](plan.md#s3) | tail-return bodies; |
| M7.5 | [S3](plan.md#s3) | closures created inside a body; |
| M7.5 | [S3](plan.md#s3) | bodies across modules. |
| M9.3 / M10.4 | [S3](plan.md#s3) | mobxlil's law-P1 price; |
| M9.3 / M10.4 | [S3](plan.md#s3) | non-private callbacks get the shared factory; |
| M9.3 / M10.4 | [S3](plan.md#s3) | a rest list read with `.length` reads `arguments`. |
| M4 / M10 | [S4](plan.md#s4) | `??=` on a place: implemented and qualified in `8ebb686d`; four carried tests enabled |
| M4 / M10 | [S4](plan.md#s4) | generic methods; |
| M4 / M10 | [S4](plan.md#s4) | an explicit `JS.undefined()` argument; |
| M4 / M10 | [S4](plan.md#s4) | a detached `charCodeAt`. |
| M10.14 | [G2](plan.md#g2) | the two wrapper-name cases, rewritten to the reflected-set contract. |
| M12.2 | [D3](plan.md#d3) | the nominal rename reverts (`~/lilscript-work/portwork/nominal/`); |
| M12.2 | [D3](plan.md#d3) | mobxlil's development bundle and its host globals; |
| M12.2 | [D3](plan.md#d3) | four site receipts. |
| Architecture §22 | Resolved by this replan: stale active text archived | "today the default strips it" is stale. |
| A1 | [C3](plan.md#c3) | the case runner's CPU-time pair. |
| No owner yet | [S4](plan.md#s4) | structs crossing to the host or through generics are refused (an `extern` taking a struct, a struct union widened to `JsValue`, a generic function taking a struct); |
| No owner yet | [S4](plan.md#s4) | M4.1's gaps: generic classes, D2 adapters for a published constructor's struct parameters, and an import of a name that is both a type and a constructor; |
| No owner yet | [C1](plan.md#c1) | `pool_strings` and `pack_string_arrays` ignore their permissions; |
| No owner yet | [D3](plan.md#d3) | katexlil keeps `src/fontMetricsData.js` for two scripts. |
| Ledger rows owned by done tasks | [S4](plan.md#s4) | M1.5's `objectHasOwn` prelude case; |
| Ledger rows owned by done tasks | [C2](plan.md#c2) | M2.6's intermittent zodlil ReDoS test; |
| Ledger rows owned by done tasks | [C2](plan.md#c2) | M2.8's mobxlil test-path mapping. |

All 43 carried-item entries from the snapshot are represented above. C3 records the missing historical A1 CPU pair as a limitation; it establishes a current baseline rather than inventing old measurements. D1 inspects the existing `~/lilscript-work/portwork/m3.3b-wip.patch` before continuing the format work.

## Compatibility and historical aliases

| Reference | Current disposition |
|---|---|
| M1.9 correctness debts | Struct/host/generic transport belongs to S4; wrapper-name/reflection cases to G2. C2 checks the harvested raw inlined-root-assignment case and its ledger disposition before treating it as an open regression |
| M3.6, M9.4 | Merged historically into M3.5; deterministic scoring/reuse belongs to Q2, schedule/selection to Q3 |
| M8.1 | Merged historically into annotations and formation; remaining transfers belong to S4 and their G/Q consumers |
| Y1, Y5a/b | Approved contracts retained. Y5c, the default application IIFE, belongs to D1 |
| Y2, Y3, Y4 | `ref`, exported constructibility and `pure` termination remain explicit language decisions in S4; this replan does not approve their semantic changes |
| Y6 | C3 owns a versioned win-label policy. Exact bytes are always reported; do not silently change historical verdict thresholds |
| Y7 | C1 owns effort/permission separation and diagnostics for existing level-16 and retired settings; compatibility is part of implementation |
| Former source-layout move | S4 owns relocation if it reduces ownership confusion; move code with its consumer migration, not as a separate prerequisite to optimization |
| Old proof/measurement laws | The active plan distinguishes sound semantic proofs, empirical defaults and exact candidate scores. Historical universal byte/noise/time claims are not inherited as guarantees |
