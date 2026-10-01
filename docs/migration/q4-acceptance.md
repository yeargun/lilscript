# Q4 const data and owned helpers

Q4 is complete after Q3 (`12200f8e`) within the supported boundary below.
[Evidence](../../benchmarks/migration-results/2026-10-01-q4-data/README.md).

1. [x] Implement checked `const` data and functions, mandatory bounded exact
   evaluation, deep mutation/escape checks and supported public boundaries.
   Runtime-dependent/host/engine-dependent evaluation fails with a source span.
   Limits come from TOML, independently of optimization permissions and effort.
2. [x] Carry immutable data provenance through shared IR and target formation;
   support bounded specialization on static schemas with current exactness,
   identity, initialization and evaluation-order rules. Keep unsafe or exhausted
   optional specialization at runtime; required const evaluation diagnoses it.
3. [x] Give runtime helpers explicit ownership and demand/placement dependencies.
   Remove the first-module decoder shortcut. Share compatible helpers or
   specialize through judged alternatives; count all emitted helper bytes.
4. [x] Finish data representations and pooling at final formation/naming,
   including a lazy alternative only with computed-key-only proof and explicit
   runtime permissions. Keep literal and eager alternatives available.
5. [x] Document controls/legality and runtime tradeoffs, then run one coherent
   focused batch with independent data/mutation/alias oracles and actual raw,
   gzip and Brotli artifact costs. Qualify startup, memory and hot access on
   affected generic workloads. Update evidence/checklist, commit and push.

The target is smaller correct delivered programs under bounded work. Re-parsing
compiler helper source is a mechanism, not a semantic objective: reuse typed
helper construction where it gives one checked owner, dependency closure and
ordinary optimization/placement. Any change from the archived mechanism must
be recorded with its exact supported scope and evidence.

## Supported scope and mechanism decisions

Required evaluation handles the implemented exact scalar/string operations,
private aggregate construction, bounded calls, branches and loops. It diagnoses
unsupported operations, host dependencies, cycles and exhausted bounds. Readonly
aliases flow through private calls and returns; unproved escapes, callbacks and
mutation are refused. This is deliberately a finite evaluator, not a second
unrestricted runtime. Native receives the same evaluated IR; static storage and
its public const ABI remain N2.

Public scalar/array/record const graphs retain exact keys, reference sharing and
deep freezing. The final interface audit found that stored value-struct graphs
need a distinct public view and a memoized reference conversion. Those crossings
now fail with a source span, before search. D2 owns their graph-preserving public
adapter alongside its existing public boundary work; private const products and
callable S4 value codecs remain supported.

Uniform static schema arguments to closed private functions forward through the
existing parameter and aggregate rules, under constant folding/dead code controls.
Mixed schemas retain runtime code or the existing bounded per-call alternatives.
No unrestricted compiler or clone expansion is claimed.

M8.5's source-prelude mechanism is replaced by typed target IR construction:
helpers have an actual module owner, normal dependency/demand/placement, final
naming and complete artifact costs. Compatible decoders share within that owner.
M9.8 retains sound eager encodings of exact mutable literals; requiring a const
keyword there would discard valid compression choices. Lazy whole-table caching
requires checked immutable provenance, computed-key-only uses and both startup
and recurring reconstruction permissions. It is off by default at every effort.
Whole-value/public/alias uses keep eager or literal representations. Per-key
partial decoding is not implemented or claimed.

Canonical enum scalars already participate in ordinary scalar spelling, pooling
and exact table choices. Arbitrary enum renumbering is not a required mechanism:
it would need a complete identity/domain proof and demonstrated size opportunity;
no such new representation or measured win is claimed. Existing exact-content
codec memoization reuses scores without merging source data identities.
