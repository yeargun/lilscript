# search-03 — the proposal budget starves the objective on real modules

Parent: [ledger](../LEDGER.md). Status: landed (finding + pack config); default unchanged.

## Question

`error-tracking` was configured `cost_model = "brotli"`, `candidate_search =
"production"`, `optimization_level = 15` and still shipped an artifact 976 Brotli
bytes larger than Oxc. Naming families that were demonstrably worth hundreds of
bytes exist in the beam. Why were they never selected?

## Current hypothesis

Confirmed: the search never reached them. `effective_candidate_proposal_limit_for_artifact`
(`src/config.rs`) scales the level limit by artifact size — `level_limit.div_ceil(4)`
above 16 KiB. At level 15 with `production` the level limit is `min(1536, 384) = 384`,
so an 18 KiB module gets **96** proposal work units against ~38 beam families.
`--explain human` reports `structural proposal work 95/96 (exhausted)`.

## Constraints specific to this task

Raising the default cost is a build-time decision for every artifact, not a
size decision — it is deliberately left alone here. An explicit
`candidate_proposal_limit` bypasses the artifact scaling but is still bounded by
the level and the search tier, so `candidate_search = "always"` is required to
reach 1536 at level 15.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-25 | shipped config, budget 96 | `lilscript src/error-tracking-entry.lil --config lilscript.toml --explain human` (posthoglil) | `structural proposal work 95/96 (exhausted)`, Brotli-11 5,978 | gate |
| 2026-08-25 | explicit `candidate_proposal_limit = 384` | same with `terminal_codec_probe_limit = 384` | 370/384 exhausted, Brotli-11 5,694, 9.3 s | gate |
| 2026-08-25 | `candidate_search = "always"`, limits 1536 | `lilscript.identity.toml` | proposal work 763/1536 (converged), terminal 384/384 (exhausted), Brotli-11 5,367, 13.5 s | gate |
| 2026-08-25 | `local_name_reserve` sweep at the starved budget | 10 compiles, `lilscript-codec --json` | Brotli-11 5,722 at 0 vs 5,978 at the configured 48 — the configured value was the worst of the ten | gate |

## Log

- 2026-08-25 — `max_candidate_raw_growth_percent` defaults to 0, so no candidate that costs raw bytes is admitted even under `cost_model = "brotli"`. Raising it to 2/5/10/25 changed the winner's raw size (18,363 → 17,688) but not Brotli (5,978 → 5,981). Not the blocker here; still a real asymmetry between the configured objective and the admission rule. — **OPEN**
- 2026-08-25 — The `stable_local_names: false` family already exists in the beam (`src/compiler.rs`, gated on `configured.stable_local_names`). It is not selected because the budget runs out before it is reached, not because it loses. `local_name_reserve` is in no family at all. — **OPEN**
- 2026-08-25 — `terminal_codec_probe_level_limit` caps at 384 at level 15 and is exhausted even with the proposal budget raised, so terminal work is now the binding constraint on this artifact. — **OPEN**
- 2026-08-25 — Unstarving the search exposed two miscompiles that the starved search never selected: [ident-06](ident-06.md) and the class-fusion identity drops in [ident-07](ident-07.md). A deeper search is only worth turning on behind those fixes. — **LANDED**
- 2026-08-25 — Shipped `posthoglil/lilscript.identity.toml` for the error-tracking pack rather than changing the compiler default. — **LANDED**

## Next step

Decide whether `candidate_proposal_limit` should stop being artifact-scaled when
`cost_model` is not `raw`, and whether naming policy (`local_name_reserve`,
`identifier_alphabet`) should become beam families. `IdentifierAlphabet::for_code`
and `::javascript_keyword` are implemented and never used in production
(`src/config.rs` always passes `canonical()`); the board already estimated
−180 Brotli for aligned mangling.
