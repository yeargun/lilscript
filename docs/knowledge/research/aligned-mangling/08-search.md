# The objective is separable — if you factor it along the right axes

Parent: [index](README.md). Produced by `factorial.mjs`. Scored with Node zlib
Brotli 1.1.0 q11 and confirmed on the winners with `lilscript-codec`.

## The question

Compiler transforms clearly affect each other. Inlining changes what naming
sees; naming changes what the window copies; pooling changes what the
dictionary can serve. If that coupling is strong and general, then no cheap
heuristic can work and every candidate must be scored in full — which is what
the beam does today, at 45–90 minutes per artifact
([07](07-ports.md)).

If the coupling is *structured*, a fast search exists. So: measure it.

## The design

Six transforms that can be applied to a finished artifact, each legality-checked,
each a pure text → text function:

| key | family | what it rewrites |
|---|---|---|
| `M` | naming (4 levels: as shipped, frequency/dialect, first-use/abc, first-use/dialect) | every identifier occurrence |
| `D` | merge adjacent declarations | declaration statement boundaries |
| `W` | `for(;t;)` → `while(t)` | loop keywords |
| `C` | outline `.slice`/`.exec`/`.replace` | call-site spellings |
| `P` | pool order by reversed string | the order of one declaration list |

Every point of the full grid is built and scored — 32 or 64 evaluations per
artifact — and from the responses we compute each factor's main effect, every
pairwise interaction, and how much of the total variance a purely additive
model explains.

## The result

| Artifact | naming as **two** switches | naming as **one** 4-level factor | top interaction, split design |
|---|---:|---:|---|
| `jquerylil/dist/jquery.esm.js` | R² 0.5510 | **R² 0.9968** | `N×O` 99% |
| `jquery-lilscript.raw.js` (in-tree) | R² 0.8061 | **R² 0.9947** | `N×O` 99% |
| `solidlil/reactive.generated.js` | R² 0.8340 | **R² 0.9896** | `N×O` 98% |
| `markedlil/dist/marked.raw.js` | R² 0.9278 | **R² 0.9631** | `N×O` 70% |

In the split design one interaction term — `N×O`, the two *renamings* — carries
70–99% of all the non-additivity.

`N` and `O` are both *renamings*. Applying one after the other does not compose
them — the second overwrites the first. Their "interaction" was never physics;
it was an artefact of modelling one decision as two switches.

Group them into a single categorical factor and the objective becomes
**99.0–99.7% additive** across families. On two artifacts the interaction
between naming and declaration merging is exactly **0.0%**.

## What that means for search

Interactions concentrate between factors that **rewrite the same bytes**, and
vanish between factors that rewrite disjoint bytes. That is a strong statement
about the shape of the problem, and it has a direct consequence:

> Partition the transforms by *which part of the artifact they rewrite*.
> Inside a partition, the choice is one categorical decision — enumerate its
> levels. Across partitions, the objective is additive to a fraction of a
> percent, so take each partition's best independently.

Cost falls from ∏|levels| to ∑|levels|. Measured on these artifacts:

| Artifact | exhaustive grid | greedy per factor | greedy leaves |
|---|---:|---:|---:|
| jquerylil | 32 points → 30,346 | 10 evaluations → 30,371 | 25 B (0.08%) |
| jquery-lil-raw | 64 points → **32,223** (−1,060, −3.2%) | 12 evaluations → 32,224 | **1 B** |
| solidlil | 32 points → 4,280 (−97) | 10 evaluations → 4,282 | 2 B |
| markedlil | 32 points → 9,487 (−22) | 10 evaluations → 9,487 | **0 B** |

Coordinate descent over correctly-factored families is within 0.08% of
exhaustive search over the product, at a third of the evaluations.

The jquery-lil-raw winner was checked the whole way down: `lilscript-codec`
reports **102,681 → 101,848 raw, 38,787 → 37,855 gzip, 33,283 → 32,223
Brotli — −1,060 bytes, −3.18%** — and all 37 jsdom observations are identical
to the shipped artifact. Its point is *rename + pool order*, with the three
structural families off. That is the largest verified number in this folder and
it came out of a 64-point grid that takes seconds, against a candidate search
that did not finish the same artifact in 4.5 hours.

## The distinction that matters

"Deltas do not add" is the folklore, and it is half right. What actually
happens is that deltas do not add **across mis-factored axes**, and add fine
across correct ones:

| Artifact | split design: stacking error | grouped design: stacking error | grouped: greedy leaves |
|---|---:|---:|---:|
| jquerylil | **601 B** | 0 B | 25 B |
| jquery-lil-raw | — | 14 B | 21 B |
| solidlil | 65 B | 0 B | 0 B |
| markedlil | 99 B | −32 B | 3 B |

So the failure [15 color-merge](../brotli-global-mangle/15-color-merge.md)
described qualitatively is not "the codec is chaotic". It is "two of those
knobs were the same knob". Fix the factoring and the cheap methods start
working: stacking is within tens of bytes, greedy within 25.

That is the actionable form of the whole page. The expensive thing is not the
search; it is searching a space you have parameterised badly.

## A family that loses alone and wins in context

`for(;t;)` → `while(t)` measured on its own against the markedlil artifact is
**+19 Brotli bytes** — a loss ([06](06-free-order.md) counsels declining it).
Its *main effect* across the grid, averaged over every other setting, is
**−12.9**. It appears in the best point of all four artifacts.

That is what a real interaction looks like, and it is the reason a family
should be screened by its main effect over the grid rather than by a single
isolated measurement.

## Honest limits

- This is a **six-factor screen on post-hoc rewrites**, not the compiler's
  actual search space. Real families — inlining, pooling, representation choice
  — change the program's structure, not just its spelling, and can plausibly
  couple harder. The claim here is about the *shape* of the objective and the
  method for measuring it, not that the compiler's own space is 99% additive.
- markedlil is the counter-example that keeps the rule honest: grouping naming
  did *not* lift its R² (0.951 → 0.941), and its declaration-merging factor
  carries a real interaction with naming (53.7% of the remaining variance).
  Partitions have to be validated per artifact, not assumed.
- The residual ~0.5% is the genuinely non-local part of the codec: shared
  window, shared prefix codes, one dictionary budget. It is small, and it is
  what stops any of this from being exact.
- **A grouped factor is only as good as its level set.** On jquery-lil-raw the
  split design found 32,223 and the grouped design 32,253: the extra 30 bytes
  come from applying *both* renamings in sequence, which composes to a naming
  none of the four enumerated levels contains. Grouping is the right move; it
  also means the level set has to be generated, not guessed.

## Heuristic

1. **Factor by what a transform rewrites.** Two knobs that touch the same bytes
   are one decision with several levels, never two switches.
2. **Screen families by main effect over a grid**, not by a single isolated
   measurement. A family that loses alone can be in every winning combination.
3. **Coordinate descent, re-measuring in context, is enough** — within 0.08% of
   exhaustive here, at ∑|levels| evaluations instead of ∏|levels|.
4. **Never add independently measured deltas.** Signs compose; magnitudes do
   not, by hundreds of bytes.
5. **Report R² and RMSE of the additive fit** whenever a new family joins the
   beam. If R² drops, the partition is wrong and the new family belongs inside
   an existing one.
