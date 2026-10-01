# G1 lexical naming completion contract

G1 is complete after Q1 (`cf14d2db`). Its [evidence](../../benchmarks/migration-results/2026-10-01-g1-complete/README.md) records the final implementation, negative size result and effort-14 gate. G2 follows.
The existing allocator already has scope reuse, capture checks, local/root read
frequency, source-name alternatives and exact-judged joint alphabets. Finish
its remaining allocation and delivered-byte behavior without weakening those
constraints or restarting broad fleet qualification.

## Implementation batch

1. [x] Consume the verifier's live-binding mask in a compact allocation
   alternative, so removed bindings no longer use short names or reserve
   irrelevant spellings. Derive a printed declaration/use order and preserve
   independent local/root frequency choices and scope reuse. Keep the existing
   allocation as an eligible prior, judged on complete bytes.
2. [x] Use the full legal continuation alphabet for compact names (digits only
   after the first character). Keep keywords, host names, pinned names, direct
   eval, exact function names, imports/exports and capture interference correct.
3. [x] Register the allocation choice with the common family contract and
   existing naming-search/identifier permissions plus the independently vetoable
   naming-compaction strategy (auto at 14; explicit on at 13). Try it after the completed
   ordinary search, protect that result, and retain its provenance for replay.
   Keep the alphabet switch independent and document behavior/tradeoffs.
4. [x] Settle equal exact objective costs using complete rendered raw bytes
   before arbitrary recipe/naming-plan identity. Confirm the selected artifact
   is the delivered artifact, with no unscored rename after selection.
5. [x] Cover dead binding holes, >54 bindings, identifier boundaries, captures,
   pins/reflection/eval and unrelated-scope stability in one focused batch.
   Retain independent raw/gzip/Brotli comparisons on affected generic workloads,
   including the carried captured-activation naming case. Broad held-out and
   competitor qualification remain V1.
6. [x] Update coverage, history, index and handoff; commit/push before G2.

Expected effect: fewer wasted identifier slots and useful order/alphabet
alternatives, at the cost of bounded naming/codec work. Exact compressed benefit
is empirical; no fixed alphabet or raw-size estimate is a compression guarantee.
