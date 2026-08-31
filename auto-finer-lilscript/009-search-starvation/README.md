# 009 — The search is starved, not too narrow

**Status: CONFIRMED. This reframes the whole effort ladder.**

## Hypothesis

[007](../007-level-13-sweet-spot/README.md) showed level 15 costs 20x the CPU of level 13 for 1.4%
of the bytes on jQuery, and that levels 12-14 sit on a flat plateau. The obvious reading is "the
search has already found everything worth finding". This hypothesis tests a different reading: that
the search *never gets to try most of its own ideas*, and raising the level adds ideas without
adding budget — so the extra effort is spread thinner instead of dug deeper.

The compiler already reports this. `--explain json` emits `scored_emission_families`,
`starved_emission_families`, and `search_stop_reason`.

## Measurement — acorn port, `candidate_search = "production"`

| level | families scored | families **starved** | starved % | Brotli-11 | emissions | stop reason |
|---|---:|---:|---:|---:|---:|---|
| 15 | 45 | **28** | 62% | 3069 | 304 | `work-budget-exhausted` |
| 13 | 45 | **23** | 51% | 3071 | 306 | `work-budget-exhausted` |
| 11 | 41 | **9** | 22% | 3102 | 308 | `work-budget-exhausted` |

## Findings

1. **The search runs out of budget at every level.** `search_stop_reason` is
   `work-budget-exhausted` at 11, 13, and 15 alike. It never converges; it is always cut off.
2. **Emission count is essentially constant across levels — 304, 306, 308.** The effort ladder is
   not buying more emissions. What it changes is how many *decision families* compete for the same
   fixed number of emissions.
3. **Raising the level therefore increases starvation.** Level 15 enables 45 families and starves 28
   of them; level 11 enables 41 and starves 9. Higher levels dilute rather than deepen. This is a
   complete explanation of the flat 11→15 byte curve (3102 → 3069, 33 bytes) that 007 measured, and
   of why level 15's extra wall clock buys so little.
4. **Among the starved families at level 13 are `string-pool-minimum-savings` and
   `local-name-reserve`** — precisely the two knobs that
   [008](../008-jquery-compressibility-gap/README.md) independently fingered as mis-set on jQuery.
   The compiler already knows how to tune them and never gets the budget to try. Also starved:
   `function-spelling`, `function-layout`, `conditional-expressions`, `comma-expressions`,
   `switch-lowering`, `loop-spelling`, `mutation-spelling`, `structural-control-flow`,
   `inline-single-use-functions`, `host-alias-spelling`.

## The decisive experiment

If starvation is the binding constraint, then *feeding the starved families at a low level* should
beat *enabling more families at a high level*. Level 13 with the terminal codec probe budget raised
from its level default of 192:

| configuration | families starved | **Brotli-11** | emissions |
|---|---:|---:|---:|
| L13 default (`terminal_codec_probe_limit` = 192) | 23 | 3071 | 306 |
| L13 + `terminal_codec_probe_limit = 1536` | 23 | **3063** | 306 |
| L13 + `candidate_proposal_limit = 1536` | 15 | 3071 | 360 |
| L13 + both = 1536 | 15 | **3063** | 360 |
| *(reference)* L14 default | — | 3068 | — |
| *(reference)* L15 default | 28 | 3069 | 304 |

**Level 13 with a wider terminal probe budget produces a smaller artifact than level 15 does**
(3063 vs 3069) with the same number of emissions. The last bytes are in the terminal search, and the
level ladder was rationing exactly the wrong thing.

Note the two budgets do different work and only one of them pays here: raising
`candidate_proposal_limit` cuts starvation from 23 families to 15 and buys **zero** bytes, while
raising `terminal_codec_probe_limit` leaves the starvation count unchanged and buys all 8 bytes.
Starvation count is a diagnostic, not the objective — a reminder not to optimize the proxy.

## Interpretation

The effort ladder conflates two independent things:

- **breadth** — how many decision families are eligible (what `minimum_level` controls), and
- **depth** — how much whole-artifact work each eligible family may spend.

Levels raise both, but the second is raised far more slowly than the first, so higher levels
systematically increase the ratio of ideas to budget. That is why the byte curve is flat and the
time curve is not.

## Landed: the probe ladder retune

Before landing, the jQuery side was measured properly, because
`effective_terminal_codec_probe_limit_for_artifact` scales the level base down by artifact size —
level 13's nominal 192 becomes roughly **42** on a 90 KB artifact, so the unscaled experiments above
overstate what a ladder change actually delivers.

Pinning the budget explicitly on jQuery, to separate the two effects:

| probes (explicit, unscaled) | Brotli-11 |
|---|---:|
| 42 (approximately what level 13 actually reached) | 30657 |
| 84 (what doubling the base reaches after scaling) | **30593** |
| 384 | 30587 |
| 768 | 30550 |

Doubling the base captures **87%** of the gain that four times the probes achieves, for half the
probes. So `terminal_codec_probe_level_limit` became `13 => 384` (from `13 => 192`).

### ...and the 14/15 half of that was wrong, and was reverted

The level-13 number was measured. The 512 and 768 for levels 14 and 15 were **extrapolated** from
"jQuery was still gaining at 768 unscaled probes", and flagged as such at the time. Then they were
measured:

| jQuery at level 15 | CPU seconds | raw | gzip-9 | Brotli-11 |
|---|---:|---:|---:|---:|
| before this workstream (384 probes) | 1829 | 92706 | 33713 | 30225 |
| with the extrapolated 768 probes | **5434** | 84599 | 33498 | 30033 |

**Three times the CPU — ninety minutes on one artifact — for 192 Brotli bytes.** That is precisely
the trade [007](../007-level-13-sweet-spot/README.md) criticized level 15 for, made worse by this
change. It was reverted: the ladder is now `11..=12 => 128, _ => 384`, so 13 keeps its measured
knee and 15 is restored to exactly the budget it had before. Level 13's output is byte-identical
across the revert, confirming the two are independent.

Two lessons worth keeping. **Extrapolating a ladder along one dimension because one artifact was
still improving is not measurement** — the byte curve kept going, the cost curve went vertical, and
only measuring the pair showed it. And a knee measured on a 26 KB artifact (acorn, flat from 384
through 3072) does not license raising the budget for a 90 KB one, because the *cost* of a probe
scales with artifact size even when the *benefit* does not.

**Verified end to end with no explicit override, level 13:**

| port | before | after |
|---|---:|---:|
| jQuery | 30651 | **30593** (-58) |
| acorn | 3071 | **3063** (-8) |
| acorn at level 15, for contrast | 3069 | 3064 |

Level 13 now produces a *smaller* acorn artifact than level 15 did before the change, and a smaller
one than level 15 produces after it. Levels 14 and 15 are extrapolated rather than measured — jQuery
was still gaining at 768 unscaled probes — and `src/config.rs` says so at the site.

## Recommended follow-up (not yet landed)

1. **Re-measure the ladder on a third large port.** The retune is justified by two artifacts. gzip
   moved only 31 bytes on jQuery (34197 -> 34166) against Brotli's 58, so a gzip-objective project
   may see less benefit for the same cost.

2. **Stop raising breadth without raising depth.** A level that enables 45 families but can fund 22
   is worse-specified than one that enables 22 and funds all of them. The ladder should keep the
   families-to-budget ratio roughly constant.
3. **Allocate by payoff rather than fairly.** `CodecBudget::begin_fair_slice` splits the budget
   evenly across families. Families have wildly different hit rates; a policy that spends the tail
   of the budget on families that have already paid off would extract more from the same work.
