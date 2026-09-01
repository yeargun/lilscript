# 035 — Where the compiler-side headroom actually is, measured

**Status: MEASURED, NOT YET ACTED ON. Two leads quantified, one of them large, and one common
assumption ruled out.**

## Method

Run Terser over **our own shipped artifacts** in three modes and measure with the pinned codec. The
modes separate what a competitor's *formatting* buys from what its *transforms* buy from what its
*renaming* buys:

| port | reprint (format only) | + compress | + mangle |
|---|---:|---:|---:|
| jquery | −93 | −359 | **−1136** |
| micromark | **0** | −254 | **−884** |
| mobx | −50 | −222 | **−856** |
| marked | −36 | −95 | −84 |

All figures are Brotli against the artifact as it ships. Measured after
[034](../034-escaped-non-ascii/README.md), so the escape defect is not in these numbers — an earlier
run of this same experiment was contaminated by it and by the fleet rebuilding the files underneath,
and was discarded rather than recorded.

## Ruled out: our printer

**A pure reprint of our output saves nothing** — zero bytes on micromark, under a hundred on the
others. Whatever LilScript's remaining gap is, it is not how we spell what we decided to emit. That
closes a line of enquiry [027](../027-tuning-is-exhausted/README.md) had left open when it found
`0 if(` against Terser's 72 and wondered about emission style.

## Lead 1: transforms we do not have, worth 95–359

Terser's `sequencesize_2` absorbs a preceding simple statement into the *next* statement's expression
slot. Probing our pipeline with the five shapes it handles:

| shape | Terser | us |
|---|---|---|
| `e;return x` → `return e,x` | yes | **no** |
| `e;if(c)…` → `if(e,c)…` | yes | **no** |
| `e;for(i;;)` → `for(e,i;;)` | yes | **no** |
| `e;switch(x)` → `switch(e,x)` | yes | **no** |
| `e;for(k in o)` → `for(k in(e,o))` | yes | **no** |
| `a;b` → `a,b` (its `sequencesize`) | yes | **yes** |

Worth stating plainly: each of these is **raw-neutral on its own** — `a=1;return x` and
`return a=1,x` are both twelve bytes. The value is that removing a statement boundary lets other
folds reach across it. That makes it a poor first target despite being the obvious missing piece.

## Lead 2: name allocation, worth up to 1136

This is the larger one, and on jquerylil it is **62% of that port's entire 1825-byte gap**.

| micromark | one-char names used | their occurrences | two-char occurrences | avg length of the 60 most frequent identifiers |
|---|---:|---:|---:|---:|
| ours | **41** of 54 | 3781 | **4093** | **3.15** |
| Terser | **54** of 54 | 6975 | 513 | **2.90** |

Two distinct deficiencies:

1. **We leave thirteen single-character names unused** — `G H I J K L M N O U W X Y`. We take all 26
   lowercase, `A`–`F`, `P`–`T`, `V`, `Z`, and stop.
2. **We do not order by frequency.** Our sixty most-frequent identifiers are 3.15 characters on
   average against Terser's 2.90, and we spend 4093 occurrences on two-character names where Terser
   spends 513. Terser gives the shortest names to the most-used identifiers; we evidently do not.

jquerylil is the counter-case that makes this precise: it uses **all 54** one-char names and its
most-frequent-60 average is 2.65, *better* than Terser's 2.85 — and Terser still finds 1136 Brotli
there, so on that port the gain is elsewhere in the rename. micromark is where the allocation itself
is visibly worse.

## Ruled out: the obvious knob

`local_name_reserve` is not it. Sweeping micromarklil:

| reserve | 0 | 8 | **24 (default)** | 48 | 96 |
|---|---:|---:|---:|---:|---:|
| Brotli | 26573 | 26213 | **26097** | 26153 | 26229 |

The shipped value is already the optimum and every direction is worse. This is not a tuning problem.

## Entry criterion for the work

Frequency-ordered allocation across the whole artifact, and using the full 54-name alphabet before
going to two characters. It is a real change to a large subsystem — `LocalNames` and the alphabet
selection in `codegen_ir_js.rs` — with scope, export and reserved-name correctness to preserve, so it
wants its own hypothesis and its own measurement rather than being bolted onto this one.

The number to beat is in the table above: **−884 on micromark, −856 on mobx, −1136 on jquery**, and
those are what a competitor extracts from artifacts we have already finished optimising.
