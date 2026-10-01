# S1 acceptance and implementation batch

S1 implementation is complete; the [current checklist](index.md) owns the next milestone. S1 closes primitive value propagation,
bounded constant execution, parameter/default transport and the dead work those
facts expose. Existing S1 work is input to this milestone, not prior acceptance:
the final source must satisfy this whole contract in one qualification.

## 1. Semantic facts

1. Keep one bounded value lattice for the program rules. In addition to
   unreachable, one exact primitive and unknown, it carries a deterministic
   finite set of at most four primitive values. Values, complete-call-set
   formals, normal results and eligible settled cells use the same join. Set
   overflow becomes unknown; it never truncates values. Operations run through
   the language's exact primitive evaluator pointwise, and a common result may
   become exact even when its inputs are a set.
2. Make branch facts path-sensitive where the region solver can prove them.
   A condition may refine a finite set or numeric range in the selected region;
   loop joins widen and terminate. Unsupported paths remain unknown. Keep exact
   values, finite sets, number ranges and primitive runtime classes distinct:
   none may stand in for another.
3. Publish primitive class and numeric range facts from the checked program for
   values, complete-call-set formals and results. Formation consumes these
   facts instead of re-deriving equivalent source-level number or binding facts.
   A classic application script uses its approved private-root contract;
   explicit open/host boundaries remain unknown.
4. Keep the checked `pure` diagnostic. Declared purity alone never authorizes
   compile-time execution: a folded call must finish inside the evaluator's
   depth, operation, step, work and retained-result limits on the concrete
   arguments. Exceptions, divergence, suspension, mutable captures, host calls
   and unsupported operations retain the call.

## 2. Folds, forwarding and removal

1. Inventory every catalog intrinsic that can reach a constant call. Implement
   exact, engine-independent primitive results, including UTF-16 and signed-zero
   edges, or record a code-adjacent refusal category: replaceable host behavior,
   engine-dependent transcendental/Unicode behavior, mutation, allocation or
   an aggregate result outside this milestone. Arity and the pristine-builtin
   contract apply before evaluation.
2. Bounded execution supports structured branches, short circuits, loops,
   local mutation, nested supported calls and early exits while preserving
   argument order. Exact string sums and deterministic known methods fold only
   when the replacement obeys the existing no-growth structural guard.
3. Forward settled scalar constants, immutable aliases and reaching scalar
   definitions in the program IR, subject to initialization, TDZ, capture,
   reference, export and root-boundary proofs. Remove algebraic identities and
   repeated primitive computations only when evaluation behavior and dominance
   make the replacement exact and the shared structural result does not grow.
4. Remove unread values, discardable initializers, dead stores, dead cells,
   unreachable operations and units no created value can reach. Effects,
   throws, resource exhaustion, divergence, re-entry, suspension and required
   argument evaluation remain in source order.

## 3. Parameters, defaults and returns

1. On complete direct call sets, transport unread and constant parameters and
   unused results through signatures and every call. Preserve coupling classes,
   reference parameters, `arguments`, rest, generic instantiations, exports,
   host-visible callable length and argument effects.
2. Represent callee-built defaults in the program operation/signature contract.
   Drop a trailing `undefined` or exact literal argument only when the callee
   applies the same default and no observable arity/`arguments` use exists.
   Record native-default length at the program-to-target boundary. Typed calls
   that always supply a defined value remove their redundant default test.
3. Delete the target-tree implementations
   `drop_typed_default_checks`, `drop_default_arguments` and
   `native_default_lengths` after equivalent program evidence passes. Delete
   source aliases through shared IR forwarding. Target alias/literal cleanup
   remains for storage introduced by later representation choices, under the
   corrected dependency below. Scalar source-root forwarding moves to program
   facts; objective-sensitive string constants remain a named family until Q1
   generalizes the choice interface.

## 4. Corrected ownership boundaries

Three deletion assignments conflict with real dependencies and are
split rather than silently waived:

- Aggregate array/object store collection needs escape, uniqueness and field
  facts. S2 owns that part of legacy M7.8 with its scalar replacement work; S1
  covers primitive values and immutable aggregate observations that require no
  identity or field proof.
