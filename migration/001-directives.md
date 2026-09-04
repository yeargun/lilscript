# 001 — Standing directives

Parent: [index](index.md). Status: binding on every step of this migration.

These are not aspirations. Each one is a rule a reviewer can hold a commit against, and each
exists because something in this repository already went wrong without it.

---

## D1 — Correctness by construction, not by conditional

> "Make sure that you create a correct system apart from optimizing suboptimal solutions."
> — owner, 2026-09-04

The migration's purpose is not a faster backend. It is a backend in which the wrong-program classes
this compiler has already shipped are **unrepresentable**. Speed is a consequence of the right
representation; it is not the objective, and it never justifies a construction that keeps a bug class
alive.

The test for every design choice: *if someone tries to do the wrong thing, what stops them?*

| Answer | Verdict |
|---|---|
| It does not compile | Accept |
| A total function has no case for it | Accept |
| A build-time check rejects it | Accept with a named owner |
| A runtime assertion catches it | Only where the above are impossible, and say why |
| A conditional guards it | **Reject.** This is what we are migrating away from |

The last row is not theoretical. `emits_ordinary_function_expression`
(`src/codegen_ir_js.rs:7409`) guards arrow spelling against `this`/`arguments` capture with a
conditional — and the conditional is incomplete, so the compiler emits a wrong program today
([002](002-the-instrument.md#live-2)). A `receiver_use` field that makes arrow spelling
structurally impossible for a capturing function cannot be incomplete.

## D2 — One fact, one owner

Every fact in the backend has exactly one owner, and bypassing that owner fails the build.

This is the single diagnosis behind every defect found in the architecture audit: host identity had
two owners (a name table and the user's spelling), operand positions had two (a rewriter and its
fork), option classification had two (a struct and a string table), token structure had 134, scope
had two, and "did this stage run" had none.

A design that adds a second owner for anything — even temporarily, even for one phase — must say so
explicitly, name the reconciling check, and name the commit that deletes it.

## D3 — Compression is a hard constraint, and byte-identity is the only clean proof

26 ports and 181 built artifacts are the non-regression surface ([008](008-fleet.md)). Their Brotli
numbers must not degrade, and neither must their runtime performance.

The measurement law that makes this hard: **a semantically empty change moves Brotli by roughly
−125..+30 bytes**, and the entire 121-fold text layer is worth **189 bytes** on markedlil. An
artifact-level byte diff therefore *cannot* distinguish "the tree emits the same program" from "the
tree silently lost an optimization." Both look like noise.

The consequence is not negotiable:

- A step that intends no byte change must prove **byte-identity**, not byte-similarity.
- A step that intends a byte change must isolate that change to a **single scored decision**, with
  its own registry row, defaulted off, and its own fleet A/B.
- "Within the noise floor" is never an acceptance criterion for a step that claimed to be neutral.

This inherits, and does not weaken, rule 2 of
[planned-migration.md](../docs/knowledge/migration/planned-migration.md#rules-of-execution):
*"Freeze and retain the current legal incumbent before adding an alternative. If a supposedly broader
search cannot reproduce it, stop and fix that first."*

## D4 — Repair the instrument before trusting it

No migration step may be gated on a check that can report a false green. As of 2026-09-04 most of
the gates this migration would naturally rely on are fiction — 18 of 25 port suites never rebuild,
the release gate runs none of them, `debug_assert` is compiled out of every fleet build, and two of
the seven ports that do rebuild test a configuration that cannot see the defect class. The evidence
and the repair are [002](002-the-instrument.md), and **Phase 0 is not optional**.

Corollary: a gate that passes because work was skipped is a failure, not a pass. A port build under
10 seconds, a build with no `lilscript-timing` line, or a port that did not recompile fails the run.

## D5 — Compiles run on the pool, not on this host

This is already law. [`finer/objective.md` §9](../finer/objective.md):

> "Compiles are the loop's clock, and the loop runs on a pool of Azure machines, not on one host.
> Fleet builds, level curves, config sweeps and A/B measurements are dispatched across the pool, one
> port or one variant per machine, **and a tool that serializes them on the orchestrator's host is
> the defect, not the workload.** A hypothesis is scoped so its builds can run side by side. […] the
> wall clock of the *loop* is a cost the owner pays, and it is bought down with machines before
> patience."

Made binding on this migration:

1. **Every phase gate is a fleet sweep.** The matrix is 61 config files across 26 ports and seven
   optimization levels ([008](008-fleet.md)) — it does not fit on one machine and must never be run
   on one.
2. **The orchestrator host does not compile.** This machine is a burstable `B8als_v2`; its credits
   drain after roughly 30 minutes of sustained compilation and throughput then halves. During this
   session cnlil — 1,591 source lines — exceeded a two-minute timeout on a drained host. Numbers
   taken here are triage, never evidence.
3. **Both arms of every A/B run on the pool, on idle workers, at the same commit**, with per-arm
   worker log directories. An A/B where one arm ran on a loaded machine is not a measurement.
4. **A step that cannot be dispatched in parallel is rescoped**, not run serially. If the tooling
   serializes, fixing the tooling is the work item.
5. Absolute timings are only comparable when taken on an idle pool worker. Cross-machine comparisons
   use counters, per [`objective.md` §8](../finer/objective.md).

## D6 — No glue, and no package knowledge

Inherited verbatim from
[typed-proofs-not-glue.md](../docs/knowledge/decisions/typed-proofs-not-glue.md):

> - No package-name, path, or library-AST matcher.
> - No default-on `pure_getters` equivalent.
> - No trailing-name convention as ownership proof in the target architecture.
> - No post-hoc fold whose only evidence is one port.

This migration **removes an existing violation** rather than merely avoiding new ones: the optimizer
currently rewrites user `extern` declarations by matching their source spelling against 105
hardcoded names ([002](002-the-instrument.md#live-1)). Deleting that table is a deliverable of this
plan, not a side effect.

## D7 — Every phase ships

No long-lived branch. Every phase lands on `main`, green, with the compiler in a shippable state and
the ports building. A phase that cannot be split into shippable commits is not yet designed.

Two forces make this non-negotiable rather than stylistic: other Claude sessions edit this working
tree and rebuild port `dist/` concurrently, and `src/codegen_ir_js.rs` is a 38,325-line file that
attracts constant edits. A three-week branch across that file is a merge disaster with no gate.

Practical consequence: prefer a mechanical no-op diff that changes 63 signatures and cannot conflict
semantically, followed by behaviour changes in separate named commits, over one sweep that does both.

## D8 — Delete rather than port, and prove the deletion

The fold taxonomy ([007](007-fold-disposition.md)) found that of 142 folds, **84 exist only to clean
up after the emitter's own output** and 23 duplicate an IR pass or are dead. Only 35 are genuine
target-level transforms.

So the default disposition of a fold is **deletion**, and the migration's main activity is teaching
the emitter not to produce the shape in the first place. Porting a fold is the exception and needs a
reason.

Inherited from [planned-migration.md](../docs/knowledge/migration/planned-migration.md) Phase 3:
*"Do not migrate a fold merely because it exists."*

The proof obligation runs the other way from the usual: before a fold is deleted, its trigger must
be shown to be **absent** from the emitted artifact across the fleet — `active == 0` on every port,
with the fold still enabled — and the scored candidate count must be unchanged. A fold that shipped
a scored variant may not become a fixed choice by accident.

## D9 — Record cost with size

Inherited from [planned-migration.md](../docs/knowledge/migration/planned-migration.md) rule 8:
*"Record compile time and peak memory with size. More search or more knobs is not an automatic
improvement."*

Every phase reports, per port, from the same run that produced its bytes: `emit_ms`, `emit_calls`,
`lex_calls`, `codec_calls`, wall time, and arena high-water. A phase that is temporarily slower says
so in its own gate rather than being discovered later.

## D10 — Pin citations to symbols, not lines

`src/codegen_ir_js.rs` is edited continuously by concurrent sessions, and line-number citations in
the prior design work had already drifted by the time they were reviewed. Every reference in this
plan resolves by **symbol name**; line numbers are a convenience and are re-resolved at the start of
each phase, not trusted.

---

## What overrides what

1. The language contract and the compilation contract. Bytes never legalize a behaviour change.
2. These directives.
3. [planned-migration.md](../docs/knowledge/migration/planned-migration.md) rules of execution — this
   plan is the execution design for its Phase 3 / `arch-05` and inherits its gates.
4. The phase documents in this folder.

Where this plan and an older document disagree about *what to build*, this plan wins and the older
document is updated in the same commit. Where they disagree about *what is currently true*, source
and tracked fingerprinted reports win over both.
