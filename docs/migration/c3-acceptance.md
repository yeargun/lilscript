# C3 acceptance and implementation batch

C3 is the sole active milestone. Its deliverable is calibration evidence and
the machinery to keep later tuning honest, not a claim of optimal heuristics.

1. Freeze medium generic programs spanning arithmetic, control flow, strings,
   records and different source sizes. Inputs remain unknown at compilation;
   independently computed observations qualify every emitted artifact. Record
   source and emitted sizes so repetition cannot masquerade as a large program.
2. Run each objective at effort 13 with pruning on, audited and off. Retain
   complete explanations, exact canonical bytes, source/config/tool identities,
   stopped work, work counts and CPU samples. Audit must preserve ordinary
   output. Compare terminal misses and final no-prune results separately:
   immediate rejection and loss of a useful search path are different events.
3. Provide family-level leave-one-out evaluation of the frozen pruning policy
   and a training-only estimator. Never infer a universal margin from tiny
   cases or transfer one objective's result to another. Q3 owns changes to
   production estimates, budgets and schedules, informed by this evidence.
4. Seal a separate algorithm-library evaluation set before reading its output
   scores, with an exposure ledger and content locks. Existing inspected ports
   remain development data; previously exposed library candidates are labeled
   accordingly. Policy selection uses training data only. Inspecting an
   evaluation failure promotes that workload to development and requires a
   replacement before another generalization claim.
5. Empty the NO3 source allowlist by auditing every remaining rule: state its
   generic legality, exact choice or bounded compatibility policy, and retain
   the historical provenance outside compiler sources. Inventory thresholds
   and their owners; do not conceal a port-derived prior as a new proof.
6. Version win labels independently of exact byte deltas and historical
   thresholds. Establish a current repeated compile-CPU baseline; the missing
   historical A1 pair stays explicitly unavailable. Batch the tool tests and
   relevant compiler checks, then qualify the frozen corpus once.

Predicted effect: no production output, schedule or runtime change. Measurement
adds diagnostic codec work. Tiny deterministic savings remain real; their
generalization and compilation cost are reported separately.

The archived ritual of fresh-session blind port authorship is replaced by a
content-locked, disjoint evaluation protocol with honest exposure records.
Authored algorithm libraries are held out from parameter selection, not claimed
to be unknown to their author or substitutes for external libraries. V1 still
qualifies maintained and independent library boundaries, including already
exposed candidates; C3 does not claim fleet superiority from synthetic programs.