- `raw_domains.rs` proves target-recipe-dependent primitive values for selected
  helper and record families under counted, demand-scoped budgets. Replacing it
  before S2 and Q1 would either discard those proofs or put target choices into
  a supposedly target-neutral fact. S1 supplies the shared program primitive
  facts and removes equivalent formation derivations. S4 deletes or absorbs the
  remaining recipe-dependent domain and target binding-class derivations after
  their S2/Q1 consumers use the common contracts.
- The first matched S1 qualification demonstrated that `eliminate_aliases`
  and scalar `forward_root_constants` also consume bindings created **after**
  shared rules: inlined parameters/results and scalarized object fields. For
  example, the rectangle layout introduces separate literal width/height
  bindings; an inlined array producer introduces a result alias. Source facts
  cannot name those physical bindings. Keep these target normalizers, with
  their existing `constant-folding` veto, until Q1 owns representation-created
  storage and S4 can remove their duplicate proof. S1 implements source-level
  forwarding; it does not delete useful downstream cleanup merely to satisfy
  an incorrectly ordered deletion. This is a measured dependency correction,
  not a waiver of the eventual deletion.

These changes preserve the requested work. They place each deletion after its
last real consumer, matching the plan's S4 fact-consumer cleanup dependency.

## 5. Configuration and receipts

- `optimization.constant_folding` controls value/set propagation that changes
  operations, known-method folds, bounded call execution and constant-parameter
  replacement. `optimization.dead_code_elimination` controls removal and
  signature shrinking; `optimization.inlining` remains independent.
- `javascript.assume_pristine_builtins` permits folds whose emitted JavaScript
  reaches replaceable host methods. Checked numeric `toInt()` and other direct
  language operations do not require that assumption. No hidden environment
  switch changes output.
- The existing broad tactics are the configuration surface; there is no flag
  per method or peephole. Their documentation states the situation, legality,
  size opportunity, compile-work bound and runtime effect. `preset = "none"`
  and explicit vetoes must disable every corresponding direct, searched and
  replayed result.
- Receipts separately count exact/set/path folds, evaluated/refused calls,
  forwarded definitions, common computations, default arguments/checks,
  native defaults and each kind of removed work. Limits and refusals are
  observable without changing output.

## 6. Completion evidence

1. Independent Node oracles cover every newly supported ECMAScript primitive,
   including binary64 bit patterns, UTF-16 lone surrogates, signed zero, NaN,
   infinities, indices and exception boundaries. Portable folds also execute in
   native C. Negative cases cover effects, TDZ, captures, references, arity,
   host replacement, recursion/divergence and every resource limit.
2. Focused generic programs demonstrate calls, branches, bindings, defaults,
   stores and dead units leaving. Compile with each relevant tactic on and off,
   and with pristine builtins both ways. The full library and case suites pass.
3. Run the frozen generic ratchet independently for raw, gzip and Brotli. Report
   exact aggregate and individual deltas, without a blended verdict or a noise
   threshold. Run the reference ports and preserve their behavior suites.
4. Compare the affected application/algorithm programs with eligible Closure
   ADVANCED artifacts under C2's contracts. This is a directional S1 result,
   not the final V1 fleet claim. Retain objective-specific bytes, compiler work
   and alternating CPU samples. Measure runtime only where a fold can change
   asymptotic work or allocation; fewer constant operations alone needs no
   speed claim.

S1 completes only when the implementation, deletions, controls, receipts and
evidence above land together and the ownership map is updated. Then S2 alone
becomes active.

## Owner-directed completion disposition

The owner requested the S1 checkpoint and push, followed by complete S2
implementation with systematic focused checks, without another full library
rerun. The implementation contract above is satisfied with the explicit
ownership corrections. Generic, language-case and reference-port evidence is
retained in the [S1 report](../../benchmarks/migration-results/2026-10-01-s1-complete/README.md).
The final library/CLI reruns and complete paired compilation study are deferred,
not passing results. This supersedes the earlier requirement to finish those
reruns before proceeding to S2; it does not weaken semantic correctness or
independent objective accounting.
