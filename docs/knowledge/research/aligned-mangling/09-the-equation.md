# The equation, why you cannot differentiate it, and what to do instead

Parent: [index](README.md). Produced by `analytic.mjs` and `libraries.mjs`,
scored with `lilscript-codec`.

## The objective is closed-form

There is no black box here. For a fixed parse, the size of a Brotli stream is
exactly

```
L(x) = H(θ) + Σ_commands [ ℓ(cmd) + ℓ(dist) + Σ ℓ(literal | ctx) ] + extra bits
```

where every `ℓ` is a prefix-code length — `−log2 p` of a symbol under that
block's own histogram θ — and `H(θ)` is the cost of describing the codes. The
decoder in [brotli-machine](../brotli-machine.html) evaluates every term of
that sum on a real stream; [01](01-where-the-bits-are.md) is that evaluation.

Two things stop it from being a function you can optimise directly:

1. **The parse is itself a choice.** Which bytes become literals and which
   become copies is an optimisation the encoder performs.
2. **θ depends on the whole block.** A local edit changes the price of every
   symbol everywhere. This is a mean-field coupling, not a local one.

## Why the derivative is useless

Freeze θ and `L` becomes a *linear functional* of the symbol counts, with a
perfectly well-defined gradient:

```
∂L/∂n_s = −log2 p_s
```

That is the natural "differentiate and step" move, and it is what any cost
model that prices a change locally — *this name costs three bits* — is
implicitly doing. Measured against the real codec over 32 design points per
artifact:

| Artifact | first order (θ frozen at the baseline) | full model (θ recomputed) |
|---|---|---|
| `solidlil/reactive.generated.js` | r 0.9721, mean error 24.9 B | r 0.9907, mean error 18.3 B |
| `markedlil/marked.raw.js` | r 0.7383, mean error 174.7 B | r 0.9261, mean error 20.6 B |
| `jquerylil/dist/jquery.esm.js` | **r −0.1373**, mean error 588.8 B | r 0.9939, mean error 70.1 B |

On the artifact whose edits are renamings, the gradient is **anti-correlated
with the truth**. The reason is structural rather than numerical: renaming
barely changes *which* symbols occur, it changes *the distribution itself*.
The entire effect lives in Δθ — the term the linearisation throws away.

So: the equation exists, it is exact, and its derivative at the current point
tells you almost nothing. That is what a mean-field problem looks like. You
cannot step along the field; you have to re-solve it.

## Re-solving it is cheap

Re-solving means: parse once, count symbols, price them at their own empirical
entropy. Closed form, one pass, no compression:

| Artifact | pearson | spearman | picks the winner | error after one scale factor | vs the codec |
|---|---:|---:|---|---:|---:|
| `solidlil/reactive.generated.js` | 0.9907 | 0.9472 | within 5 B | 7.5 B (0.17%) | **5.9× faster** |
| `markedlil/marked.raw.js` | 0.9261 | 0.9021 | within 35 B | 19.3 B (0.20%) | **9.4× faster** |
| `jquerylil/dist/jquery.esm.js` | 0.9939 | 0.9736 | **exactly** | 42.2 B (0.14%) | **7.7× faster** |

That is the usable object: an inner-loop scorer that ranks candidates to
within a fifth of a percent, in one pass, in unoptimised JavaScript against a
C implementation of q11. The real codec stays where it belongs — ranking the
finalists, and gating.

## The hardness is real, and it is in the obvious place

The natural surrogate objective this folder measured —
*use fewer distinct names* ([05](05-concentration.md)) — is, stated exactly:

> minimise the number of distinct names, subject to two bindings sharing a name
> only when they do not interfere

which is graph colouring on the interference graph, and NP-hard. The true
objective is worse: colouring does not even know about the codec. So exact
optimisation is out, and this is not a gap in anyone's cleverness — it is the
problem.

