# phase2-02 — unify terminal acceptance

Parent: [ledger](../LEDGER.md). Status: landed.

## Question

Can every terminal challenger use one materialize, validate, exact-score, guard,
and compare operation while retaining the current candidate set and byte order?

## Current hypothesis

`JavaScriptArtifactAdmission`, `AdmittedGeneratedJavaScript`, and the shared
candidate ordering key provide the required pieces. The remaining work is to
replace per-family score-and-accept sequences without changing family generation.

## Constraints specific to this task

No new alternatives, search budget changes, package-specific logic, or ranking
changes. Invalid candidates remain charged and rejected before codec invocation.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-29 | Predecessor consolidation | `cargo check --lib`; focused registry and terminal tests | shared codec admission, ordering, phase/compress recipes, terminal naming, and pooling are green | gate |
| 2026-08-29 | Final challenger acceptance | `cargo test --lib selected_canonical_peephole_uses_only_reserved_terminal_work`; `cargo test --lib search_off_finalization_scores_the_artifact_it_returns` | canonical and search-off challengers share startup guards, artifact admission, exact scoring, and ordering; both focused tests passed | gate |
| 2026-08-29 | Late cleanup acceptance | `cargo test --lib terminal_cleanup_reopens_canonical_peephole_on_unprepared_finalist`; `cargo check --lib` | cleanup beams, local variants, common arms, Boolean finalization, and parenthesization share admitted exact scoring; focused checks passed | gate |

## Log

- 2026-08-29 — Began inventory of duplicate late-cleanup and remap acceptance sequences. — **OPEN**
- 2026-08-29 — Extracted `accept_finalized_javascript_challenger` for canonical and search-off finalization without changing their budgets or selected bytes. — **OPEN**
- 2026-08-29 — Routed late cleanup through `score_terminal_javascript` / `score_reserved_terminal_javascript`; binding-only remaps retain V-02 proof and final admission. — **LANDED**

## Next step

Continue with `phase2-03`: normalize remaining compilation policy inputs before
lower layers consume them.
