# search-04 — what the proposal budget actually buys, per objective

Parent: [ledger](../LEDGER.md). Status: landed. Predecessor: [search-03](search-03.md).

## Question

`search-03` showed the budget starves one 18 KiB module. Does the objective
selected in config (`javascript.cost_model`) win its own metric once the budget
is lifted, and is more search always better?

## Current hypothesis

Two answers, both confirmed by measurement:

1. **The budget is the scarce resource, not the beam.** Widening the beam or
   admitting raw growth *loses*, because a fixed pool of proposal work units is
   spread over more finalists and each family gets fewer proposals.
2. **The objective is honored where the search converges.** On otlp and surveys
   each cost model wins its own metric. On error-tracking `brotli` wins all
   three — `raw` loses raw by 252 and `gzip` loses gzip by 155 — because the
   search still exhausts its terminal budget there.

## Constraints specific to this task

Compile time roughly doubles at the lifted budget (error-tracking 5.4 s → 10.9 s).
That is why the compiler default is unchanged and the lift lives in the pack
configs.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-25 | cost model vs its own metric, `production` | 3 compiles of error-tracking | every metric won by a *different* cost model than the configured one | gate |
| 2026-08-25 | same at `always`/1536 | 9 compiles across et/otlp/surveys | otlp and surveys honored on all three; error-tracking loses raw and gzip to `brotli` | gate |
| 2026-08-25 | `candidate_beam_width` 12 → 24 → 48 | error-tracking, brotli | 5,156 → 5,194 → 5,240; wider is worse | gate |
| 2026-08-25 | `max_candidate_raw_growth_percent` 0 → 10 | error-tracking, brotli | 5,156 → 5,329; allowing raw growth is worse | gate |
| 2026-08-25 | `local_name_reserve` sweep at the lifted budget | 16 compiles | best 96 for error-tracking (spread 203 B), best 8 for otlp (spread 50 B) | gate |
| 2026-08-25 | `local_name_reserve` as a beam family | `src/compiler.rs`, 4 variants | error-tracking +75, autocapture +6, otlp −6, others 0 — **reverted** | gate |
| 2026-08-25 | budget lift on the other four packs | `lilscript.deep.toml` | Brotli −28 surveys, −39 otlp, −85 autocapture, −26 replay-core | gate |
| 2026-08-25 | emitted-code performance model, same config, before vs after this session | `--explain human` on all five packs | performance score flat (autocapture −0.1%), deoptimization risk flat, allocation pressure flat, parse cost −5.3% on error-tracking | gate |
| 2026-08-25 | performance model, shallow vs lifted budget | `--explain human`, error-tracking | parse cost −14%, startup memory −12%, performance score −0.07% | gate |

## Log

- 2026-08-25 — Adding exploration under a fixed work budget is a *trade*, not a gain: wider beams, raw-growth admission, and an extra naming family all lost. Any future family has to argue against the families it displaces. — **LANDED**
- 2026-08-25 — `IdentifierAlphabet::for_code` / `::javascript_keyword` are implemented and unused, but the lead is spent: the emitted single-character assignment is already frequency-ordered (`t e r n i a o s …` against a derived `e t r n a s i c …`), and a diagnostic remap made Brotli *worse*. `remap_single_character_identifiers` already probes this for non-raw cost models. Do not build an alphabet family on the −180 estimate; it is stale. — **REJECTED**
- 2026-08-25 — Terser on our own output (diagnostic, never shippable) leaves 5,156 → 5,022 on error-tracking: 87 from compress-class folds, 61 from re-mangling. That is the remaining headroom on this artifact and it is small. — **OPEN**

- 2026-08-25 — Similarity-ordered function layout, on the theory that shortening LZ
  distances between alike bodies pays: a greedy nearest-neighbour chain over
  6-byte shingles, applied to hoisted top-level `function` declarations only.
  Brotli got *worse* on every artifact — jQuery +96, Monaco +248 (and gzip
  +1,083), error-tracking +2, marked unchanged. The emitter's existing layout,
  which groups by call structure, already beats naive textual clustering. A
  layout family would have to score orderings with the real codec, not a
  similarity proxy. — **REJECTED** as specified
- 2026-08-25 — Near-duplicate function merging (bodies identical up to
  identifiers, numbers and strings): 0 groups on error-tracking, jQuery and
  marked; 2 groups worth 256 bytes on Monaco, 0.2% of the file.
  `identical_private_function_folding` and `permuted_private_function_merging`
  have already taken this. — **REJECTED**

## Next step

Decide the budget question properly: should `effective_candidate_proposal_limit_for_artifact`
stop scaling by artifact size when `cost_model` is not `raw`? That is the one
change that would make the objective reachable without a per-pack config, and
its cost is compile time. Measure the compile-time curve across the corpora
before changing the default.
