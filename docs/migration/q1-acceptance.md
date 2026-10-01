# Q1 representation interface contract

Q1 is complete after S3 (`4d6e457f`). G1 follows this package. Keep existing useful alternatives and conservative legality
proofs; this is a migration of the existing compiler, not a new search engine.

## Implementation batch

1. [x] Move representation site/alternative identities, immutable assignments
   and move scheduling into one target-independent interface. Register source
   layouts, inline transport, strings, target call representations and data
   encoding with their owning permissions, risk and invalidation.
2. [x] Make structural discovery/proof dispatch consume that family contract.
   Keep typed proof payloads and their compatibility checks; common metadata
   must not replace legality with a flag. Preserve existing opportunity order,
   product-component equivalence and structural combinations.
3. [x] Migrate S3's expression duplication, constant signatures and helper
   sharing from whole-artifact decisions to stable per-binding choices. Keep
   whole-family defaults as useful joint moves and offer the retained form.
4. [x] Replace the data-only site scheduler with the shared move scheduler:
   single-site moves, family rollback and bounded joint assignments. Offer
   legal alternatives even when a raw estimate is nonpositive; exact configured
   final bytes decide. Retain deterministic work accounting and incumbents.
5. [x] Admit explicit site assignments through the same permission/provenance
   path as defaults and reused artifacts. Reject unsupported alternative IDs,
   source-recipe keys in target maps and missing runtime permission. No separate
   publication path or unscored post-selection edit.

## Evidence and completion

- [x] One focused batch: site identity, independent selection of multiple
  helpers, coupled moves, structural opportunity preservation, configuration
  veto/replay and existing data alternatives. Use independent runtime oracles.
- [x] Measure affected generic programs for raw/gzip/Brotli; retain exact output
  identities and actual work counts. No full-library/fleet rerun here.
- [x] Remove superseded definitions and data-only scheduling; update coverage,
  history, checklist and handoff, then commit/push before G1.

Expected effect: more useful mixed representations and joint choices, with extra
bounded discovery/trials. Existing structural/frontier and terminal phases may
retain different work queues because proofs and formed sites exist at different
stages; both must consume the same family/assignment contract and artifact
admission. Q2 owns dirty formation and cache/resource completion, Q3 owns policy
calibration, and G3/Q4 add further spelling/data families through this interface.

The completed interface lives in `src/representation.rs`. Structural publishers
and discovery use its family metadata; proof payloads retain their typed owners.
The former JS-only choice definitions and data-only scheduler are removed. One
private-call representation rule replaces three whole-artifact rules; defaults
remain joint alternatives and explicit sites can override them independently.

Additional family/pair moves run after the ordinary completed walk, under
`representation-joints` (automatic at 14; explicit on/off). Joint assignments
merge once, retain provenance and cannot replay under a veto. Source and target
phases use the existing artifact arena, verifier and exact admission path.
Algorithm 15 / search schedule 30 / walk 9 identify the changed policy.

[Evidence](../../benchmarks/migration-results/2026-10-01-q1-complete/README.md)
contains 48 distinct passing focused checks and 18-program independent objective
comparisons. No full-library/fleet or paired timing claim is made. Dirty reuse,
full resource accounting and policy calibration keep their Q2/Q3 ownership.
