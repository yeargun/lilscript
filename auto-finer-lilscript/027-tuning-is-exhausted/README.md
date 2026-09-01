# 027 — Config tuning is exhausted; what is left is emitted volume

**Status: NEGATIVE RESULT, and a useful one. A 20-build grid over the two closest losses buys 10
bytes, and the 10 bytes are not safe.**

## The grid

The two nearest misses on the fleet scoreboard were `remark-mathlil` (+137) and `unifiedlil` (+234).
Ten variants each, every build measured with the pinned codec:

| variant | remark-math (target 2097) | unified (target 4425) |
|---|---:|---:|
| **incumbent** | **2287** | **4659** |
| beam 8 @ L13 | 2337 | 4659 |
| beam 16 @ L13 | 2336 | 4659 |
| beam 20 @ L13 | 2336 | 4659 |
| beam 24 @ L13 | 2336 | 4659 |
| beam 28 @ L13 | 2336 | 4659 |
| beam 32 @ L13 | 2336 | 4659 |
| beam 48 @ L13 | 2336 | 4659 |
| level 14 | 2306 | 4666 |
| `assume_pure_property_reads` | 2311 | **4649** |
| `candidate_proposal_limit=1536` | 2336 | 4659 |

**Nothing beats the incumbent on remark-math**, and the single improvement anywhere is 10 bytes on
unified — which **fails 5 of its tests** and is therefore not available. (Those 5 failures turn out
to predate it: unifiedlil fails them on its *committed* config too. Not caused here, but recorded.)

## Two things worth keeping

**Beam width is port-specific, not a global knob.** It is worth 47 bytes on posthoglil (5668 → 5621,
a plateau across widths 22–26) and **exactly zero** on unified — 4659 at every width from 8 to 48.
Tuning it is a per-port measurement, never a default to raise.

**Level 13 is not universally the best bytes**, only the best *trade*. remark-math is 2287 at level
15 and 2336 at 13 — 49 bytes worse. The objective's "13 is the sweet spot" is a statement about
time-for-bytes, and this is the measurement that shows the two can diverge: on unified 13 also beats
15 (4674 vs 4696), so the direction is not even consistent between ports.

`unifiedlil` still gains 416 bytes from this session's config work — its committed config measures
**5075** and the level-13 + `always` config measures **4659**.

## Where the remaining gap actually is

[025](../025-brotli-repetition-gap/README.md) established that raw emitted volume predicts the losses
at r=+0.940. Profiling unified's artifact against Terser's shows what that volume is made of:

| | LilScript | official + Terser |
|---|---:|---:|
| functions | **100** | 68 |
| median function body | **124 B** | 96 B |
| total bytes inside functions | **20777** | 12334 |
| tiny helpers (<40 B) | 16 | 15 |
| `if(` statements | **0** | 72 |
| `var` declarations | **87** | 12 |

Not helper proliferation — the tiny-helper counts match. We emit **47% more functions, each 29%
bigger**. That is the lowering, and no search knob reaches it.

The zero `if(` is the compiler having converted every branch to an expression. That is usually the
right call for bytes (`return a?b:c` beats `if(a)return b;return c`), but paired with 87 `var`
declarations against 12 it suggests the expression form is being bought with hoisted temporaries that
cost more than the statements saved. That is the first concrete thing to measure next, and it is a
lowering question, not a configuration one.
