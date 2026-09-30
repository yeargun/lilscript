# S1 — primitive facts, evaluation and transport

Qualification is paused at the owner’s request. Generic, language-case and
reference-port checks are complete. Final library/CLI checks and completed
paired compilation measurements are deferred; see the [handoff](../../../docs/migration/handoff.md).

The [acceptance contract](../../../docs/migration/s1-acceptance.md) covers
bounded primitive sets and paths, exact scalar execution, source forwarding,
default transport and dead work. The final qualification candidate is pinned
in [s1-complete-4-identity.json](s1-complete-4-identity.json); its source digest
includes every Rust source plus Cargo manifests. C2's immutable compiler is
the matched previous implementation; C3 changed no production behavior.

## Implementation and controls

One value lattice transports unreachable/exact/up-to-four-values/unknown
through private cells, complete call sets, normal results and branch paths.
Overflow becomes unknown. Pointwise evaluation has a 64-combination limit;
concrete execution retains the depth, operation, step, work and result limits.
The evaluator classifies unsupported intrinsics by host, engine-dependent,
aggregate, observable, unsupported or exhausted-limit reasons. Declared `pure`
alone never permits execution or removal.

Source rules forward immutable aliases and reaching scalar values without
moving their evaluations. Reuse preserves allocation owners and call/branch
boundaries: new snapshots can grow output, and an old load cannot replace a
later path's range observation without transporting that path fact too.
Common primitive computations reuse an already shared result; introducing new
storage remains a judged representation choice. Raw producer classes guard
coercion and getter behavior independently of checked source annotations.

Literal defaults stay explicit in the shared operations. Private call
metadata permits JavaScript to omit matching trailing literals; semantic
verification recomputes the proof. JavaScript default syntax separately obeys
arity, activation, strictness and parameter-layout requirements. Native keeps
the semantic arguments. The three old target default passes are deleted.
Folded nullable/union literals use native tagged conversions and ownership
operations rather than writing a primitive into an `ls_value` slot.

Shared numeric facts now publish conditional and short-circuit results after
their child regions. Ordinary formation consumes those facts; the explicit
`int32_hints` spelling alternative keeps its separate local tier.

`constant-folding` owns these transforms and target default transport;
`dead-code-elimination` owns newly removable stores, operations, signatures
and units; `inlining` stays separate. Replaceable host methods retain the
pristine-builtin requirement. There are no method-specific flags or hidden
output switches. Receipts distinguish actual removals/folds from repeated
evaluator attempts and refusals. The [configuration reference](../../../docs/configuration.md)
describes bounds, permission interactions and tradeoffs.

## Dependency correction and failed development evidence

The first [generic run](ratchet-initial.json) passed all behavior oracles but
grew every corpus/objective. Forwarding allocation owners and mutable loads
introduced extra snapshots; unrestricted expression sharing introduced new
temporaries. These changes were corrected. The initial
[case run](cases-initial.json) exposed the nullable native boxing defect,
which is repaired and covered through the public service in both targets.

Deleting target alias/root-literal cleanup was also premature. Inlined
parameters/results and scalarized fields are created after shared rules. The
rectangle example creates literal width/height bindings; an inlined array
producer creates a result alias. Those physical bindings have no source IR
identity for a source pass to rewrite. Their existing target normalizers and
veto remain until Q1 supplies representation storage contracts and S4 replaces
the proofs. The acceptance and ownership map record this actual dependency;
the eventual deletion is still required.

The [intermediate transport run](ratchet-transport.json) repaired the case and
application totals but still grew algorithms by 18 raw, 9 gzip and 12 Brotli
bytes. Its baseline update was refused. Preserving branch/call load sites
recovered all of those losses: the [algorithm check](algorithms-path.json)
matches the prior sizes under every objective. Development deltas are retained
in [development-deltas.json](development-deltas.json).

The full case suite additionally found that C2's generated
`differential_pinned_regressions` case was missing from the existing native
Record ledger. The [previous compiler diagnostic](native-record-baseline.log)
reproduces the refusal. It now joins the exact `Record<T>` entry owned by N2
(legacy M11.4); it is neither a new S1 failure nor a passing native program.

## Generic qualification and selected tradeoff

The final [ratchet](ratchet.json) passes all 642 programs in all three
objective lanes. Exact changes against the pre-S1 baseline are:

| Corpus | Raw | Gzip 9 | Brotli 11 |
|---|---:|---:|---:|
| Cases | −593 | −340 | −280 |
| Applications | 0 | 0 | +2 |
| Algorithms | 0 | 0 | 0 |
| Total | **−593** | **−340** | **−278** |

The two-byte application increase is `higher-order-pipeline`: equivalent
bindings receive different short names. A [call-forwarding variant](ratchet-call-forwarding.json)
recovers it but gives back 92 raw, 133 gzip and 46 Brotli bytes overall. The
selected implementation therefore preserves call-result storage and keeps the
better total under **each** objective. The ordinary strict ratchet verdict
remains visible; the baseline was updated with `--accept-growth` and the
explicit, recorded `--accept-corpus-growth` reason. That opt-in requires the
same full input set, every old/new lane passing, nonincreasing totals for all
three objectives and no other blocker. Default acceptance remains stricter.
No noise threshold, blended score or workload-specific compiler rule is used.

The [development delta receipt](development-deltas.json) retains every changed
item and corpus/objective total for the four full development runs. The
selected table agrees byte-for-byte in measured sizes with the prior path
qualification; this final run records the review decision through the runner.

Closure ADVANCED comparisons remain directional S1 evidence:

| Corpus | Raw, ours / Closure | Gzip 9, ours / Closure | Brotli 11, ours / Closure |
|---|---:|---:|---:|
| Applications, 7 | 835 / 1,332 | 727 / 982 | 576 / 834 |
| Algorithms, 11 | 4,920 / 4,522 | 3,125 / 3,031 | 2,844 / 2,703 |

The algorithm gap is still open; these numbers do not claim that S1 or the
compiler beats Closure across the fleet.

## Remaining final qualification

The [language matrix](cases.json) has 5,466 passing lanes, 180 ledgered
failures, 1,608 masked lanes and zero unexpected failures. Marked and PostHog
pass 29/29 and 21/21 in [their corrected run](ports.json); Zod passes 1,353/1,353
in the [initial port run](ports-patch-attempt.json). That initial run stopped
Marked/PostHog before compilation because it tried to reapply landed patches;
only those two were rerun with `--patches none`.

The owner stopped the paired compilation run after two completed cells. The
remaining cost cells, semantic demonstration and final library/CLI runs are
deferred. Nothing is still running. This milestone makes no runtime-speed
claim and no final V1 claim about the external library fleet.
