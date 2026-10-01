# Q2 target use index and admitted graph work

1. Replace the private four-round unreferenced-function traversal with one
   admitted dependency/use index rooted at evaluated statements, exports and
   pinned bindings. Deferred function and inert initializer dependencies are
   reached only when their binding is observed. Preserve effects and lexical
   dependencies; unused recursive components may disappear as a unit.
2. Share statement/expression visitors with the target tree. Charge graph
   vertices/edges, sorting/traversal and retained results before allocations.
   Remove the former traversal after its integrated consumer works.
3. Migrate entry/delivery graph and cycle bookkeeping to admitted owners without
   changing evaluation order, lazy roots or cycle refusal behavior.

Prediction: chains/cycles need one liveness solve rather than repeated whole
walks. Existing rooted behavior stays exact; previously retained dead cycles
can shrink output. Resource refusals release all temporary backing. Compare
focused independent behavior oracles, graph/cycle cases and affected objective
artifacts after this coherent batch. Q2 remains active for dirty scheduling,
formation and persistent reuse; no broad suite/fleet or premature speed claim.

## Result

The admitted use graph replaces the old four-pass pruning routine. Roots are
executed regions, pinned bindings and exports; only reached declarations expose
their bodies/initializers. Dead chains and recursive groups close in one solve.
Unreachable arena remnants do not produce false semantic progress edits. Root
rows retain exactly the same statement mask.

Checking and delivery now share admitted iterative graph routines. Delivery
reuses static orders for lazy discovery. Tarjan consumes its existing component
stack; cycle-entry lookup uses a bitset, and cycle refusal compares each cycle's
first entry instead of every pair of roots. Projection storage releases after
placement without releasing target mutations. Diagnostic formatting remains
at the reporting boundary.

The focused run exposed a previously existing delivery bug: inserting setters
after spelling formation left cached print forms behind. Delivery now re-proves
those forms with the same permissions/assignments, without repeating structural
alias/declaration changes. It releases the replaced form storage. A separate
oracle proves an old logical-assignment form cannot bypass the new setter call.

43 distinct affected tests pass across the focused runs. The initial run had
40/42 passing: the delivery defect above and an incorrect new test expectation
for cyclic DFS order. After fixes, only affected checks were repeated; a new
regression fixture's forward expression edge was corrected to follow the real
setter edit. No full library or fleet run.

Release pin `/home/azureuser/lilscript-work/bin/q2-use-index-1/lilscript`, SHA-256
`563df22dfa771db23006916ada15442460f9a6ed93a219a0babdc9a780a66bad`;
algorithm 43 / search 32 / walk 10 / local facts 15. Release build: 58.91 seconds.
[identity.json](identity.json) pins source hashes; [comparison.json](comparison.json)
retains all artifact sizes, work counts, old setter failures and oracles.

| Matched program | Raw before = after | Gzip before = after | Brotli before = after |
|---|---:|---:|---:|
| Public product | 175 | 134 | 112 |
| Dead recursive group | 50 | 67 | 54 |
| Two-file control | 66 | 106 | 74 |
| **Total** | **291** | **307** | **240** |

21 release artifact oracles pass, with scored bytes equal to delivered bytes.
The old release panics on the setter case in all three lanes. The repaired
output is 154 raw / 177 gzip / 137 Brotli; it observes the independently expected
result 7. This repair is separate from the matched size totals. The graph tests
cover 4,096-node depth, independent transitive-closure SCC checks, exported and
pinned recursion, captures, side effects, row alignment and admission cleanup.
No fleet, CPU-speed, Closure or completed-Q2 claim. Delivery placement, dirty
scheduling, incremental formation and persistent reuse still belong to Q2.
