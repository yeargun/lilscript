# jquery-01 — bottom-up on the port that loses to its own minifier

Parent: [ledger](../LEDGER.md). Status: active. Instrument: per-function
attribution keyed on string literals.

## Question

jQuery is the port with the largest competitive loss. Where exactly do the
compressed bytes go, function by function?

## Current hypothesis

Confirmed for two causes and refuted for two more. The single largest recovered
loss was not an emission defect at all: the beam never converged.

## Constraints specific to this task

The comparison was checked for fairness first. Under jsdom the two builds expose
`jQuery.fn` 145 vs 145 and `jQuery.*` 94 vs 95 (the extra is `prototype`), and
the compat suite is 6/6. The gap is not extra surface on either side.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-25 | module split via the slim builds both sides ship | `lilscript-codec --json` | slim core +1,220 Brotli (+5.6%); ajax+effects +1,105 (+19.7%) on **4,050 fewer** raw bytes | gate |
| 2026-08-25 | per-function attribution, own bytes only | `attrib2.py` | 40 matched groups, +3,362 drift; largest single function is `jQuery.ajax` at 2,392 own bytes | diag |
| 2026-08-25 | candidate search run to convergence | `candidate_search = "always"`, limits 1536 | Brotli-11 **29,770 → 28,895 (−875)**, compat 6/6, compile 40 s → 8 min | gate |
| 2026-08-25 | same on the shipped ESM | `node scripts/build.mjs --compile` | **29,881 → 29,011 (−870)** | gate |
| 2026-08-25 | `function_spelling` forced | `= "function"` / `= "arrow"` | 30,264 and 29,859 — both **worse** than the searched 29,770 | gate |
| 2026-08-25 | `Array.prototype.m.call(x,…)` rewritten to `x.m(…)` | textual, 42 sites | −873 raw, **−70 Brotli**, compat 6/6 | diag |

## Log

- 2026-08-25 — The gap is not spelling waste in the ordinary sense: we emit **fewer** raw bytes than `jquery.min.js` (85,456 against 87,533) and **more** compressed ones. Repeated-14-gram coverage is 132% of file against their 206%. — **OPEN**, and the mechanism is [emit-05](emit-05.md)
- 2026-08-25 — Running the beam to convergence recovers 875 Brotli bytes, more than every emission fix found this session combined. The gap against `jquery.min.js` narrows from +8.5% to +5.3%. Landed in the port config with the compile-time cost written down. — **LANDED**
- 2026-08-25 — Arrow versus `function` spelling: forcing either direction loses to what the search already picks per artifact. The `=>` 399 / `function(` 152 imbalance against their 0 / 518 is therefore **not** a defect. — **REJECTED** as a cause
- 2026-08-25 — 42 sites emit `Array.prototype.m.call(x,…)` where jQuery writes `x.m(…)`; official has zero. Worth −70 Brotli and −873 raw, and the compat suite passes the rewrite. But it is **not** free: `JS.shift(x)` on an arbitrary `JsValue` is not the same as `x.shift()` if `x` carries an own `shift`. The direct form already exists and is gated on provable array-ness (`codegen_ir_js` asserts it for `JS.array()` receivers). The fix is a wider array-ness proof, not a lowering switch. — **OPEN**
- 2026-08-25 — The same convergence config was tried on zodlil and **reverted**: `src/entry.lil` improved 56 bytes but the shipped `dist/zod.core.js` regressed 1,086. Per-port verification has to measure the artifact that ships, not a convenient entry. — **REJECTED** for zod

- 2026-08-25 — **The residual gap is IR-level control-flow shape, not spelling.**
  Normalized per 1,000 raw bytes, against `jquery.min.js`: `if(` **1.85x**,
  `else` **2.48x**, `;` **2.21x**, while ternaries run **0.89x** and `&&`/`||`
  about 0.88x. They convert statements into expressions and we do not, so their
  functions share long prefixes -- repeated 24-gram coverage is **81% of file
  against our 68%**, and their most repeated run is
  `function(e,t,n){var r,i,` fourteen times: header *plus* hoisted declarations
  in canonical order. — **OPEN**, and this is where the remaining 1,450 bytes live
