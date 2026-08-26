# Compression goal — jQuery, and the objective behind it

## The goal

> LISTEN 5% is HUGE differnece.. please fix yeah keep going. dont stop until its
> realy smaller.. without breaking overal behaviuors.. you know, lilscript aims
> to be better compressing things vs vite/oxc/terser regardless of that js module
> we play with.. its an optimized language, special syntaxes, this that..
>
> Also dont forget, there mightbe some else problems, causing the drift. maybe
> tree shaking is broken, something else, .. i dont even know.. take your time.
> dont stop until lilscript version wins.
>
> never glue fixes. by design robust elegant

**Win condition:** the LilScript jQuery artifact compresses smaller than
`jquery.min.js` on Brotli-11, without breaking behaviour, without
library-specific hacks, and without glue.

Three standing rules that shape every decision below:

1. **No library-specific optimizations.** A change has to improve how LilScript
   compiles JavaScript generally. If a lever only pays on one port, the exits are
   to find the general principle behind it, improve the language, or make that
   port's `.lil` source explicit — not to reshape the compiler around one
   library.
2. **Judged on the configured codec.** Brotli-11 of the `cost_model = "brotli"`
   compile, measured with `lilscript-codec`. A raw win with a Brotli loss is not
   a win. jQuery must improve and the other ports must not regress.
3. **Bottom-up.** Find the submodules that lose and fix those; do not re-test a
   hypothesis by rebuilding the whole library. *(With one measured caveat — see
   "Method notes" below.)*

---

## Where it stands

| | raw | **Brotli-11** | vs `jquery.min.js` |
|---|---:|---:|---:|
| previously published | 94,289 | 30,741 | +12.01% |
| **published now** | 83,163 | **28,206** | **+2.77%** |
| official `jquery.min.js` | 87,533 | 27,445 | — |

**−2,535 Brotli (−8.2%).** Raw is 4,370 bytes *smaller* than the official build.
Against the three minifiers run on the same source, this now beats **esbuild
(28,442)** and trails **terser (27,435)** and **oxc (27,682)**.

**The goal is not met.** 761 bytes remain.

### Corpus, matched entries and configs

| port | before | after | Δ |
|---|---:|---:|---:|
| jquery | 28,889 | 28,156 | **−733** |
| zod | 31,894 | 31,001 | **−893** |
| mobx | 16,103 | 15,872 | −231 |
| marked | 9,497 | 9,474 | −23 |
| posthog | 5,781 | 5,779 | −2 |
| monaco | 34,181 | 34,181 | 0 |
| **total** | 126,345 | **124,439** | **−1,906** |

Nothing regressed. jQuery compat suite 6/6, all 145 `jQuery.fn` methods and 94
statics intact, nine behaviour probes matching real jQuery.

---

## What landed

Seven commits on `compression-objective-lane`.

**Two real bugs.**

- `c5a4a4a` — the compiler could **emit invalid JavaScript and exit zero**: a `:`
  answering no `?`, which every existing check missed because the parentheses
  balance. It reached `manipulation.lil` and a full jQuery build.
  `validate_conditional_operators` closes the class; checked against 176
  artifacts across every port with zero false positives.
- `f369698` / `70cedc7` — `terminal_codec_probe_limit` and
  `candidate_proposal_limit` were silently clamped to the optimization level's
  tier, so a config asking for 1536 received 384. The terminal one was worth −33;
  the proposal one is worth zero bytes but a silently ignored config value is
  worse than an honored unused one.

**Two levers that paid, both from the same instinct: a longer form that repeats
can beat a shorter one.**

- `6b2c360` + `b52d7b1` — **name convergence.** Terser remangling our artifact
  was **+300 raw and −459 Brotli**; isolating it showed its frequency-sorted
  alphabet was worth only **15**, and the whole effect was *which name goes
  where*. Two things were wrong with our pass: it converged onto `abcdef…` in
  files whose own identifiers are `e,t,n,r`, and it decided availability by "does
  this spelling appear in the scope" rather than "would it capture" — so a nested
  function binding `e` for itself blocked `e` for its parent. jQuery went from
  **68 header spellings to 27** (terser reaches 26), and our mangling now **beats
  terser's**: remangling our current output makes it +132 worse.
- `732cf08` — **object-method spelling.** `k(){…}` is shorter than
  `k:function(){…}` and the emitter always chose it. Turning it off is **−404 raw
  *and* −94 Brotli** — better on both axes at once, which pure length reasoning
  does not predict.

**Three value-placement folds** (`b293406`): a function bound once and read once
moves to its use site; `()=>{q();return v}` becomes `()=>(q(),v)`;
`fold_returned_temporaries` now sees the last declarator of a `var` list.

---

## Method notes worth keeping

**Submodule magnitudes do not transfer.** Core reports **+5** for the exact change
the whole artifact reports **−94** for. Bottom-up is the right way to *localize* a
problem; it is not a way to size a fix. Every final decision was measured on the
full artifact, at roughly ten minutes a build.

