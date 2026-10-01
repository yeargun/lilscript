# Q2 reuse, scheduling and resource contract

Q2 is the sole active milestone after S4 (`c85ad5d2`). Complete it before Q3.
Existing score memoization, effect/view caches, retained target heads and stable
scheduler suffixes remain useful starting points; do not rebuild their owners.

1. [x] Replace the target's private repeated function-pruning walk with an
   admitted dependency/use index. Close unused declaration chains and cycles
   from actual roots, preserving effects, exports, pinned bindings, captures,
   initialization and strictness. Remove the old four-iteration traversal.
2. [~] Drive remaining source/target normalization with dirty dependencies,
   retaining deterministic structural edit order and fail-closed convergence.
   Reuse stable facts only for their immutable dependencies; edited bodies and
   changed interfaces invalidate their consumers.
3. [ ] Complete incremental/shared formation and bounded persistent reuse.
   Repeated assignments and unchanged delivered files must not repeat physical
   work. Include the content-addressed build/elaboration cache and the explicit
   fingerprinted decision-lock replay from M3.9. Keys include complete
   source/config/compiler/encoder identities;
   cached evidence grants neither a permission nor extra search work.
4. [ ] Finish resource admission for graph/cycle/delivery and analysis/edit
   storage, including failure cleanup. Entry graph, cycle checking and the
   target use index, edit journals, local dirty maps, renumbering plans and complete retained target copies are admitted; delivery placement, simulation and setter payloads are now
   admitted. Source arena edits are now admitted; rendered bundle storage is admitted. Global analysis/planning owners still need completion. Count logical work independently from
   physical cache hits and retain scaling/work/byte telemetry.
5. [ ] Wire/document cache and scheduling controls with hard TOML behavior,
   lifetime/capacity, invalidation, defaults and tradeoffs. Transparent controls
   may change physical cost but not chosen output or admission decisions under
   the same deterministic logical limits. Q3 owns search breadth and heuristics.
6. [ ] After coherent batches, compare dense/cold and reused behavior under all
   objectives, limits and relevant delivery forms. Use affected oracles and
   pinned measurements; reserve full-fleet/held-out qualification for V1.
7. [ ] Remove replaced paths, retain actual results and negative evidence,
   update coverage/checklist/history/handoff, then commit/push before Q3.

First implementation batch: the target use-index consumer and graph resource
admission. Prediction: fewer repeated liveness traversals, complete dead-cycle
removal, unchanged retained behavior and occasionally smaller output. The index
uses bounded admitted storage/work. Qualification covers roots, recursive
cycles, captures, effectful initializers, admission cleanup and affected
raw/gzip/Brotli artifacts. No full library rerun.

The first batch is [implemented and qualified](../../benchmarks/migration-results/2026-10-01-q2-use-index/README.md).
It also closes stale print proofs after delivery inserts cross-file setters.
Q2 remains active for tasks 2–7; this is not a milestone completion claim.

The [bounded journal batch](../../benchmarks/migration-results/2026-10-01-q2-journals/README.md)
closes repeated edit-list growth and its success/refusal lifetimes. Remaining
resource owners and dirty scheduling are still open.

The [persistent codec batch](../../benchmarks/migration-results/2026-10-01-q2-codec-cache/README.md)
adds bounded optional disk measurement reuse, strict TOML/CLI controls and
cold/warm/disabled output and logical-admission parity. This completes only the
codec portion of tasks 3 and 5; build/elaboration caches and decision locks
remain open.

The [normalization batch](../../benchmarks/migration-results/2026-10-01-q2-normalization/README.md)
implements local source revision reuse, three target stable-proof consumers and
admitted in-place expression ordering. All-objective on/off logical/byte parity
is qualified. SCC/global dependency invalidation and incremental formation stay
open; measured target reuse is modest and no broad speed gain is claimed.

The [complete target-copy batch](../../benchmarks/migration-results/2026-10-01-q2-target-copies/README.md)
admits nested payloads and behavior-proof lifetimes. Affected artifacts remain
byte-identical with higher, correctly counted copy work. This closes the retained
copy owner, not initial placement or source analysis/edit admission.

The [effect-component batch](../../benchmarks/migration-results/2026-10-01-q2-effect-components/README.md)
implements exact SCC invalidation and dirty recursive effect propagation, plus
structural/scheduled summary reuse. Cold/reused all-objective parity passes;
an existing conditional-choice convergence defect is repaired. Other global
rule dependencies, analysis storage, formation and persistent build/lock work
stay open.

