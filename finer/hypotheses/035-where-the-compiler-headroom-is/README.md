# 035 — Where the compiler-side headroom actually is, measured

**Status: MEASURED AND DIAGNOSED, NOT YET ACTED ON.** The largest lead is name allocation, and the
mechanism is now located: we allocate module and local names from disjoint pools, so 62 of 63
top-level bindings get two-character names where Terser gives 53 of 58 one-character ones.

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

## The mechanism, located

Reading the allocator settles what the statistics only hinted at. Top-level names **are** already
assigned in descending use order — `assign_top_level_names` builds a `bindings` vector carrying each
binding's use count and sorts it `right.0.cmp(&left.0)` before handing out names. Frequency ordering
is not missing.

Two wrong guesses, both cheap to kill:

- *"Locals overflow the reserve and spill to two characters."* **No.** The deepest function in
  micromarklil declares 23 locals, the median declares 1, and nothing exceeds the 24-name reserve on
  any port measured.
- *"`local_name_reserve` is mistuned."* **No** — swept, and the shipped 24 is the optimum.

The actual mechanism is the pool split. Counting names declared at brace depth zero:

| micromark | top-level bindings | one-char | two-char |
|---|---:|---:|---:|
| ours | 63 | **1** | **62** |
| Terser | 58 | **53** | 5 |

**We give essentially every top-level binding a two-character name.** The reserve, the synthesized
runtime roots, the class identities, the import aliases and the adapter factories consume the
one-character alphabet before module bindings are reached, so they start at the two-character names.

Terser does not have this problem because it **lets a function's locals shadow module bindings**. It
can spend all 54 one-character names on module bindings *and* spend the same 54 again on locals in
every function, because a local `a` inside a function that never mentions the module's `a` is
unambiguous. We allocate the two from disjoint pools, so with 54 one-character names available we
can give them to locals or to module bindings but not both — and the sweep result now makes sense:
every `local_name_reserve` setting is just a different point on that trade, and 24 is where it
balances.

## Entry criterion for the work

**Scope-aware shadowing**: let a function's locals reuse a module binding's name when that binding is
not referenced anywhere in the function. That removes the disjoint-pool constraint and is what buys
Terser both columns of the table above. It is a real change to `LocalNames` and
`assign_top_level_names` in `codegen_ir_js.rs`, and it has to prove it never captures — the existing
reserve exists precisely to make capture impossible by construction, so replacing it means replacing
that guarantee with an analysis. That wants its own hypothesis and its own measurement.

The number to beat is in the table above: **−884 on micromark, −856 on mobx, −1136 on jquery**, and
those are what a competitor extracts from artifacts we have already finished optimising.
