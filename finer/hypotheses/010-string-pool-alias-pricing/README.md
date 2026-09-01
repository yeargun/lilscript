# 010 — The string pool priced every alias as one character

**Status: REVERTED. The cost model *is* wrong, but correcting it per-candidate is net-negative
because the decisions are coupled through a shared name allocator. Measured: −8 Brotli on markedlil,
**+282 on jQueryLil**.**

## How it was found

[008](../008-jquery-compressibility-gap/README.md) audited the top-level bindings of the jQuery
artifact and asked, for each pooled string, whether it actually pays:

| binding | literal | uses | raw bytes saved |
|---|---|---:|---:|
| `If` | `""` | 105 | **−6** |
| `Jf` | `" "` | 31 | +24 |
| `Kf` | `"*"` | 14 | +7 |
| `Lf` | `"application/x-www-form-urlencoded"` | 2 | +27 |
| `Nf` | `"border-box"` | 4 | +24 |

Pooling the **empty string** across 105 uses *loses* bytes: `""` is two characters and so is `If`,
so there is no per-use saving at all, only the cost of the declaration. And it is worse than the raw
number suggests under a compressing objective — 105 repetitions of `""` were already a trivial LZ
match, so the pool traded a free repetition for a new token.

## Cause

`assign_string_aliases` (`src/codegen_ir_js.rs`) scored candidates as:

```rust
let literal_length = value.len() + 2;
let unaliased = count * literal_length;
let aliased = literal_length + 7 + count;   // <- one byte per use
let savings = unaliased.saturating_sub(aliased);
```

`+ count` charges **one character per use**, i.e. it assumes every alias is a single-character name.
The pool draws from the same top-level identifier namespace as every other module binding, so on any
artifact with more than a handful of module bindings most aliases are two characters wide. For `""`
the model computes a saving of 96 bytes; the real result is a 6-byte loss.

This over-estimates savings for *every* pooled string, not just the pathological one. It only
changes the decision for literals short enough that the error flips the sign.

The fix was already sitting in the same file: `assign_numeric_aliases`, twenty lines below, clones
the mangler, takes the name the candidate would actually receive, and prices against `name.len()`.
The string path now does the same. Ranking is untouched, so candidate ordering — and therefore
determinism — is unchanged; only the final admission test is honest.

## Measurement (level 13, after the 009 ladder retune)

| port | raw | gzip-9 | Brotli-11 |
|---|---:|---:|---:|
| jQuery | 89861 (**+12**) | 34160 (**−6**) | 30590 (**−3**) |
| acorn | 25955 (0) | 3574 (0) | 3063 (0) |

## Findings

1. **The win is small and the honest reason is second-order effects.** Rejecting one alias frees the
   name it would have taken, so every subsequent pooled binding shifts by one name and some get
   longer. The direct 6-byte saving is mostly eaten by that reshuffle; what survives is −3 Brotli and
   −6 gzip. **Raw goes up 12 bytes**, which is worth stating plainly rather than burying: on this
   artifact, under a Brotli objective, the search is content to spend raw bytes, and it did.
2. **It is still worth landing**, because a cost model that assumes a one-character alias is wrong
   independently of what any one artifact measures, and it was wrong in the direction of admitting
   candidates that lose bytes. Both measured artifacts improved or held on their objective metric,
   and the full suite is unchanged (1629 pass; the one failure predates this workstream).
3. **This is not the jQuery compressibility gap.** 008 measured that disabling string pooling
   *entirely* is worth 50 Brotli bytes, and this fix recovers 3 of them. The other 47 are in
   candidates that pass an honest per-string test and still hurt a compressing objective. The
   admission threshold `string_pool_minimum_savings` is already objective-scaled
   (`src/config.rs:249` — Raw 1, Gzip 4, Brotli 8), so the follow-up is that **8 is too low under
   Brotli**: threshold calibration, not a missing mechanism.


---

# Reverted, and why

[015](../015-does-this-work-help/README.md) compiled the two sibling ports whose source and config
are unchanged — the only clean compiler comparison available — against a compiler built from
`lilscript` HEAD. This change was isolated behind a switch and run three ways on jQueryLil:

| compiler | raw | gzip-9 | Brotli-11 |
|---|---:|---:|---:|
| `lilscript` HEAD | 83681 | 32480 | **29209** |
| this workstream, alias fix **on** | 87543 | 32763 | **29491** |
| this workstream, alias fix **off** | 83681 | 32480 | **29209** |

Two things fall out. **This change alone accounts for the entire +282 Brotli regression**, and
**every other change in this workstream is byte-neutral on jQueryLil** — with the fix off, the
output is byte-identical to HEAD's.

Weighed across the two measurable ports: **−8 Brotli on markedlil against +282 on jQueryLil.**
Reverted.

## What was actually learned

The premise is still correct: pricing every alias at one character is wrong, and it really does
admit `""` across 105 uses into a two-character name at a net loss. What the measurement adds is
**why fixing it per-candidate does not work**.

Skipping one candidate frees the name it would have taken, so **every later pooled binding shifts by
one name** — some getting shorter, some longer. That reshuffle is larger than the direct saving, and
its sign depends on the artifact. 010's own first measurement already showed the tell and I
under-weighted it: raw moved the *wrong way* (+12) on the benchmark port while Brotli moved −3. A
6-byte direct saving that nets to −3 after reshuffle is not a fix, it is noise with a favorable sign
on one artifact.

**The pooling decisions are coupled through the shared top-level allocator, so they cannot be priced
one at a time.** A correct fix scores the whole assignment jointly — pick the set of pooled strings
and their names together — which is a search problem, not an admission test. That is recorded at the
call site in `src/codegen_ir_js.rs` so the naive version is not re-attempted.

## Process note

This change was landed on a −3 byte result from a single artifact, with a caveat about second-order
effects written into this document at the time. That caveat was the finding; it should have blocked
the landing until a second port confirmed it. The generalizable rule: **a change whose measured
effect is smaller than its own known second-order noise has not been measured yet.**
