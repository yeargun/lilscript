# 018 — A second admission regression, 7940 raw bytes on mobxlil

**Status: BISECTED to a file and a commit range, four mechanisms falsified, reported not patched.**

## Subject

`mobxlil` is the best remaining bisect subject in the fleet: **zero modified source, zero modified
config**, and it compiles in **59 seconds** — against jQueryLil's 30–60 minutes. Its committed
artifact is a straight compiler comparison.

| `mobxlil/dist/mobx.esm.js` | raw | gzip-9 | Brotli-11 |
|---|---:|---:|---:|
| committed | 57005 | 17337 | 15594 |
| built with the current compiler | 64945 | 18496 | 16514 |
| **regression** | **+7940 (14%)** | +1159 | **+920** |

## Bisect

107 commits, predicate = compile `src/mobx.lil` with its own config and measure raw:

```
probe 8f82ff6 -> 58117     probe fbd2b3b -> 64945
probe 42c1ad0 -> 57399     probe edbdf3a -> 64945
```

Seven revisions in the middle **could not be probed** — they either fail to build or exceed the
900-second compile timeout, which is itself a finding about that stretch of history. So the result is
a range rather than a commit:

**good `42c1ad0` → bad `edbdf3a`**, the 08-29 21:21–22:11 "admission" cluster: *Pin partial artifact
admission*, *Validate exported callable ABI*, *Pin callable ABI admission*, *Validate callable and
property witnesses*, *Pin property admission candidate*, *Admit typed candidates before scoring*,
*Pin pre-score admission candidate*, *Carry admission through terminal selection*.

Reverting files individually across that range:

| file reverted to `42c1ad0` | mobxlil raw |
|---|---:|
| **`src/compiler.rs`** | **57399 — recovered** |
| `src/js_peephole/mod.rs` | does not build alone |

`src/js_peephole/mod.rs` gained 433 lines in the range but they are **all analysis helpers** —
`generated_javascript_export_witnesses`, `generated_javascript_static_property_names`,
`generated_class_shape` and friends. No new folds, no pipeline change. The behavior change is in
`compiler.rs`'s wiring of those helpers into validation.

## Five mechanisms falsified

The obvious story is that validation rejects good candidates and the search settles for worse ones.
It is not what happens.

0. **Budget starvation** — 20x the search work recovers 280 bytes of 7546 and zero classes. Detailed
   below, because it was the most plausible mechanism and disproving it is what made the real
   statement possible.
1. **Admission rejecting candidates.** Instrumented: **150 validations, 0 rejections** on mobxlil.
   (markedlil: 201 validations, 0 rejections.)
2. **Direct-artifact validation dropping whole plans** before admission is ever reached — this would
   be invisible to the counter above, so it got its own bucket (`timing::DIRECT_VALIDATE`):
   **11 calls, 0 failures.**
3. **The property-introduction check.** `validate_observed_javascript_artifact` rejects any candidate
   whose static property names are not a subset of the direct emission's, which would plausibly
   forbid `o["a"]` → `o.a`. Disabling it outright leaves mobxlil at **64945** — unchanged.
4. **The callable-ABI expectation source**, fixed in commit `41b88f2` for markedlil. mobxlil is
   unmoved by it, so this is a *second, distinct* instance in the same family.

**Nothing is being rejected anywhere, and the output is still 7940 bytes bigger.** Whatever the
mechanism is, it changes which candidates are *generated or scored*, not which are *refused*.

## What the bytes actually are: `class` emission is gone

Structurally diffing the recovered artifact against the current one is what finally located it:

| | good (57399) | bad (64945) | delta |
|---|---:|---:|---:|
| **`class`** | **10** | **0** | **−10** |
| **`.prototype`** | 19 | **120** | **+101** |
| `function` keyword | 206 | 310 | +104 |
| `=function` | 114 | 200 | +86 |
| `Object.setPrototypeOf` | 0 | 2 | +2 |
| `;` | 952 | 2196 | +1244 |
| `,` | 2194 | 1338 | −856 |
| identifier occurrences | 11155 | 12631 | +1476 |

**The good build emits ten `class` declarations; the current one emits none**, falling back to
`function` + `.prototype` assignment tables and `Object.setPrototypeOf` for inheritance. That is the
entire 7.5 KB: a class body is dramatically more compact than the constructor-plus-prototype-table
spelling it replaced.

This is the Closure-ADVANCED-style structural choice the objective names — and it is being lost, not
by a rejection, but by never being scored.

