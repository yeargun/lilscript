# 001 — Where does compile time actually go?

**Status: CONFIRMED.** Measurement, not a change. Everything downstream depends on this number.

## Hypothesis

The complaint is "compilation takes infinitely long". Before changing any knob, establish *what*
consumes the wall clock. The prior is that the compressor-in-loop search dominates, because every
scored candidate triggers a **whole-artifact canonical Brotli q11 encode**
(`src/compiler.rs:compressed_size` -> `canonical_brotli_size`) plus a **whole-artifact
re-lex-and-validate** of the generated JavaScript
(`CodecBudget::measure_reserved` -> `js_peephole::analyze_generated_javascript`).

If true, compile time is not spread across the optimizer — it is concentrated in two O(artifact)
primitives called hundreds of times, and every effort-ladder decision should be made in terms of
"how many whole-artifact encodes did we buy".

## Instrument added

New module `src/timing.rs`, wired at exactly two sites:

- `src/compiler.rs` — `fn compressed_size` (every canonical gzip/Brotli encode)
- `src/js_peephole/mod.rs` — `fn analyze_generated_javascript` (every generated-JS re-analysis)

Both are zero-cost unless `LILSCRIPT_TIMING` is set (`timing::enabled()` is a cached `OnceLock`
probe; the `Scope` guard is only constructed when enabled). `src/main.rs` prints one JSON line to
stderr, so it can never contaminate JavaScript on stdout.

```
LILSCRIPT_TIMING=1 ./target/release/lilscript <input> --target js-module -o out.js
lilscript-timing {"wall_ms":...,"codec_cpu_ms":...,"codec_calls":...,"codec_mb":...,
                  "analyze_cpu_ms":...,"analyze_calls":...,"analyze_mb":...}
```

CPU columns are summed across Rayon workers, so they can exceed wall clock.

## Measurements (8-core host, `target/release`, warm)

### acorn port (40 KB `.lil` source, 26 KB emitted JS), `candidate_search = "production"`

| level | wall | codec CPU | codec calls | codec MB | analyze CPU | analyze calls | analyze MB | raw | gzip9 | brotli11 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 15 | 16.78 s | 15.08 s | 307 | 7.86 | 7.18 s | 1620 | 41.40 | 26290 | 3550 | **3069** |
| 13 | 12.96 s | 11.28 s | 276 | 7.06 | 5.44 s | 1257 | 31.97 | 25952 | 3575 | **3071** |

### jQuery port (219 KB `.lil` source, 93 KB emitted JS), shipped `lilscript.toml`

| level | wall | raw | gzip9 | brotli11 |
|---|---:|---:|---:|---:|
| 15 | **403.97 s** (6 min 44 s), peak RSS 319 MB | 92706 | 33713 | 30225 |

For reference the acorn port with `candidate_search = "off"` (its shipped setting) compiles in
**0.45 s** with **2** codec calls. The entire cost is the search.

## Findings

1. **The codec is the compiler.** On acorn L15, 15.1 s of the 16.8 s wall is inside Brotli.
   Per-call cost is `15078 ms / 307 = 49 ms` for a ~26 KB artifact — Brotli q11 runs at roughly
   0.5 MB/s. This is inherent to q11; it is not a LilScript inefficiency, it is a *call-count*
   problem.
2. **Generated-JS re-analysis is a real second tax.** 1620 calls over 41 MB for 7.2 s CPU
   (~4.4 ms/call). It is ~11x cheaper per byte than Brotli but is invoked **5.3x more often**,
   because `measure_reserved` re-validates the whole artifact on every measurement even when the
   same bytes were already validated.
3. **Level 15 buys almost nothing over level 13 on this corpus.** acorn: 3069 vs 3071 Brotli
   bytes — **2 bytes, 0.065%** — for **+29% wall clock**. This is direct evidence for the
   owner's instinct that 13 should be the default.
4. **Cost is superlinear in artifact size.** acorn (26 KB out) = 16.8 s; jQuery (93 KB out) =
   404 s. That is 3.6x the bytes for 24x the time. Per-encode cost grows with size *and* the
   search issues more encodes, so the product explodes. `gradual_artifact_work_limit` already
   tries to damp this and is evidently not damping enough.
5. **No memoization.** `compressed_size` is a pure function of `(bytes, model)` and there is no
   cache. The search re-emits many candidates that normalize to identical byte strings.

## Consequences / follow-up hypotheses

- **002** — make level 13 the default everywhere, and re-tune the ladder so 13 is genuinely the
  knee of the curve rather than "15 minus two feature flags".
- **003** — memoize `compressed_size` on a content hash. Free bytes, strictly fewer encodes.
- **004** — stop re-validating identical generated JavaScript in `measure_reserved`.
- **005** — two-tier scoring: screen candidates with a cheap proxy, exact-encode only survivors.
  This is the only change that attacks the 49 ms constant rather than the call count.
