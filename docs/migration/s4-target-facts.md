# S4 target fact ownership

The shared Program owns source meaning. Target formation also creates codec
closures, storage, receiver adapters and representation helpers after source
optimization. Those nodes need physical facts; deleting their normalization
would leave required ABI machinery unnecessarily large.

| Question | Owner and consumers |
|---|---|
| What can source operations return or observe? | `program::facts`, including domains, calls, effects, values, fields and initialization; formation carries proved annotations |
| What writes or observes a physical binding? | `js::facts::BindingUse`, accumulated once by the admitted reachable-tree walk; expression/statement inlining, alias and namespace cleanup, initializer cleanup, array receiver lowering and root ordering consume it |
| When is a physical binding initialized? | `js::facts::BindingFacts` consumes Program initialization stamps and current reachable writes, with conservative target order for generated storage; constants and value placement consume the same answer |
| What primitive category can a generated expression return? | `js::facts_values` transfers syntax; pristine host calls use `catalog::host_values`, keyed by checked host identity |
| Can a generated operation run user code? | `js::facts::operation_behaviour` uses unchanged source stamps first, then conservative physical transfer; placement and frame-elision queries share that owner |
| Is a value safe to substitute at this particular site? | The transformation retains its local argument-order, capture, initialization, identity and strictness obligations; a primitive category or pure call alone does not establish these |

Reach facts describe one current tree revision and release with their admitted
analysis owner. Every subsequent request rebuilds them after edits. Dead arena
nodes are not live writes; imports and implicit loop/catch writes are represented.
Operation stamps apply only while their node is unchanged. Invalidation never
leaves a copied annotation attached to a different operation.

The separate write/call scans in target inliners, alias/namespace cleanup and
initializers, the array-origin scan, duplicate closure-factory scans, copied
binding-class table and inliner operation-effect table are removed. Primitive
host-result tables have moved from operator simplification into the catalog.
Earlier S4 work removed `raw_domains.rs` and `quiet.rs`; neither remains a
consumer obligation.

Retained rules use the explicit `Rule::progress` contract. Most remove reachable
functions/nodes; others retire binding/member reads, object recipes, call sites
or observed names. Empty declarations move strictly earlier. Operator reduction
orders node count before strict-comparison count, covering shorter equal-node
forms. Debug/test execution verifies the measure and edit journal.

Inlining retains bounded template/nesting admission. Registered Q1 alternatives
are one-way producers in each fresh candidate and may grow syntax. Both must
reach the scheduler's fixed point within its fail-closed round bound; an
unfinished tree is never published. These are not claims that fewer nodes or a
chosen representation always compress better. Existing family permissions and
exact final-artifact admission remain in force.

Q2 owns dirty scheduling, persistent reuse and replacement of the remaining
private liveness traversal. Q4 owns const-data/prelude producers. These later
owners are explicit dependencies, not stale S4 transition labels. S4 author
controls and the [supported public ABI/capability contract](s4-public-abi.md)
are complete.
