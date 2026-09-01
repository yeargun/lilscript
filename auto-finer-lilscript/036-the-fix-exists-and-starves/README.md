# 036 — The name-allocation fix already exists, and starves

**Status: THE TRANSFORM EXISTS, AND IT LOSES.** 035 concluded we needed scope-aware shadowing; it is
already implemented as `precise_cross_scope_shadowing`, already registered as a scored family, and
starves. Forcing it on costs **+432 Brotli**. 035's premise is falsified below.

## What 035 asked for

035 measured that micromarklil gives **62 of its 63 top-level bindings two-character names** where
Terser gives 53 of 58 one-character ones, traced it to module and local names being allocated from
disjoint pools, and concluded:

> **Scope-aware shadowing**: let a function's locals reuse a module binding's name when that binding
> is not referenced anywhere in the function. […] It is a real change to `LocalNames` and
> `assign_top_level_names`.

It is not a change to make. It is `IrJsOptions::precise_cross_scope_shadowing`, and the comment
sitting beside it in `codegen_ir_js.rs` describes the exact defect 035 spent a day measuring:

```rust
// Preferred local spellings are a separate namespace policy, not
// top-level allocations. A function may shadow a module binding
// it does not reference, while `local_mangler` keeps referenced
// bindings reserved. Consuming these names here forced the most
// frequent globals to two-byte spellings merely to make the same
// one-byte names available inside functions.
```

Under that flag, local reservations are drawn from a **separate** mangler and the one-character
alphabet is left for module bindings. Without it, they are consumed from `top_level_mangler` and
every module binding starts at two characters — which is precisely the 62-of-63 measurement.

## Why it never happens

The flag is deliberately `false` in the pinned emission, with a comment saying production search
scores the aggressive regime separately. It is registered exactly as that comment claims:

```rust
family!("precise-cross-scope-shadowing", EmissionPhase::BeforeEntropy, …)
```

`--explain json` on micromarklil, at its shipped config with `candidate_search = "always"`:

```
scored emission families : 47
starved emission families: 46
  precise-cross-scope-shadowing:  scored = true, starved = true
  frequency-order-local-names:    scored = true, starved = true
search stop reason        : work-budget-exhausted
terminal probe limit hit  : true
candidate proposal limit  : not reached
```

**Forty-six of forty-seven families starve.** The proposal budget is not the constraint — the
*terminal codec probe* budget is. The search generates the proposals and then cannot afford to
measure them, so the conservative default ships by forfeit.

## Why this matters beyond one flag

[018](../018-mobx-admission-regression/README.md) saw the same signature — `work-budget-exhausted`,
33 of 35 families starved — and could not attribute it, concluding budget starvation was *not* the
explanation because 20× the budget recovered only 280 bytes of 7546. That conclusion was right about
018's own question (the classes were being refused, not starved — see
[031](../031-admission-blocks-the-class-rewrite/README.md) and
[033](../033-member-bodies-are-scopes/README.md)) and it should not be read as clearing starvation
generally. Here starvation is the whole story for a different, measured 884 bytes.

The general shape is worth stating: **a scored family that never gets measured is indistinguishable
from a feature that does not exist.** Two hypotheses in this log went looking for missing transforms
that were already implemented and simply never reached the beam.

## FALSIFIED: the transform loses, and the current allocation is correct

Forcing `precise_cross_scope_shadowing = true` in the pinned emission and rebuilding micromarklil
does exactly what [035](../035-where-the-compiler-headroom-is/README.md) predicted structurally —
and the artifact gets **bigger**:

| | top-level 1-char | top-level 2-char | raw | Brotli |
|---|---:|---:|---:|---:|
| shipped (flag off) | **1** | 62 | 87117 | **26097** |
| flag forced on | **40** | 23 | 90710 | **26529** |
| | | | **+3593** | **+432** |

The transform works. Module bindings get their one-character names, exactly as the structural
comparison against Terser said they should. And it costs 432 Brotli, because those one-character
names come *out of the locals' pool*, and locals outnumber module bindings by an order of magnitude
in occurrences. Trading 62 module bindings up to one character to push thousands of local
occurrences down to two is a bad trade on this artifact.

**So 035's premise was wrong.** The 62-of-63 two-character top-level naming is not a defect; it is
the compiler already picking the better side of the trade. Terser's distribution is better *for
Terser's artifact*, and reading a structural difference as a defect was the error — the same mistake
[025](../025-brotli-repetition-gap/README.md) made with repeat coverage, and it should have been
caught the same way, by measuring before theorising.

This also explains cleanly why **every budget knob produced byte-identical output**:

| variant | Brotli | time |
|---|---:|---:|
| base | 26097 | 225s |
| `terminal_codec_probe_limit = 1536` | 26097 | 314s |
| `terminal_codec_probe_limit = 4096` | 26097 | 540s |
| `candidate_proposal_limit = 1536` too | 26097 | 315s |

The family was never the bottleneck. Even scored, its variant loses, so the incumbent survives and
the artifact does not move. "Starved" in `--explain` marks a family that did not get to run; it does
not promise the family would have won.

## What was tested

`terminal_codec_probe_limit` at 1536 and 4096 against micromarklil's default, plus a combined
variant. jquerylil is a useful control: it already sets 1536 by hand and still loses 1825, so budget
alone is not the whole answer everywhere.

The prize, from 035's measurements of what Terser extracts from our finished artifacts:
**−884 micromark, −856 mobx, −1136 jquery** — the last being 62% of that port's entire gap.
