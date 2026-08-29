# Superseded phase 07 plan

Status: historical architecture sequence. Canonical execution plan:
[planned migration](planned-migration.md). Current implementation:
[current architecture](../compilation/current-architecture.md). Live state:
[ledger](board/LEDGER.md).

The former 07.1-07.7 plan produced the compilation contract, decision census,
operation provenance, reversible priors, keep-object alternative, named class
emission, constructor export, expression `if`, scalar `match`, ordinary objects,
owner-scoped properties, closure snapshots, reserved work, and starvation
reporting. Those landed capabilities are summarized in
[`docs/current-status.md`](../../current-status.md).

The unfinished work is no longer organized as “finish the old proposal.” The
current plan starts from measured regressions and legality gaps: make recipes and
evidence replayable, strengthen expected-versus-observed ABI and final-byte
identity checks, recover legal incumbents, consolidate existing decision paths,
then migrate only necessary generated-text folds to a narrow hygienic target-JS
representation. Large new abstractions require a measured failure of the smaller
design.

The detailed historical plan remains available in Git history before the
documentation consolidation on 2026-08-29.

## Size-first library contract

Superseded by [mission](../mission.md#user-intent),
[evidence before claims](../decisions/evidence-before-claims.md), and phase 6 of
the [planned migration](planned-migration.md#phase-6-delete-superseded-paths-and-certify-release).

## Semantic firewall

Superseded by [contracts before objectives](../decisions/contracts-before-objectives.md).

## 07.1 - Identity before search

Landed history; see the [ledger](board/LEDGER.md#ident--javascript-identity-blocks-everything).

<a id="072--one-registry"></a>
## 07.2 - One registry

Landed foundation; remaining consolidation is
[phase 2](planned-migration.md#phase-2-consolidate-contract-decisions-and-evaluation).

<a id="073--reversible-priors"></a>
## 07.3 - Reversible priors

Landed foundation; current behavior is in the
[decision registry](../compilation/decision-registry.md).

<a id="074--ir-emits-legal-shapes"></a>
## 07.4 - IR emits legal shapes

Landed foundation; current behavior is in
[current architecture](../compilation/current-architecture.md#typed-ir-and-proofs).

<a id="075--peephole-is-contraction"></a>
## 07.5 - Peephole is contraction

Active successor:
[phase 3](planned-migration.md#phase-3-introduce-the-minimal-hygienic-target-js-tree).

<a id="076--search-that-can-finish"></a>
## 07.6 - Search that can finish

Reserved work and starvation reporting landed. Replay, scheduling, and measured
interaction work continues in
[phase 5](planned-migration.md#phase-5-improve-bounded-search-only-where-evidence-shows-a-miss).

<a id="077--language-proofs-and-explicit-lowering-contracts"></a>
## 07.7 - Language proofs and explicit lowering contracts

Several slices landed. Remaining reusable proof work is
[phase 4](planned-migration.md#phase-4-close-library-losses-with-reusable-proofs).
