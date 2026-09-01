# 025 — We lose where we emit more code, not where we compress worse

**Status: ROOT CAUSE MEASURED. My first hypothesis here was wrong and is kept below, falsified, with
the number that killed it.**

## What I set out to test

`remark-mathlil` looked like proof that our *compression* was the problem, because size was not:

| | raw | Brotli | ratio |
|---|---:|---:|---:|
| LilScript | **6376** | 2287 | 2.79× |
| official + Terser | 6442 | **2150** | 3.03× |

Fewer bytes, 137 more after Brotli. No knob fixes it — `function_spelling`, `local_name_reserve`,
`pool_strings` and property mangling move it by at most −4, and three of five make it worse. And the
cause there is real and measurable. It is not identifier reuse, where we are already **better** than
Terser (8.60× vs 7.43× token reuse). It is long-range phrase repetition:

| match length | LilScript | official + Terser |
|---|---:|---:|
| ≥ 8 bytes | 48.2% | 55.6% |
| ≥ 16 bytes | 26.1% | 30.1% |
| **≥ 32 bytes** | **5.7%** | **10.5%** |

Terser leaves structurally identical functions spelled identically; we inline and specialize each
site, deleting a few raw bytes and destroying a 32-byte repeat worth more.

## The generalization was false

I predicted this explained the losing ports. It does not. Across the 15 comparable ports
(`rehype-katex` excluded — [023](../023-unparseable-class-expressions/README.md) explains why):

```
corr(repeat-coverage gap >= 32B, relative Brotli delta) = +0.134
```

**No predictive power, and the sign is backwards from the theory.** `hast-util-to-html` and
`rehype-stringify` have *more* long-repeat coverage than Terser and win; `rehype` has 15.9 points
*less* and wins anyway. remark-math is a real specimen of a real effect, and a single specimen.

## What actually predicts it

Raw emitted volume:

```
corr(raw excess %, Brotli excess %) = +0.940   over 15 ports
```

| port | lil raw | official raw | raw % | Brotli % |
|---|---:|---:|---:|---:|
| mdast-util-to-hast | 14243 | 17117 | −16.8 | **−15.0** |
| remark-rehype | 13669 | 17263 | −20.8 | **−13.6** |
| hast-util-to-html | 30291 | 31882 | −5.0 | **−10.3** |
| rehype-stringify | 30604 | 31975 | −4.3 | **−8.0** |
| remark-breaks | 2742 | 3045 | −10.0 | **−5.8** |
| remark-gfm | 33559 | 42343 | −20.7 | **−3.4** |
| rehype | 192557 | 221625 | −13.1 | **−3.1** |
| remark-math | 6376 | 6442 | −1.0 | +6.4 |
| katex | 293173 | 267745 | +9.5 | +9.2 |
| mdast-util-from-markdown | 92865 | 84681 | +9.7 | +13.6 |
| remark-parse | 94285 | 84866 | +11.1 | +13.9 |
| micromark | 100212 | 81530 | +22.9 | +18.2 |
| unified | 20869 | 13579 | +53.7 | +18.3 |
| remark | 183772 | 119872 | +53.3 | +31.9 |
| react-markdown | 218159 | 117759 | +85.3 | +59.7 |

**Every win emits less JavaScript than Terser; every loss emits more.** remark-math is the sole
exception in either direction — the one port where the repetition effect decides the outcome.

Our compression *ratios* are competitive or better (micromark 3.72× vs 3.58×, remark 4.28× vs 3.68×,
react-markdown 4.39× vs 3.79×). We are not compressing badly. **We are emitting 9.5% to 85% more
code than the library needs**, and Brotli faithfully reports it.

## Naming is a contributor, not the driver

The obvious follow-up is *why* we emit more. Counting distinct identifiers in each artifact splits
the losers in two:

| port | unique ids (lil / official) | excess | raw % | Brotli % |
|---|---|---:|---:|---:|
| react-markdown | 5412 / 1788 | **+203%** | +85.3 | +59.7 |
| rehype | 7764 / 3322 | **+134%** | −13.1 | **−3.1 (win)** |
| micromark | 4120 / 2589 | +59% | +22.9 | +18.2 |
| remark | 4603 / 2915 | +58% | +53.3 | +31.9 |
| unified | 312 / 256 | +22% | +53.7 | +18.3 |
| katex | 2822 / 2744 | **+3%** | +9.5 | +9.2 |
| remark-parse | 2660 / 2602 | **+2%** | +11.1 | +13.9 |
| mdast-util-from-markdown | 2658 / 2599 | **+2%** | +9.7 | +13.6 |

```
corr(unique-identifier excess %, Brotli excess %) = +0.693
```

Weaker than raw volume's +0.940, and `rehype` is a flat counterexample: 134% more distinct names and
it still wins. Function *counts* track the official closely everywhere (micromark +6%, remark +12%,
react-markdown +13%) — we are not emitting more functions, we are emitting bigger ones with more
names in them.

So the losers are at least two populations:

- **Volume without naming excess** — katex, remark-parse, mdast-util-from-markdown all sit within 3%
  of the official's distinct-name count and still emit 9–11% more bytes. Nothing here is a mangling
  failure; the code itself is larger.
- **Volume with naming excess** — micromark, remark, react-markdown, unified. Every port in the fleet
  declares the *same* `[mangle]` block (`identifiers`, `properties`, `pool_strings` on;
  `exports` off), so this is not configuration: it is names the mangler is not permitted to touch,
  which is exactly what [021](../021-reflective-ffi-predicts-loss/README.md) measured as reflective
  host-FFI density.

## What this changes

Compressor-side tuning is the wrong place to look for the big losses, and this session's knob
sweeps confirm it from the other direction: `candidate_search`, `candidate_beam_width` and
`optimization_level` bought 68 bytes on posthog and 663 on unified, but nothing on the ports that are
50–85% oversized. Those need **less emitted code**, which is a lowering/FFI problem, not a search
problem — and it is the same set [021](../021-reflective-ffi-predicts-loss/README.md) reached from
reflective host-FFI density, now with a much stronger statistic behind it.

Tools: `auto-finer-lilscript/repeat-coverage.mjs` (the falsified metric — still the right instrument
for the remark-math class) and the raw/Brotli columns of any harness run for the one that predicts.
