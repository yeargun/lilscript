# search-01 — when candidate search comes back on

Parent: [ledger](../LEDGER.md). Status: blocked(ident-02). Covers search-01..02.

## Question

What has to be true before search is trusted again, and what does it cost when it is?

## Current state

- `benchmarks/popular/monaco-layers/lilscript.toml:8` — `candidate_search = "off"`.
- Root `lilscript.toml:15` — `candidate_search = "production"`.
- `comparison/cases/configs/*.toml` — `candidate_search = "always"` with a 1536 limit,
  per [the case runner](../../../../../comparison/cases/README.md).

## Why it is off

Search ranks whole artifacts under the configured codec. If the emitted program can be
semantically wrong, search is choosing among wrong programs and its ranking is noise —
worse, it is *convincing* noise, because the winner is genuinely smaller.

## The condition to flip

Both `ident-02` (the invariant is a class, every rematerialization site routes through
one check) and `marked-02` (660/660 recorded) are `landed`. Not one of them.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-19 | Config state | `grep -n candidate_search lilscript.toml benchmarks/popular/monaco-layers/lilscript.toml` | `production` / `off` | diag |

## Log

- 2026-08-19 — Recorded the off-switch and its exit condition so a later context does
  not flip it back on for a quick win. — **OPEN**

## Next step

Nothing until the blockers land. When they do: flip the monaco lane, re-run the
corpora, and record per-corpus deltas — including cases where search costs raw bytes
and wins compressed ones, since those are the results that justify the search at all.
