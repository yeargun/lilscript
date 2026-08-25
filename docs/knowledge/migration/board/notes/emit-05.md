# emit-05 — shape repetition, and why converged naming does not yet reach it

Parent: [ledger](../LEDGER.md). Status: active.

## Question

LilScript emits **fewer** raw bytes than `jquery.min.js` and **more** compressed
bytes. Where does the compressed deficit come from, and can naming close it?

## Current hypothesis

Confirmed for the cause, **not yet** for the cure.

Cause: an LZ match costs about the same however long its text is, so one header
spelling repeated many times is cheaper than several shorter spellings.
Source-name-stable mangling binds the emitted letter to the *source variable's
identity*, so `(elem,key)` and `(key,elem)` become `(a,b)` and `(b,a)`. Terser
binds the letter to position and frequency *within the scope*, so headers
converge on `(e,t)`.

Cure attempted: assign every color from one canonical order (parameters by
position, then the rest by descending use). It moves the metric but not the
bytes, because each function's *pool* of available names still differs.

## Constraints specific to this task

The comparison was checked for fairness first: under jsdom the two builds expose
`jQuery.fn` 145 vs 145 and `jQuery.*` 94 vs 95 (the extra is `prototype`), and
the compat suite is 6/6. The gap is not extra surface on either side.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-25 | jQuery, ours vs official min | `lilscript-codec --json` | ours raw **85,456** / Brotli **29,770**; official raw 87,533 / Brotli 27,445 — 2,077 fewer raw bytes, 2,325 more compressed | gate |
| 2026-08-25 | module split via the slim builds both sides ship | same | slim core +1,220 Brotli (+5.6%); ajax+effects +1,105 (+19.7%) on **4,050 fewer** raw bytes | gate |
| 2026-08-25 | repeated 14-gram coverage | analysis script | ours 132% of file, official 206%; their top repeats are all `function(e){`, `function(e,t){`, `function(e,t,n` | diag |
| 2026-08-25 | multi-parameter arrow headers | analysis script | ours 217 headers / **69** distinct; the same code re-mangled by terser reaches **20**; official `function(…)` 518 / 36 with `(e)`x181 | diag |
| 2026-08-25 | terser `--mangle` alone on our own output | `ablate.mjs` | **−602 Brotli** while **+373 raw** — the win is unreachable by any raw-calibrated heuristic | diag |
| 2026-08-25 | converged naming, this change | `lilscript-codec --json` | distinct 69 → **41**, top-3 coverage 42% → **63%**, Brotli **−30** | gate |
| 2026-08-25 | scored as a beam family, six artifacts | same | **−1 Brotli total**; the search evaluates it and mostly declines | gate |
| 2026-08-25 | `local_name_reserve` 8/16/24/32 with converged naming | same | distinct stuck at 40–41 in every case — the reserve is not the blocker | gate |
| 2026-08-25 | `precise_cross_scope_shadowing` with converged naming | same | byte-identical output — not the blocker either | gate |

## Log

- 2026-08-25 — Root cause identified and measured. The metric moves (69 → 41 distinct, 42% → 63% top-3) but Brotli only −30, versus −602 for a full re-mangle. — **OPEN**
- 2026-08-25 — The residual spellings name the blocker: `(c,d)`x15, `(b,c)`x10, `(d,e)`x4, `(d,e,f)`x4, `(c,d,e,f)`x3. Those are the **same canonical order at a different starting offset**. `local_mangler` clones the top-level mangler and then *releases* a different set of top-level function names into each function's pool, so `next_name()` yields a different sequence per function. Canonical ordering cannot converge spellings while the pool itself diverges. — **OPEN**, and this is the next step
- 2026-08-25 — Kept the option and the family even though both are neutral: the option is the scaffold for the pool fix, and the family is what makes the choice calculated rather than assumed. If proposal-work pressure matters later, the family is the first thing to drop — see [search-04](search-04.md) on breadth displacing depth. — **LANDED**

- 2026-08-25 — **Uniform local pool: tried, measurably worse, reverted.** Stopped
  releasing top-level function names into each function's pool so every
  `next_name()` sequence would start at the same offset. jQuery went raw 85,439
  to **91,940** (+6,501) and Brotli 29,740 to **30,751** (+1,011), and distinct
  header spellings got *worse*, 41 to 49. Locals lose the recycled short names
  and the offsets do not converge anyway. — **REJECTED**
- 2026-08-25 — Second hypothesis, that `collect_top_level_references`
  over-approximates and reserves names a function cannot see: checked the code
  and it is false. The walk recurses only through `callee_body_nests_in_caller`
  callees and closures, which really are textually nested. The reservation set
  is correct, so the per-function divergence it produces is legitimate. —
  **REJECTED**
- 2026-08-25 — What the two negatives leave: terser reaches 20 distinct
  spellings for **+373 raw**, so convergence is achievable without surrendering
  short names. It manages that because it renames the **final laid-out text**
  with an exact scope chain, while we assign names per function before the
  nesting layout is known and must reserve conservatively. The gap is
  information, not policy — no ordering rule applied at our stage can recover
  it. — **OPEN**

## Next step

A scope-accurate renaming pass over the emitted text, at the peephole layer,
where the nesting is already resolved and the exact scope chain is available.
That is the only stage with the information terser has, and the measured prize
is 602 Brotli bytes on jQuery for 373 raw.

It needs the same thing the correctness lane needs: a real scope model in the
peephole. All three miscompiles in [ident-06](ident-06.md) and
[ident-08](ident-08.md) came from folds that could only ask "what token precedes
this?" rather than "what does this name resolve to?". One scope model pays for
both — it closes a live bug class and unlocks the largest measured naming win.
Build it once, for correctness, and take the compression as the second return.
