# Coverage of unfinished work

This is an ownership map, not another task order or progress board. The [checklist](index.md) owns progress and the [plan](plan.md#7-work-packages-and-dependencies) defines the work packages. It accounts for all 95 unfinished M-tasks in the [archived checklist](../old-history/migration/index.md), followed by every carried-item entry. Completed tasks stay in the archive; their remaining defects are assigned below.

Old IDs remain valid historical aliases, including IDs in test ledgers, language clauses and source comments. Each row has a primary owner; that owner coordinates any supporting work package. Close the residual under its new owner when implementation and evidence land. Old step numbers are intentionally removed.

## Legacy task ownership

| Legacy ID | Owner | Remaining scope |
|---|---|---|
| M2.3 | [C2](plan.md#c2), complete | Independent generation, disagreement refusal and measured coverage receipts |
| M2.4 | [C2](plan.md#c2), complete | Nominal/collection/generic/UTF-16 models; unsupported host/effect features explicitly use separate independent observers |
| M2.7 | [C2](plan.md#c2), complete | Typed generation, feature masks, native/C lanes and regression-prologue migration |
| M2.10 | [C2](plan.md#c2), complete | Pinned recipe grid and common behavior oracles; broader fleet verdicts remain V1 |
| M2.11 | [C3](plan.md#c3), complete | Content-locked protected algorithm-library set and exposure protocol; maintained/external library qualification remains V1 |
| M2.12 | [C2](plan.md#c2), complete | Paired controls/intervals, operation inventories and identity-scoped ledger; affected-workload runtime qualification remains V1 |
| M2.15 | [C3](plan.md#c3), complete | Medium generic corpus, per-objective pruning audit, leave-one-family-out analysis and compile-work baseline |
| M3.1 | [C1](plan.md#c1) | Configuration contract; remaining format producers belong to D1, instrumentation to Q2 and runtime evidence to C2 |
| M3.2 | [C1](plan.md#c1) | Family registry; new producers register permissions within their owning milestone |
| M3.3b | [D1](plan.md#d1) | Formats (work in progress: `~/lilscript-work/portwork/m3.3b-wip.patch`) |
| M3.3c | [D2](plan.md#d2) | Facts and choices |
| M3.3d | [D2](plan.md#d2) | Lazy effects and cycles |
| M3.3e | [D3](plan.md#d3) | Ports, with M12.2 for motionlil |
| M3.4 | [D3](plan.md#d3) | Remaining graph consumers and shared formation; independent TOML/API/CLI objective sets implemented |
| M3.5 | [Q3](plan.md#q3) | `-j` scoring (after M5.6) |
| M3.7 | [C1](plan.md#c1) | Environment variables |
| M3.8a | [D2](plan.md#d2) | Consumer-shakeable delivery, first half |
| M3.8b | [D2](plan.md#d2) | Consumer-shakeable delivery, second half |
| M3.9 | [Q2](plan.md#q2) | Caches and the decision lock |
| M4.2 | [S4](plan.md#s4) | Implemented: type-parameter identity and canonical checked types |
| M4.3 | [S4](plan.md#s4) | Implemented: defaults evaluated in the selected declaration's scope |
| M4.4 | [S4](plan.md#s4) | Implemented: statement ids, binder identity and canonical checked types |
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
| M6.3 | [S1](plan.md#s1), implementation complete | The `pure` contract. Computed, not consumed |
| M6.4a | [S1](plan.md#s1), implementation complete | finite sets, `simplify::known`'s constant cases |
| M6.4b | [S1](plan.md#s1), then [S4](plan.md#s4) | S1 complete: shared primitive classes/ranges and equivalent formation number sources; S4: absorb/delete recipe-dependent `raw_domains.rs` and the target binding-class derivation after S2/Q1 consumers migrate |
| M6.5 | [S4](plan.md#s4) | deleting `quiet.rs`'s order and `root_constants.rs`'s own proof |
| M6.6 | [S2](plan.md#s2), complete | Bounded allocation/alias/escape facts and complete-use physical ownership witnesses |
| M6.7 | [S2](plan.md#s2), complete | Field reads, writes, constants, identity and boundary observations |
| M6.8 | [S3](plan.md#s3), complete | Bounded caller-path frequency, conservative recursion/host/loop handling and counted consumption in capture placement |
| M7.1 | [S1](plan.md#s1), implementation complete | Removal |
| M7.3 | [S1](plan.md#s1), implementation complete | callee-built defaults and deletion of the three tree passes (M5.3b) |
| M7.4 | [S1](plan.md#s1), then [S4](plan.md#s4) | S1 complete: source value/alias forwarding; S4 after Q1: replace/delete target alias and literal normalization for representation-created bindings |
| M7.5 | [S3](plan.md#s3), complete | Cross-module calls, terminal returns, fresh nested closures and objective-judged repeated expression bodies |
| M7.6 | [S2](plan.md#s2), complete; [S3](plan.md#s3), complete | Private lexical namespaces flatten through shared field proofs; known private calls use shared call evidence |
| M7.7 | [S2](plan.md#s2), complete | Safe fixed-field forwarding, unread fields and overwritten stores; initializer effects and normalization preserved |
| M7.8 | [S1](plan.md#s1) and [S2](plan.md#s2), implementation complete | Primitive folding plus bounded fresh array/object/record store collection with the correct prototype contract |
| M7.9 | [S2](plan.md#s2), complete; formation dependencies below | Shared scalar banks/record aliases, owned JavaScript product updates and native final-use transfers |
| M7.10 | [S3](plan.md#s3), complete | Identity-safe primitive body sharing; general runtime-prelude ownership remains Q4 (M8.5) |
| M8.2 | [G3](plan.md#g3), complete | Logical normalization remains a classified, decreasing target rule with capture/HTMLDDA guards; printer recognizers moved to proved formation |
| M8.3 | [G3](plan.md#g3), complete | Loop, statement, compound, quote and edition-gated modern forms have stable sites and prepared printer forms |
| M8.4 | [D2](plan.md#d2) | Host modules |
| M8.5 | [Q4](plan.md#q4) | Runtime helpers as prelude code |
| M8.6 | [D2](plan.md#d2) | Source maps |
| M8.7 | [C3](plan.md#c3), complete | Source allowlist is empty and enforced; generic policy/provenance replaces library citations |
| M9.1 | [Q1](plan.md#q1), complete | Shared family/site/alternative contract, per-helper choices, bounded coupled moves and replay admission |
| M9.2 | [Q3](plan.md#q3) | gzip's own prior row |
| M9.3 | [G3](plan.md#g3), complete | Common family/site assignments, permission vetoes and admitted final-byte choices |
| M9.5 | [G1](plan.md#g1), complete | Live printed-order allocation, independent local/root frequency, hygienic reuse, full continuations and controlled joint alphabets |
| M9.6 | [G2](plan.md#g2) | Complete in G2: private generic/observed eligibility, inherited/sibling slot reuse and reflected boundaries |
| M9.7 | [Q1](plan.md#q1), complete | Record/product/call layouts use the common family contract with typed proof payloads and compatible recipe combinations |
| M9.8 | [Q4](plan.md#q4) | the rest, after M10.3 |
| M9.9 | [S3](plan.md#s3), complete | Constant signatures, alpha-equivalent bodies and opt-in parameterized helpers compete on final bytes |
| M9.10 | [Q3](plan.md#q3) | Calibration |
| M9.11 | [G3](plan.md#g3), complete | Effect-free declaration/closure/literal groups, module/anchor barriers and gzip-window evidence |
| M9.12 | [G3](plan.md#g3), complete | Optional forms and receiver aliases compete through the common exact artifact path |
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

S4 coordinates removal of duplicate fact derivations with their actual S/G/Q consumers. S2 completes source-level namespace flattening in M7.6; S3 completes known private-call transport and folding under complete-use evidence. Target scalar/namespace cleanup still sees bindings created by inlining and representation choices after source rules. Q1 now provides their common family identities; S4 owns the fact-consumer replacement and deletion. S3 supplies counted frequency facts to capture placement and Q3. Native capability clauses in M4.5 are shared with N1. M10.18 keeps the current `ref` contract; S2 improves physical value-copy implementation under that contract. The [S2 acceptance record](s2-acceptance.md) makes these dependencies explicit.

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
| M8.7 | [C3](plan.md#c3), resolved | NO3's second half is empty; every retired threshold has a generic owner and historical provenance outside compiler sources. |
| M9.1 / M9.5 | [G1](plan.md#g1), complete | Equal exact objective costs compare complete raw bytes before identities; memo/replay retain the full naming choice |
| M7.9 | [S3](plan.md#s3), resolved | Field-written reference parameters forward the evaluated reference when whole-cell writes, ref and captures are absent; value-copy parameters retain their snapshot. |
| M7.9 | [S3](plan.md#s3), resolved | Immutable initialized product roots rematerialize through copy chains without a waiting operand; mutation, reentry and representation-specific snapshots retain storage. |
| M7.5 | [S3](plan.md#s3), resolved | tail-return bodies; |
| M7.5 | [S3](plan.md#s3), resolved | closures created inside a body; |
| M7.5 | [S3](plan.md#s3), resolved | bodies across modules. |
| M9.3 / M10.4 | [G3](plan.md#g3), complete | Guarded receiver aliases for repeated `this` reads; derived constructors, direct eval and shared owners excluded. Affected runtime evidence retained, broad parity remains V1. |
| M9.3 / M10.4 | [S4](plan.md#s4) | Implemented: non-private callbacks use the common value codec with typed receiver, default/rest and reflected-length contracts. |
| M9.3 / M10.4 | [S4](plan.md#s4) | Implemented: callable adapters use actual rest arrays, preserving `.length`, undefined/default behavior and variable indices. |
| M4 / M10 | [S4](plan.md#s4) | `??=` on a place: implemented and qualified in `8ebb686d`; four carried tests enabled |
| M4 / M10 | [S4](plan.md#s4) | Generic methods implemented and checked on JavaScript/native; observed generic classes remain open |
| M4 / M10 | [S4](plan.md#s4) | an explicit `JS.undefined()` argument; |
| M4 / M10 | [S4](plan.md#s4) | a detached `charCodeAt`. |
| M10.14 | [G2](plan.md#g2) | Complete in G2: both wrapper-name cases explicitly select name preservation and expect anonymous adapters. |
| M12.2 | [D3](plan.md#d3) | the nominal rename reverts (`~/lilscript-work/portwork/nominal/`); |
| M12.2 | [D3](plan.md#d3) | mobxlil's development bundle and its host globals; |
| M12.2 | [D3](plan.md#d3) | four site receipts. |
| Architecture §22 | Resolved by this replan: stale active text archived | "today the default strips it" is stale. |
| A1 | [C3](plan.md#c3), resolved | The missing historical CPU pair is explicitly unavailable; a current repeated per-cell baseline is retained without a cross-version claim. |
| No owner yet | [S4](plan.md#s4) | Concrete generic struct schemas and nullable/default/rest public adapters are implemented; opaque unions, mutable collection aliases and wider generic callable transport remain open. |
| No owner yet | [S4](plan.md#s4) | M4.1: published constructors/prototype methods with struct parameters and dual type/constructor imports/re-exports are implemented; observed generic classes remain open. |
| No owner yet | [C1](plan.md#c1) | `pool_strings` and `pack_string_arrays` ignore their permissions; |
| No owner yet | [D3](plan.md#d3) | katexlil keeps `src/fontMetricsData.js` for two scripts. |
| Ledger rows owned by done tasks | [S4](plan.md#s4), resolved | Object.hasOwn's explicit host binding and forwarded-call contract pass eight script/module lanes; stale ledger entry removed |
| Ledger rows owned by done tasks | [C2](plan.md#c2), resolved | Zod ReDoS timeout remains an exact host-dependent upstream-analysis ledger entry; unchanged assertion, quiet-host C1 suite passes |
| Ledger rows owned by done tasks | [C2](plan.md#c2), resolved | MobX mapping committed locally at `ccccb1d`; all three affected public Jest tests pass, stale path entries removed |

The three M9.3/M10.4 rows above follow their actual dependencies: the [archived A1 diagnosis](../old-history/migration/history.md) identifies receiver aliasing as a spelling choice and the other two as R7 language changes. S3 private direct-call optimization does not authorize changing those adapter contracts.

All 43 carried-item entries from the snapshot are represented above. C3 records the missing historical A1 CPU pair as a limitation and establishes a current baseline rather than inventing old measurements. D1 inspects the existing `~/lilscript-work/portwork/m3.3b-wip.patch` before continuing the format work.

## Compatibility and historical aliases

| Reference | Current disposition |
|---|---|
| M1.9 correctness debts | Struct/host/generic transport belongs to S4; wrapper-name/reflection cases to G2. C2 confirms the harvested root-assignment case passes all four JavaScript case lanes, including raw; it is not an open regression |
| M3.6, M9.4 | Merged historically into M3.5; deterministic scoring/reuse belongs to Q2, schedule/selection to Q3 |
| M8.1 | Merged historically into annotations and formation; remaining transfers belong to S4 and their G/Q consumers |
| Y1, Y5a/b | Approved contracts retained. Y5c, the default application IIFE, belongs to D1 |
| Y2, Y3, Y4 | `ref`, exported constructibility and `pure` termination remain explicit language decisions in S4; this replan does not approve their semantic changes |
| Y6 | C3 complete: `exact-bytes-v1` labels every eligible negative byte delta as smaller, reports exact bytes and leaves historical verdict thresholds immutable |
| Y7 | C1 supplies explicit policy version 3 and diagnosed version-2 compatibility; retired aliases remain supported only with consistent values |
| Former source-layout move | S4 owns relocation if it reduces ownership confusion; move code with its consumer migration, not as a separate prerequisite to optimization |
| Old proof/measurement laws | The active plan distinguishes sound semantic proofs, empirical defaults and exact candidate scores. Historical universal byte/noise/time claims are not inherited as guarantees |

G1 qualification found an existing **S4** blocker: frozen C3 `records-128` fails
source-program verification with `IntBinary(Add)` after program rules in both Q1
and G1, including with inlining or scalar replacement disabled. The 64-helper
prefix passes; 128 fails. [Reproduction and evidence](../../benchmarks/migration-results/2026-10-01-g1-complete/README.md#existing-refusal-found-during-qualification).
S4's contracts batch fixes the refined result view and passes the independent
eight-input oracle under all three objectives. The source stays frozen.