What is *not* out is the structure. [08](08-search.md) measured it: once
transforms are factored by what they rewrite, the objective is 99.0–99.7%
additive across families, and coordinate descent lands within 0.08% of
exhaustive search over the product. Combine that with a scorer that is 8×
cheaper than the codec and the search becomes: enumerate levels inside each
partition, evaluate analytically, confirm the finalists with the real codec.

## What it buys on the shipped libraries

The whole pipeline — build the grid, take the best point, prove each step,
score with the gate codec — run over 14 artifacts from five LilScript
libraries. **12 seconds of search, total.**

| Library | Artifact | Brotli-11 | Δ | | Verified by |
|---|---|---:|---:|---:|---|
| jquerylil | `jquery.esm.js` | 30,973 → 30,397 | **−576** | −1.86% | 12 jsdom observations |
| posthoglil | `error-tracking.raw.js` | 6,200 → 6,020 | **−180** | −2.90% | export surface |
| solidlil | `reactive.generated.js` | 4,377 → 4,282 | **−95** | −2.17% | 7 reactive observations |
| posthoglil | `otlp.raw.js` | 2,594 → 2,543 | **−51** | −1.97% | export surface |
| motionlil | `animate.js` | 26,348 → 26,329 | −19 | −0.07% | export surface |
| markedlil | `marked.raw.js` | 9,543 → 9,527 | −16 (raw **−945**) | −0.17% | 680 spec cases |
| motionlil | `mini.js` | 11,085 → 11,070 | −15 | −0.14% | export surface |
| motionlil | `scroll.js`, `index.bundle.js`, `full.js` | — | 0 | | |
| posthoglil | `surveys`, `replay-core`, `autocapture`, `posthog` | — | 0 | | |

**196,441 → 195,489 Brotli bytes, −952, −0.48% overall**, every step
individually proved and every winner behaviourally identical.

Read the zeros as carefully as the wins:

- **motionlil is already at this grid's optimum** on its three largest
  artifacts, and the only thing that moves the other two is pool ordering.
  Those files come out of a bundler, and the pattern from
  [07](07-ports.md) repeats exactly: where something re-mangles downstream,
  naming has nothing left.
- **posthoglil splits.** Two of six modules carry 2–3%; four are at the
  optimum. The two that move are the two largest compiler emits.
- **markedlil gives back 945 raw bytes for 16 Brotli bytes** — the same
  tie-break as [07](07-ports.md), now on a third artifact.

## Proofs, and one caution about them

Each factor gets the check it admits, applied to *its own step* rather than to
the composite:

| Factor | Proof |
|---|---|
| renaming, declaration merging | the resolution sequence: every identifier occurrence resolves to the same binding, in source order |
| pool order | canonicalisation: sorting every pool's declarators makes the two texts byte-identical, and the run has literal initialisers and distinct names |
| `for`→`while`, call outlining | **none exists** — outlining adds bindings by construction — so they are excluded from a proved search and need a battery |

Building that exposed a false positive in this folder's own tooling: the
resolution-sequence check indexes bindings by declaration position, so a
*reordering* rewrite permutes the index and the check fails on a program that
is perfectly correct. A checker is only valid on the rewrites it was designed
for, and composing rewrites does not compose their proofs.

## Heuristic

1. **Write the equation down.** It is `Σ −log2 p` plus a header, and every term
   is computable from one parse.
2. **Do not differentiate it.** The gradient at the current point is
   uncorrelated to anti-correlated with the truth for the edits that matter.
3. **Re-solve instead.** Recomputing the histograms is one pass and ranks
   candidates to 0.15%, eight times cheaper than compressing.
4. **Accept the hardness and exploit the structure.** The surrogate is NP-hard;
   the objective is 99% separable once factored correctly. Enumerate levels
   inside a partition, greedy across partitions, exact codec on the finalists.
5. **Prove each step with the check that step admits**, and behaviourally test
   anything that has no proof.
