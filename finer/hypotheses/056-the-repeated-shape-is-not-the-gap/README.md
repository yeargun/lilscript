# 056 — The repeated shape is not the gap

**Status: FALSIFIED, with the mechanism found.** Converging alike scopes onto one spelling loses on
every port measured, and it loses for a reason the LZ argument never accounts for: **frequency-ordered
naming is itself an entropy-coding optimisation**, and it is worth more than the matches convergence
buys. Measured on jquerylil: identifier-byte entropy 3.9785 → 4.0663 bits, +82 Brotli.
Lane: measure. Objective: brotli. Ports: jquerylil, mobxlil, markedlil, katexlil, react-markdownlil,
motionlil. Opened: 2026-09-03.

## Prior art

Neither Terser nor Oxc has a cross-scope notion of structural equivalence. Terser's mangler is
per-scope and frequency-ordered (`TERSER/mangle.js`, base54 weighted by source character frequency);
Oxc's is the same shape. Closure's `RenameVars` optimises name *length* and reuse, never agreement
between two structurally alike scopes. 053 measured the consequence: Terser's mangler over our own
finished artifact is worth −251, an eighth of the +2,113 the identifier stream is behind.

**What the reading missed, and this folder found:** every one of those manglers ranks by frequency,
and the literature — and the brief this folder came from — treats that as a *raw-size* heuristic that
a compressing objective should discard. It is not. Assigning the commonest letter to the most-used
binding skews the identifier byte distribution, and Brotli's literal coder collects that skew
directly. Frequency ranking is a compression optimisation that happens to also minimise raw bytes.

## Claim

That the identifier-stream gap is structurally alike code spelled differently: two subtrees doing the
same thing, given different names by SSA specialisation, so the compressor sees novelty where it
could have seen a copy. Predicted lever: a shape key driving canonical naming, then layout, then
near-miss alignment. **Confirms** ≥ −400 Brotli on katexlil from naming alone. **Falsifies** under
−400, or coverage at or below the baseline's.

## Method

Pinned codec (`lilscript-codec`, Brotli-11/lgwin 22, gzip-9), acorn for offline skeletons, the
compiler's own `BindingResolution` for every legal rename. Ceilings are unbuildable limits that bound
any admissible pass; the end-to-end rows are real compiles.

`LILSCRIPT_NAME_ORDER` = `uses` (incumbent) | `decl` | `first` | `shape`, with
`LILSCRIPT_SHAPE_MIN_SPAN`, and `LILSCRIPT_RENAME_TEMPLATES` for the quarantine below. All default
off; behaviour is unchanged without them.

## Result

### 1. The cost constants — the only part that survives

Append a region to a real artifact in two conditions of identical length and near-identical byte
histogram: an exact copy from distance `D`, and the same region with its lowercase letters permuted.

| | Brotli | gzip |
|---|---|---|
| match cost growth with length (`eps`) | **≈ 0** at every distance | 0.011 @1 KiB → 0.33 @32 KiB → **0.47 @128 KiB** |
| novel text (`lambda`) | **≈ 0.43 bytes/byte** | ≈ 0.44 bytes/byte |

A Brotli match costs the same whatever its length; a gzip match stops paying past its window. These
are the correct affine-gap parameters for any future alignment, and they say a shape decision cannot
be shared between the two objectives.

### 2. There is no material at function granularity

| artifact | functions | exact classes | coverage | ceiling (brotli) |
|---|---:|---:|---:|---:|
| katexlil | 522 | 25 / 67 members | **1.9%** | **−23** |
| jquerylil | 590 | 32 / 88 members | 3.8% | −96 |

### 3. At subtree granularity we are already ahead of the baseline

Every AST node with span ≥ 40 bytes, skeletonised, grouped, members selected without overlap:

| artifact | classes | members | coverage | ceiling brotli |
|---|---:|---:|---:|---:|
| katexlil | 128 | 1009 | **30.1%** | −410 |
| **upstream katex (Terser)** | 134 | 719 | **26.3%** | **−373** |
| react-markdownlil | 84 | 268 | 18.6% | −464 |
| motionlil | 133 | 385 | 18.5% | −883 |
| mobxlil | 33 | 73 | 7.4% | −141 |
| jquerylil | 30 | 93 | 7.0% | −151 |

**We carry more repeated structure than the baseline, and the baseline has the same headroom we do.**
katexlil's largest classes are data, not code — `g("math","main","rel","⊑","\\sqsubseteq",h)` × 220 —
already converged, so converging them buys nothing.

### 4. Near-miss clustering adds nothing

Re-clustered on a relaxed key (the multiset of structural node types, so a one- or two-operation
difference collapses into the same bucket — a generous bound on what alignment could reach):

| artifact | coverage exact → relaxed | ceiling exact → relaxed |
|---|---|---|
| katexlil | 30.1% → 30.6% | −410 → **−328** |
| jquerylil | 7.0% → 7.2% | −151 → **−132** |