The [whole-build cache batch](../../benchmarks/migration-results/2026-10-01-q2-build-cache/README.md)
implements bounded, content-addressed completed build reuse and strict TOML/CLI
controls. Fresh graph/host identities and full config/compiler/limits qualify
hits; all-objective logical/output parity passes. This closes whole-build
persistence, not the per-module elaboration cache, decision lock or incremental
formation requirements.

Next batch: explicit decision locks. Save separate objective assignments with
complete input/config/compiler identities, reconstruct structural recipes through
current proof producers, and judge complete delivered bytes through the ordinary
admission path. Replay is the first optional terminal move, within that walk's
existing limits; the mandatory baseline survives every refusal. Prediction:
reproducible useful starts, sometimes fewer subsequent moves, with additional
proof/formation cost when the saved assignment does not help. No speed or size
win is presumed. TOML and CLI controls are explicit; malformed or stale files
produce a reported miss, never evidence or permission. Requested writes fail
visibly if they cannot be completed.

This corrects the old M3.9 proposal to reuse a compression verdict from a matching
site/unit hash. Compression, naming and incumbent comparisons depend on complete
delivered bytes. An unchanged local site cannot certify an unchanged verdict;
only the existing exact-byte, encoder-qualified measurement cache can reuse that
cost. Decision locks therefore require complete identities and fresh legality
and artifact admission, and do not promise reuse across source edits.

The [decision-lock batch](../../benchmarks/migration-results/2026-10-01-q2-decisions/README.md)
implements that explicit full-identity replay and its TOML/CLI controls. Current
proofs and exact artifact admission protect each independent objective; ten
focused checks and 36 release oracles pass. Bytes are unchanged on the controls,
and search cost varies in both directions. This completes the lock portion of
tasks 3 and 5, not per-module elaboration or incremental formation.

Next batch: complete the delivery planner's allocation owner. Admit reference
walks/facts, atom/label propagation, placement and trial copies, link tables,
simulation, setter payloads and final plan metadata before allocating. Reuse
the existing budget; temporary storage must release on success, refusal and
unwind without borrowing the target's retained storage. Remove redundant
context clones and keep deterministic placement/merge order. Prediction:
unchanged sufficiently funded artifacts, higher correctly counted work/bytes,
and earlier safe refusal at genuinely insufficient limits. Qualify the complete
owner with focused single/split/preserved, lazy/cycle/setter controls and refusal
cleanup after the implementation batch; no full library/fleet rerun.

The [delivery batch](../../benchmarks/migration-results/2026-10-01-q2-delivery/README.md)
completes that planner owner. Sixteen focused checks and 36 release checks pass,
with identical files, scores and judgment counts across independent objectives.
Previously uncounted work/storage is now charged; no speed or size gain is
claimed. Temporary input and rejected trial lifetimes are qualified, including
refusal and unwind. Source analysis/edit and later artifact/render allocation
owners remain open, alongside shared/incremental formation and elaboration.

Next batch: source rule edits and their arena owner. Admit touched/remap storage,
shared-body/table copies, new regions/operations/calls and nested payloads before
allocation. Reuse the construction ledger and remove the post-edit net-size
reservation as an admission mechanism. Preserve source revisions, cell remaps,
copy-on-write snapshots and failure cleanup. Replace unnecessary compaction
copies where arenas can be moved safely. Prediction: unchanged meaning and
funded artifacts, correctly higher logical bills where edits previously escaped
accounting, fewer physical copies for compacted bodies. Qualify substantial
kernel/producer changes together with focused source-rule, native/JavaScript
behavior and refusal checks; no full library or fleet rerun. Global fact owner
migration remains a separate outstanding part of Q2.

The [source edit batch](../../benchmarks/migration-results/2026-10-01-q2-source-edits/README.md)
completes admission for retained source mutations, compaction and shared copies.
104 focused checks and 36 paired release checks pass; all control bytes and
judgment counts match. An existing namespace call-convention defect is repaired.
Retaining arena capacity reduces copying but raises measured peaks modestly;
newly counted work is explicit. Global analysis/planning, artifact/render,
shared formation and elaboration remain open.

Next batch: rendered delivery and artifact metadata. Admit final/provisional
filenames, content-hash walks, preload/specifier scratch, delivered layouts,
artifact file vectors and objective rows through their existing owners. Release
provisional print buffers before final printing and transfer one complete
artifact reservation; a refusal must release every completed file and layout.
Avoid temporary string copies in the printer and stream number spelling where
possible. Include carried-host output and preserve exact naming/hash/container
behavior. Prediction: unchanged funded files and scores, more accurate work
and memory bills, less temporary copying; focused rendering, delivery, hash,
refusal and exact-codec checks follow one coherent implementation batch.

