# Configuration matrix

Parent: [verification](README.md). Config behavior:
[config](../config/README.md). Search: [candidate search](../compilation/candidate-search.md).

## Required axes

| Axis | Values/boundaries to own |
|---|---|
| `priority` | size, balanced, realistic-performance, performance |
| `cost_model` | raw, gzip, brotli |
| `candidate_search` | off, production, always; CLI development override |
| `optimization_level` | 0, each feature threshold, 15 |
| `optimizations` | omitted, empty, exact single/mixed lists |
| `compression` | omitted, empty, each decision, interaction lists |
| candidate budgets | 1, below/at/above beam and byte boundaries |
| raw growth | 0, small positive, maximum; raw vs codec admission |
| startup/performance | each hard limit and ranking priority |
| `[optimization]` | preset none/maximum; explicit false hard-offs |
| `[mangle]` | identifiers/properties/exports/pooling unset/false/true |
| aggregate/function ABI | named/positional, named layout, arrow/function |
| bundle | single/split/preserve, preload, chunk/deploy-cost limits |
| profile | absent/file/inline override/invalid/stale key |
| native/lint/format | storage/lint/format contracts, no unintended JS drift |

## Current facts to pin

- `production` search has an effective cap of 384 even when `candidate_limit = 1536`;
  `always` can use the full configured limit.
- CLI `--mode development` forces multi-IR/emission candidate search off before
  compilation; exact tests must also pin that configured parsed-peephole finalization
  may still compare its transformed and untouched forms.
- Exact `javascript.optimizations` removes the level-derived feature/cap set, but
  production’s 384 cap still applies.
- Exact `javascript.compression` now gates region-outlining probes as well as the
  canonical optimizer; `[optimization].region_outlining = false` is a hard off.
- `host-alias-spelling` leaves the configured emission shared and admits a direct
  native dotted spelling only for static callee-only uses; detached/exported/bound
  uses must keep their binding under every search budget.
- Closed-script structural probes may carry one helper interaction into the terminal
  frontier: `AllEligible` after a changed outline, or `SingleStaticUse` when ordinary
  IR inlining is off. Test budgets below the late helper-family width so this seed,
  its candidate accounting, and the configured-baseline admission rule stay pinned.
- For gzip/Brotli, raw growth is admitted when transfer does not exceed baseline, or
  when it falls inside `max_candidate_raw_growth_percent`.
- `size-first` ranks exact transfer bytes first; the performance score only breaks an
  exact transfer tie. Other priorities intentionally use normalized mixed ranking.
- Intermediate emission candidates are retained round-robin across selected/raw/
  gzip/Brotli rankings (selected objective first), but final selection still follows
  only the configured objective and priority. The bounded frontier is not a proof of
  a mathematical global minimum.
- Split/preserve compilation does not run the full single-artifact emission search.
  Joint chunk/symbol search does carry its winning emission options into final output,
  but chunk config still needs its own fixtures rather than borrowing single-mode
  claims. Explicit chunk plans currently force
  `fresh-literal-factory-inlining-variants` off so their ownership/import plan cannot
  request a factory binding that emitter-local substitution suppressed.
- In `split`, every optional eager/shared chunk must strictly improve deploy cost and
  mandatory lazy chunks count toward `max_chunks`; exceeding the cap is a compile
  error. `preserve-modules` is exempt from that split-mode cap.

## Test form

Each config test should report:

1. parsed/resolved config;
2. enabled optimizer/search/compression decisions;
3. candidate count and rejection reason where explain data exposes it;
4. output hash and all size metrics;
5. semantic result.

Config validation failures are positive tests: unknown keys, duplicates, zero/over-
maximum budgets, invalid profile counters, and incompatible ABI choices should fail
with stable actionable diagnostics.
