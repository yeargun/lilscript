# 029 — Specialisation is not what costs us the repetition class

**Status: HYPOTHESIS FALSIFIED. Every lever that reduces cloning makes the artifact bigger.**

## The hypothesis

Two ports lose Brotli while emitting **fewer** raw bytes than the competitor:

| | raw | Brotli | ≥32B repeat coverage |
|---|---:|---:|---:|
| remark-mathlil | 6376 (−1.0%) | 2287 | 5.7% |
| its Terser baseline | 6442 | **2150** | **10.5%** |
| jquerylil | 83044 (−5.1%) | 28225 | 4.2% |
| jquery.min.js | 87533 | **27445** | **5.1%** |

[025](../025-brotli-repetition-gap/README.md) measured the mechanism: roughly half the long-range
repetition. The natural suspect is **specialisation** — cloning a callee per constant argument or per
call site shortens each site by a few bytes and destroys the long identical spans a compressor was
charging almost nothing for. If that were it, telling the compiler to keep one generic callee would
trade raw bytes for Brotli bytes and win.

## Tested, on the port where a build costs 12 seconds

| `[optimization]` setting | Brotli | vs base |
|---|---:|---:|
| **base** | **2287** | — |
| `constant_parameter_specialization = false` | 2287 | 0 |
| `capture_signature_cloning = false` | 2287 | 0 |
| `identical_function_folding = true` | 2287 | 0 |
| `function_subsumption = true` | 2287 | 0 |
| both of the above | 2287 | 0 |
| `call_site_specialization = false` | 2332 | **+45** |
| all three cloning switches off | 2335 | **+48** |
| `inline_closure_factories = false` | 2315 | +28 |

**Nothing helps.** The folding switches are already on, so setting them is a no-op; every switch that
actually suppresses cloning makes the artifact *larger*. Turning off call-site specialisation costs
45 bytes, and turning off all three costs 48.

That is the opposite of the prediction. Specialisation is not buying its raw savings at the
compressor's expense here — it is paying for itself twice, and the repetition deficit comes from
somewhere else.

## Also falsified on jquerylil, separately

Same class, and two documented levers from `config.rs` doc comments:

| variant (level 13) | Brotli |
|---|---:|
| level 13 base | 30672 |
| `+ region-outlining` | 30672 (**no effect**) |
| `+ local_phi_expression_regions = true` | 30667 (**−5**, not the recorded −87) |
| both | 30667 |

`region-outlining` — the repeated-region outliner, the one pass named for this exact problem, and the
one pass jquerylil's explicit `compression` allowlist omits — changes **nothing** when added. The
transform was verified to apply before the run.

Also settled while there: **level 13 is 1402 bytes worse than 15 on jquerylil** (30672 against
29270), so this port's level-15 config is correct, and ESM-vs-UMD is worth only 70 bytes, so the
comparison against `jquery.min.js` is fair rather than a format artifact.

## Where that leaves the repetition class

Every configuration surface is now exhausted for these two ports: search effort, beam width,
proposal limits, spelling knobs ([027](../027-tuning-is-exhausted/README.md)), and now the whole
cloning and folding family. The deficit is in how the emitter *spells* congruent code, and no knob
that exists reaches it.

The lever [025](../025-brotli-repetition-gap/README.md) proposed is still the only candidate, and it
is a new pass rather than a setting: when two emitted fragments are structurally congruent, prefer
the identical spelling for both — same local names in the same order — even where a shorter local
spelling exists, and let `cost_model = "brotli"` adjudicate. Nothing in the pipeline proposes that
today, which is why the compressor-in-loop search never sees it.
