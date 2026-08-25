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

## Next step

Widen the array-ness proof so `JS.shift`/`JS.push`/`JS.slice` on a value that
provably holds an array lower to a direct call. 42 sites on jQuery alone, and
the proof already exists for `JS.array()` receivers — it just does not survive a
round trip through an object field.
