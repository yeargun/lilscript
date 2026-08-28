# search-01 / search-02 — when candidate search is trusted

Parent: [ledger](../LEDGER.md). `search-01` landed (markdown 15/16).
`search-02` todo (ident-05 landed).

## Question

What has to be true before search is trusted on every lane, and what does it
cost when it is?

## Current state

- Markdown stack: `candidate_search = "always"` on 15 of 16 committed ports
  (search-01). `react-markdown` committed toml stays `off`; compiling with
  `always` is 93/93 after ident-05.
- `benchmarks/popular/monaco-layers/lilscript.toml` — `candidate_search = "off"`.
- Root `lilscript.toml` — `candidate_search = "production"`.
- `comparison/cases/configs/*.toml` — `candidate_search = "always"`, limit 1536.

## Constraints specific to this task

- ident-05 is landed; flipping committed `always` is this task, not ident-02.
- Do not treat a smaller throwing artifact as a win.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-19 | Config state | `grep -n candidate_search lilscript.toml benchmarks/popular/monaco-layers/lilscript.toml` | `production` / `off` | diag |
| 2026-08-27 | Markdown stack search-01 | per-port compile + suite | 15/16 green with `always`; react-markdown off | gate |
| 2026-08-28 | ident-05 marked + RM always | marked reserve 0/8/12/48 vs official; RM `npm test` | 660/660 each; RM 93/93 | gate |

## Log

- 2026-08-19 — Recorded the monaco off-switch. — **OPEN**
- 2026-08-27 — search-01 landed on the markdown stack (md-01). — **LANDED**
- 2026-08-28 — search-02 waits on ident-05, not ident-02. — **OPEN**
- 2026-08-28 — ident-05 landed. search-02 may flip react-markdown and re-run
  corpora. Keep committed port tomls until ident-02 rematerialization sites are
  on the shared check. — **OPEN**

## Next step

Do not flip committed `candidate_search` while ident-02 still has unrouted
rematerialization sites. When ident-02 lands: set react-markdown to `always`,
re-run markdown and monaco corpora, record Brotli deltas as `gate` rows.
