# 011 — String pooling under a compressing objective

**Status: CONFIRMED as a modelling flaw. Config win taken; the compiler fix is specified, not landed.**

## Hypothesis

[008](../008-jquery-compressibility-gap/README.md) found that disabling string pooling on jQueryLil
*saves* Brotli bytes. [010](../010-string-pool-alias-pricing/README.md) fixed one arithmetic error in
the admission test and recovered 3 of them. This asks the remaining question: **is the pooling
savings model right at all under gzip or Brotli?**

The model (`assign_string_aliases`, `src/codegen_ir_js.rs`):

```
unaliased = count * literal_length          // every use spells the whole literal
aliased   = literal_length + name + 2 + name * count
```

`unaliased` credits **every repeated occurrence at full literal width**. That is correct for a raw
objective. Under gzip or Brotli it is not: the second and subsequent occurrences of a literal were
already LZ matches costing a handful of bits, so pooling does not save what the model says it saves —
and it spends real bytes on a declaration plus a fresh identifier token that has no match history.

The admission threshold *is* already objective-scaled (`src/config.rs` — Raw 1, Gzip 4, Brotli 8), so
the question was whether Brotli's 8 is simply too low.

## Measurement — jQuery port, level 13, Brotli objective

| `string_pool_minimum_savings` | raw | gzip-9 | Brotli-11 |
|---|---:|---:|---:|
| 8 (current default) | 89861 | 34160 | 30590 |
| 16 | 89879 | 34162 | 30598 |
| 32 | 89973 | 34151 | 30606 |
| 64 | 89953 | 34132 | **30555** |
| pooling disabled entirely | 89953 | 34132 | **30555** |

## Findings

1. **Raising the threshold is not the fix.** The curve is **non-monotone**: 16 and 32 are *worse*
   than 8, and 64 is identical to disabling pooling outright (no jQuery literal clears a 64-byte
   raw-savings bar). A knob whose middle values are worse than both ends is not a cost dial — it is
   perturbing which candidates the beam explores. Tuning the constant would be fitting noise on one
   artifact, so it was not done.
2. **"Pooling off" is the honest optimum for this port**: −35 Brotli, −28 gzip, +92 raw. Since
   jQueryLil declares `cost_model = "brotli"`, that is a straight win and it was taken as a config
   change (`string-pooling` removed from the port's `compression` list) rather than a compiler
   change.
3. **The modelling flaw is specific and fixable.** Under a compressing objective the benefit of
   pooling is bounded by what the compressor could *not* already match — roughly the first
   occurrence — not by `count * literal_length`. Every candidate is scored as if the compressor did
   not exist.

## Recommended compiler fix (specified, not landed)

Make the *benefit* term objective-aware rather than the threshold:

- `Raw`: keep `count * literal_length`. The per-use saving is real.
- `Gzip` / `Brotli`: credit roughly one full-width occurrence plus a small per-repeat match cost,
  so that a short literal repeated many times scores near zero rather than near `count * width`.

This was deliberately not landed in this pass. The measurement above shows the surrounding search is
sensitive enough that a formula change needs validating across several ports before it can be
trusted, and every jQuery-scale datapoint is minutes of compile time on a contended host. The two
changes that *were* made here — [010](../010-string-pool-alias-pricing/README.md)'s alias-width fix
and this port config — are both locally verifiable.

## Verification

`string_pool_minimum_savings` is left at Raw 1 / Gzip 4 / Brotli 8, with a comment at the site
recording why raising it is not the answer, so the next person does not re-run this sweep.