**Run the heroes on our own output.** Comparing our artifact to `jquery.min.js`
compares two programs that were never the same program. Running terser over
*our* output holds the program and the dependency closure fixed, which is what
makes a number attributable to the compiler rather than to the port.

**Two corrections made to earlier conclusions in this work**, both because a
later measurement contradicted them:

- `deferred` and `queue` were called "worse programs than jQuery's, so this is
  `.lil` source work". Measured as programs they are not: 37 top-level functions
  against 39, 6,462 bytes in functions against 6,657, and **ours is the smaller
  file**. Their residual is the same diffuse compressibility difference the whole
  artifact shows.
- Moving the body fold into the unconditional pipeline was justified as "it only
  removes syntax, so it needs no scoring". marked said **+37**, corpus net +11,
  and it was reverted. Shorter is not smaller, for simplifications too.

---

## The remaining 761 bytes

The local half of the gap is **gone**. At a 1KB Brotli window the deficit fell
from 870 to **143**; what is left is mid-range repetition.

| window | gap |
|---|---:|
| 1KB | 143 |
| 4KB | 669 |
| 16KB | 1,023 |
| 64KB+ | 761 |

Compressibility is 2.949 bytes per compressed byte against the official build's
3.189. Terser can still find **−345** in our output, and that is the whole
identified budget.

### Everything measured and rejected

All on the full artifact, Brotli. These are recorded so they are not retried.

| hypothesis | Δ raw | Δ Brotli |
|---|---:|---:|
| assignment sinking, all three populations (192 sites) | +100 | **+64** |
| assignment sinking, statement-to-statement only (26 sites) | 0 | −3 |
| batching same-receiver assigns into `Object.assign` (min 3/5/8/12) | | **+116 / +53 / +13 / +8** |
| hoist declarations to one leading `var` | −19 | **+279** |
| `function_spelling = "function"` (global) | +7,089 | **+547** |
| arrows → `function` in property position | +96 | +5 |
| top-level arrow declarators → `function` declarations | +651 | +55 |
| disable structured-closure inlining | −141 | +295 |
| property mangling off | −335 | +24 |
| `local_name_reserve` 0 / 48 | | +216 / +36 |
| rematerialize single-use pure paths | −36 | +83 |
| wider candidate proposal budget (384 → 1536) | 0 | **0** (byte-identical) |
| reprint / formatting normalization only | −107 | +3 |

Two of these are worth understanding rather than just recording:

- **`collapse_vars` is not sinking.** Reading its diff rather than its name, its
  −512 raw is **block-brace removal** and redundant parentheses. Reprinting alone
  is +3 Brotli, so the codec does not pay for that raw.
- **Our assignment sequences beat jQuery's `extend({…})` literals.** We emit 364
  member-assign statements to the official build's 140, and `.extend(` 11 times to
  its 47 — but converting ours to object literals loses at every threshold,
  because the repeated `O.` prefix is nearly free under LZ while
  `Object.assign(O,{` is 17 novel characters per run.

---

## Future direction

Ordered by what the measurements actually support.

1. **Do not chase the remaining transforms individually.** `collapse_vars` −75,
   `join_vars` −65, `conditionals` −70, `if_return` −55, `sequences` −50,
   `unused` −44 — six items that interact, and every attempt to isolate one into
   an implementable fold has produced a loss. Terser's −345 is emergent from its
   whole pipeline. If this is attempted, it has to be as one scored bundle, not
   six separately-landed passes.

2. **The open question is compressibility, not size.** We are 4,370 raw bytes
   smaller and 761 compressed bytes larger. Everything that closed distance this
   session did so by *increasing or holding* raw while improving repetition —
   naming convergence (+300 raw for −459) and method spelling (−404 raw for −94)
   both. The next real lever is likely another whole-artifact uniformity policy,
   found the same way: look for a hardcoded "compact is better" choice the search
   never scores. `src/config.rs` still has 21 emission policies the candidate
   search never flips; `struct_method_shorthand` was one of them and it was worth
   −94.

3. **The language gap that is real but unexploited.** LilScript has no ternary;
   `?` is only the nullable-type suffix. A conditional value must be written as
   an `if` statement, and the port has **zero** `match` expressions — the
   language's conditional-expression form — across 323KB of source. That is why
   `local_phi_expression_regions` exists at all. Forcing it on is −87 on jQuery
   and +503 across the other five ports, so it is exposed as configuration rather
   than flipped; it also exposed the malformed-ternary miscompile, which is now
   caught.

4. **Do not rewrite `deferred.lil` / `queue.lil`.** See the correction above: the
   programs are already equivalent to jQuery's and smaller.

### Where the numbers live

`docs/knowledge/migration/board/notes/emit-07.md` carries the full evidence
tables, the isolated per-transform budget, and the corrections. The board is the
entry point: read it before the code.