The ceiling gets *worse*: relaxing merges classes that do not actually align. The DP has no material,
so it is unbuilt.

### 5. Convergence loses end to end, on every port

`converge_local_names` run in isolation over finished artifacts (raw byte-identical in every row —
this is a pure spelling change):

| artifact | order | rewrites | brotli | Δ |
|---|---|---:|---:|---:|
| jquerylil | `uses` | 836 | 28233 | |
| jquerylil | `first`/`decl` | 3325 | 28315 | **+82** |
| jquerylil | `shape`, span ≥ 20/40 | 854 | 28293 | **+60** |
| jquerylil | `shape`, span ≥ 80/160 | 836 | 28233 | 0 (nothing fires) |
| mobxlil | `uses` | 481 | 15561 | |
| mobxlil | `first` | 1261 | 15635 | **+74** |
| mobxlil | `shape`, span ≥ 40 | 481 | 15561 | 0 (nothing fires) |

Monotone in departures from use-count order. Full compiles agree: markedlil level 15 **+11**,
jquerylil level 15 **+104** (82210/31694/28387 → 82340/31821/28491), so the search does not rescue it.

### 6. The mechanism

| order | identifier entropy | modelled cost | top bytes |
|---|---:|---:|---|
| `uses` | **3.9785** bits/byte | 7006 | e:2910 t:2240 n:1761 r:1456 i:920 |
| `first` | **4.0663** bits/byte | 7160 | e:2877 t:2212 n:1590 r:1294 i:840 |

Over jquerylil's 14,087 identifier bytes that is **+154 modelled against +82 measured** — the gap
being the match term, which is real and roughly half the size of the entropy term it has to pay.
Use-count ranking concentrates identifier mass on the few commonest letters; convergence flattens it.
**The cost is global and the gain is local**, which is why a global reordering cannot win: it pays
everywhere to collect in a few places.

### 7. Reordering is negative at the ceiling

| artifact | clustered by class | greedy similarity chain |
|---|---:|---:|
| katexlil | **+171** | +77 |
| upstream katex | **+480** | +129 |
| jquerylil | **+151** | +71 |

Source order beats both strategies on both codecs, which also retires the `CompressionSimilarity`
repair this folder was going to make (`codegen_ir_js.rs:636-641` profiles named text).

### 8. Found on the way: the rename bails on a whole artifact for one template

`scan_template` (`token.rs:307`) swallows `${...}` substitutions into one token, so an identifier
inside one is invisible to the resolver and a rename would leave it behind. The pass therefore
refused *every* scope in any artifact carrying a single template — including katexlil,
react-markdownlil and motionlil, three of the four with the highest ceilings above.

The sound narrowing, behind `LILSCRIPT_RENAME_TEMPLATES`: quarantine the *names*, not the artifact.
Every identifier-like substring inside any template becomes untouchable in both directions — a
binding spelled that way keeps its name, and nothing may be renamed to it. Deliberately over-broad
(it also collects words from the literal text), so it can only reserve more names than necessary.

| artifact | rewrites unlocked | brotli | gzip |
|---|---:|---:|---:|
| katexlil | 1499 | **−87** | −37 |
| react-markdownlil | some | +20 | −13 |
| remarklil | 857 | +37 | +18 |
| motionlil | 0 (bails for another reason) | 0 | 0 |

All four inside the ±100 band. The quarantine is correct and it unlocks a pass that was silently
dead, but the pass is not worth much on these artifacts. Kept off by default; it wants its own folder
with a real test-suite gate before it flips, since `node --check` only proves the output parses.

## Verdict

Falsified at every granularity — function, subtree, near-miss — and at every scale — whole-artifact,
selectively gated, and through the full search on two ports. The premise is backwards: our artifacts
are *more* structurally regular than Terser's, not less.

What survives:

- **The cost constants** (§1). Any pass pricing a rewrite against a compressing objective should use
  these rather than character counts.
- **The entropy finding** (§6), which is the reusable one: frequency-ordered naming is not a raw
  heuristic to be replaced under compression, it is a first-order entropy optimisation that Brotli
  pays for directly, and it is roughly twice the size of the match term that competes with it. Any
  future proposal to re-spell names for the sake of repetition has to clear this bar first.
- **The template quarantine** (§8), off by default, as a lead for its own folder.
- **The negative on reordering** (§7), a second independent falsification of 029's class.

## Measurement hazard

katexlil's `dist/` was being rewritten by a concurrent session during this work: `katex.esm.js` moved
from 275,444 raw / 64,907 Brotli to 250,708 / 62,755 mid-folder. Every comparison here is
within-run (input and output measured together in one codec invocation), so the deltas hold, but
katexlil's absolute numbers are not comparable across sections. status.md's warning applies —
measure when no agent's fleet pass is rewriting `dist/`.

## Next

Not this. For katexlil, 055 already attributes the gap to an untyped port, and 025's r=+0.92 volume
relation explains the rest. The compiler lever this folder went looking for is not there.