The [rendered bundle batch](../../benchmarks/migration-results/2026-10-01-q2-render/README.md)
completes admission for delivered render buffers, hash/name/specifier/preload
scratch, host wrappers, artifact file metadata and objective rows. Twenty-six
focused checks and 54 release checks pass; all files, scores and search counts
match. Bills rise for newly admitted work/storage. Global analysis/planning,
shared/incremental formation and per-module elaboration remain open.

Next batch: share completed family tails across naming trials and repeated
assignments. Keep exact formation keys inside the candidate owner, bound retained
tails, and replay the ordered cold admissions on a hit. Record bounded traces
through the existing ledger; overflow falls back to physical execution. The
TOML reuse veto executes the cold path with the same reservation schedule. Fresh
permission and output checks remain required. Prediction: identical sufficiently
funded artifacts and on/off logical limits, fewer physical tail executions, at
the cost of bounded cache/trace storage. Qualify admission-prefix refusals,
unwind cleanup, controls and all-objective cold/reused output after the coherent
implementation batch; measure repeated formation before claiming a speed gain.


**Formation admission design correction.** The ordered-event trace proposal
above is rejected. Three markedlil probes still performed all 1,451 formations,
with the latter two overflowing 1,441 traces and taking 70.09/70.68 CPU seconds
versus the preceding compiler's 56.12-second sample. Single-event and repeated-
sequence encodings added complexity and bounded-storage failures. Preserve
this negative evidence; do not describe the trace as a completed optimization.

Instead, the candidate owner deterministically retains two completed assignments
in both physical reuse modes. A repeated assignment admits its previously
measured complete work and peak storage as one stage before either borrowing
its result or physically executing the same fully funded builder. The stage's
logical refusal is independent of physical occupancy; its prefix is deliberately
versioned rather than reproducing millions of individual ledger events. A
recovered resource refusal disqualifies capture. This directly serves Q2's
product contract—bounded storage, unchanged cache-on/off selection and hard
limits—without making an event log part of the compiler's hot path. Qualify
complete-stage refusal, physical-execution gating, unwind and all-objective
transparency, then measure markedlil again before claiming a speed gain.


Next analysis-owner batch: admit call/storage graphs, iterative alias resolution,
recursive components and initialization/dominance/scheduling storage before
allocation. Carry their ownership through source normalization and target demand,
including invalidation, retained prior effects and failure cleanup. Replace
allocation-heavy local maps with sorted admitted occurrence tables where their
queries permit it. Then complete the effect-summary, class/range/aggregate/value
and rule-planning owners through that same interface. Cache vetoes must preserve
logical admission; do not instrument only the physically executed reuse branch.
Prediction: funded output stays unchanged, the bill includes previously omitted
storage/work, and long alias chains no longer consume recursive Rust stack.
Qualify complete producers/consumers in coherent groups; no broad library rerun.


The [shared formation batch](../../benchmarks/migration-results/2026-10-01-q2-formation/README.md)
qualifies that stage design: eleven core checks and 81 release checks pass;
all control files and independent-objective scores match the preceding compiler.
Markedlil avoids 22 of 1,451 formations, but its paired CPU medians are unchanged.
The event-log design and its negative results are retained as rejected evidence.
This closes repeated complete assignments, not changed-unit formation,
per-module elaboration or the outstanding analysis/planning owners.

The connected analysis batch also removes discarded per-operation flow history
from range propagation. Its solver admits exact live state copies, and demand
owns its effects and ranges until formation finishes. Declared-pure checking
uses a temporary admitted owner and leaves no inspection cache in publication.
Nested complete-stage receipts preserve deterministic effect SCC/member
admission when physical reuse is disabled. This is policy algorithm 55; limits
now count previously omitted graph/summary/range work and backing. Class,
aggregate, exact-value and rule-planning owners remain the next part of Q2.

The [connected analysis batch](../../benchmarks/migration-results/2026-10-01-q2-analysis/README.md)
qualifies those graph/effect/range paths with 63 distinct focused checks, 54
release comparisons and one markedlil raw/13 probe. Funded files and search
choices remain unchanged; all tested cache vetoes preserve logical admission.
The larger artifact stays 31,882 bytes with 354 judgments; one 55.24-second CPU
sample is not a speed claim. Remaining class/aggregate/exact-value, rule-planning,
default-verification and local-fact initialization owners stay open.