## Starvation looked like the answer, and is not

`--explain json` on mobxlil reports `work-budget-exhausted` with **33 of 35 families starved** — 94%,
against 51–62% on acorn ([009](../009-search-starvation/README.md)). The obvious reading is that the
class-emission variant is simply one of the families that never runs, and that the admission work
pushed an already-thin budget past it.

**Tested, and false.** Re-running with `terminal_codec_probe_limit = 4096`,
`candidate_proposal_limit = 1536` and a 32 MiB candidate byte budget — roughly twenty times the
default work, and twenty minutes of compile against fifty-nine seconds:

| | raw | `class` declarations |
|---|---:|---:|
| default budget | 64945 | 0 |
| **20x budget** | **64665** | **0** |
| pre-regression | 57399 | **10** |

280 bytes of 7546, and **still not one class**. The class form is not being out-competed and it is not
being starved: it is **no longer in the candidate space at all**.

That is the sharpest statement available and it reconciles every falsified mechanism above. Nothing
is refused, nothing is out-scored, and no amount of budget recovers it — so something in that
`compiler.rs` range stopped the class-shaped emission from being *generated*.

Two further notes from the same telemetry, both worth acting on independently:

- mobxlil declares `priority = "realistic-performance-first"`, not `size-first`. Some of its distance
  from upstream is a deliberate performance choice, not a compiler failure, and its −3577 should not
  be read as a pure size loss.
- only **35** families are scored here against acorn's 45, so this port is starting from a narrower
  search before starvation is even counted.

## It is not only classes — a whole family of spellings reverts

Comparing the same function in both artifacts shows the loss is broad, not a single fold:

```js
// good
Nb=a=>{if(0!=(a.dependenciesState_|0)){...  while(b>0)b--,a=c[b],a.lowestObserverState_=0}}

// bad
let Nb=a=>{if(0==(+a.dependenciesState_|0))return;...  while(e>0){e=e-1|0;a=c[e];a.lowestObserverState_=0}}
```

Four independent size decisions revert together:

| decision | good | bad |
|---|---|---|
| declarator chaining | `Nb=…,ab=…` under one keyword | `let Nb=…; let ab=…` |
| integer-coercion elision | `a.dependenciesState_\|0` | `+a.dependenciesState_\|0` |
| update operators | `b--` | `e=e-1\|0` |
| comma statement bodies | `while(b>0)b--,a=c[b],…` | `while(e>0){e=e-1\|0;a=c[e];…}` |
| class spelling | `class na{constructor(b){…}}` | `function` + `.prototype` + `setPrototypeOf` |

Several unrelated decisions all falling back to their plain form at once is the signature of the
**selection landing on the baseline emission** rather than on an optimized candidate — not of any one
fold breaking.

The search telemetry is consistent with that: `optimizer_emissions_attempted` is **4**,
`candidates_evaluated` **13**, and both the proposal and probe limits are **96** and both reached.
96 is what `gradual_artifact_work_limit` scales 384 down to for a 65 KB artifact.

But raising those limits twentyfold recovers 280 bytes of 7546 and zero classes, so a thin budget is
the *condition*, not the *cause*. The optimized spellings are not losing a scoring contest; they are
not being produced.

## Reproduction

59 seconds per probe, frozen source:

```sh
git worktree add /tmp/mb edbdf3a && cd /tmp/mb && cargo build --release
./target/release/lilscript ~/mobxlil/src/mobx.lil --target js-module \
  --config ~/mobxlil/lilscript.toml -o /tmp/bad.js        # 64945
git checkout 42c1ad0 -- src/compiler.rs && cargo build --release
./target/release/lilscript ~/mobxlil/src/mobx.lil --target js-module \
  --config ~/mobxlil/lilscript.toml -o /tmp/good.js       # 57399
```

## Why this is a report

That range is the owner's own in-flight admission work, and its stated purpose — proving the emitted
ABI matches the contract — is one this workstream has no standing to undo. `41b88f2` fixed the one
instance whose intent was legible from the code comment. This one is not: nothing rejects, so the
change is doing something other than what its commit messages describe, and guessing at a 342-line
refactor's intent is how a size fix becomes a correctness bug.

**The prize is large and cheap to verify: 7940 raw and 920 Brotli on one port, one minute per test.**
Combined with [016](../016-marked-size-regression/README.md)'s 1568 bytes on markedlil, the
admission work across 2026-08-29 is the single largest identified source of avoidable bytes in the
fleet.