- 2026-08-25 — Every attempt to emulate that shape **after** emission lost, which
  is the strongest evidence that the search already sits at the best point
  reachable from our IR. Measured on the converged jQuery artifact, Brotli:
  declaration hoisting **+277**; converged renaming **+454**; both together
  **+675**; Yoda canonicalization of `x==null` **+37** (our mixed form
  compresses better than official's uniform one); `comma_expressions = true`
  **+126**; `function_spelling` forced to `function` **+494** or `arrow` **+89**.
  Post-hoc rewriting is the wrong lever for this port. — **REJECTED**, all of it

- 2026-08-25 — **The beam is greedy over single flips and misses pairs.** On
  `callbacks.lil`, `function_spelling = "arrow"` alone scores 2,021 and
  `stable_local_names = false` alone scores 2,021 — both exactly the incumbent.
  Together they score **2,015**. A beam that extends one option at a time from
  the current finalist can never reach a pair whose members are individually
  neutral, however much budget it is given. Across six jQuery modules against a
  converged baseline the same pair is worth **−106 Brotli**, improving five and
  regressing one (effects +17). On the **full** build the same pair is worth
  only −11, because the whole-artifact search already recovers most of it from
  a richer context; the defect is real but its size shrinks with scope. —
  **OPEN**, and this is a search defect rather than an emission one
- 2026-08-25 — Per-module work needs its own baseline: the npm `jquery/src/`
  tree is **AMD**, so bundling a module with esbuild yields a 123-byte
  `define([…])` stub, not the module. Every "official per-module" number from
  that route is meaningless. What does work is ranking our own modules by the
  slack terser can still find, which came out uniform at about 3% — the gap is
  diffuse, not concentrated in a submodule. — **LANDED** as method
- 2026-08-25 — Terminal codec probes are **not** the constraint. jQuery reports
  `terminal work 384/384 (exhausted)` at level 15, but lifting the cap to 1,536
  buys **11 bytes** for six extra minutes of compile (7:41 to 13:37). The level
  tier that pins it is a deliberate invariant with a test, and it should stay.
  — **REJECTED**
- 2026-08-25 — Small modules do converge and win: `callbacks` at the deep
  setting scores 2,021 against terser-on-our-own-production-output at 2,044. The
  emitter is competitive once the search finishes; the residue is the
  control-flow shape recorded above. — **LANDED**

- 2026-08-25 — **Token census: we emit more operations, not longer ones.** Ours
  44,437 tokens against their 40,991 (+8.4%) while being 3,138 raw bytes
  *smaller* — 1.90 bytes per token against their 2.14. Broken down: assignments
  **2,860 against 1,658 (+72%)**, while member reads run 2,444 against 3,054 and
  calls 1,711 against 1,841. We store what they re-read. — **LANDED** as the
  measurement that reframes the gap
- 2026-08-25 — The compiler's own `LILSCRIPT_STORE_CENSUS` says why: stores are
  dominated by `cross_block` (16,632) and `unstable` (17,402), with the
  common-subexpression case only 6%. More branches make more blocks, more blocks
  make more values that cross them, and every crossing value needs a name. The
  assignment excess is a *consequence* of the branch excess, not an independent
  defect. — **LANDED**
- 2026-08-25 — `fold_returned_temporaries`: a value stored only to be returned
  on the next statement is returned directly. Built on
  [`BindingResolution`](../../../../src/js_peephole/binding.rs), so the store
  must *resolve* to the returned binding rather than merely spell like it — the
  first draft absorbed `f(y){let x=…;y=2;return x}` into `return 2,mutate()`,
  which the corpus caught and which is now a regression test. Earlier reads are
  allowed; a use after the store or a capture by any nested scope refuses the
  fold. Brotli: posthog packs −58, marked −184, zod −36, jQuery **+24**. — **LANDED**
- 2026-08-25 — Terser's compress on our converged jQuery output is worth −477
  Brotli and removes 120 of 374 `if`s, but the individual passes only sum to
  −245: `collapse_vars` −112, `conditionals` −84, `sequences` −31, `if_return`
  −18, `join_vars` +1. **The combination is worth about twice the sum of its
  parts**, which is the same combiner effect the beam cannot reach one flip at a
  time. — **OPEN**

## Next step

Two, in order.

**Shape the control flow in the optimizer, not the emitter.** We emit 1.85x
their `if` and 2.48x their `else` per byte. Every post-emission attempt to
recover that lost, so the branchiness is decided in the IR and has to be fixed
there: merging arms, sinking common tails, and choosing expression form before
the emitter ever sees a statement list. That is also the only lever left that
the candidate search cannot already reach, because the search chooses among
spellings of a fixed control-flow graph. Inventing source ternaries is
[arch-07](arch-07.md) (expression-if), not a peephole.

**Then widen the array-ness proof** so `JS.shift`/`JS.push`/`JS.slice` on a
value that provably holds an array lower to a direct call: 42 sites on jQuery,
worth −70 Brotli and −873 raw. The proof exists for `JS.array()` receivers and
does not survive a round trip through an object field.
