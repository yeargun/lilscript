# 034 — The bundler was escaping 2304 characters into six bytes each

**Status: FIXED on micromarklil and rehypelil. −27689 raw and −1093 Brotli across the two, from one
line of build configuration. Largest saving in this log.**

## How it surfaced

Terser run over our own output with **`compress: false, mangle: false`** — a pure reprint, no
transform at all — came back 7021 raw bytes smaller on micromarklil. On every other port the same
reprint moved tens of bytes:

| port | reprint delta (raw) |
|---|---:|
| **micromark** | **−7021** |
| jquery | −93 |
| mobx | −50 |
| marked | −36 |
| remark-parse | −24 |

A reprint that changes nothing but formatting cannot legitimately save 7.5% of an artifact, so
something in micromark's bytes was not what the compiler wrote.

## It was not ours

Measuring the compiler's own output against the shipped bundle separates them cleanly:

| | reprint slack |
|---|---:|
| `dist/micromark.raw.js` — what the compiler writes | **−48** |
| `dist/micromark.esm.js` — after the port's esbuild bundle | **−7021** |

Our emitter is tight. The bundle is not.

## What it was

```
                          \uXXXX    \xXX   literal non-ASCII
dist/micromark.raw.js          1       0                2304
dist/micromark.esm.js       1924     113                   0
```

The compiler emits 2304 non-ASCII characters **literally** — two or three UTF-8 bytes each. esbuild
defaults to an ASCII-safe charset and re-prints every one of them as a `\uXXXX` or `\xXX` escape:
**six ASCII bytes**. 2304 characters × roughly three bytes of overhead is the 7021.

The fix is `charset: "utf8"` on the esbuild lanes:

| | raw | Brotli |
|---|---:|---:|
| before | 94138 | 26314 |
| after | **87117** | **26097** |
| | **−7021** | **−217** |

`1963/1963` tests pass. micromarklil ends at **+3321** against its Terser baseline, from +4154 at the
start of this session.

## Why Brotli understates it

7021 raw bytes for 217 Brotli is a 32:1 ratio — escapes are extremely compressible, since
`\u0` repeats thousands of times. That is *why* nobody noticed: on a Brotli objective the defect is
almost invisible, while a raw- or gzip-objective artifact would have been paying nearly the full
7 KB. The check that finds it has to be structural, not a size comparison.

## Scope

Scanning every port's shipped ESM for escapes: **micromarklil (2037)** and **rehypelil (6327)** are
the two carrying them, with katexlil a distant third at 55. rehypelil is the larger of the two by
far:

| port | raw before | raw after | Brotli before | Brotli after |
|---|---:|---:|---:|---:|
| micromarklil | 94138 | 87117 | 26314 | **26097** (−217) |
| **rehypelil** | 192557 | **171889** | 53239 | **52363** (−876) |
| | | **−27689** | | **−1093** |

`159/159` rehypelil tests pass. It was already a win at −1841 against upstream and is now **−2717**.
One line of build configuration, two ports, 27 KB of raw and 1093 Brotli.

Worth noting for anyone reproducing: rehypelil compiles into a temporary directory and only
regenerates its ESM under `--compile`, so a plain `node scripts/build.mjs` silently measures the old
artifact.

## The pattern, fourth instance

This is the fourth time the whole of a defect has lived *between* the compiler and the artifact:

| | what | worth |
|---|---|---:|
| [006](../006-markdown-stack-loss-diagnosis/README.md) | rehypelil bundled without `minifyWhitespace` | 2517 Brotli |
| [028](../028-unminified-lil-lane/README.md) | the size harness minified only the official lane | 10634 Brotli |
| [030](../030-the-build-undoes-the-compiler/README.md) | micromarklil's bundler re-printed `!0` as `true` | 229 Brotli |
| **034** | **the bundler escaped every non-ASCII character** | **217 Brotli / 7021 raw** |

`shipped-vs-compiled.mjs` was written after 030 to catch exactly this class and it did **not** catch
this one: it compares compact-form spellings, not character encoding. Extending it to compare the
non-ASCII character count between `dist/*.raw.js` and the shipped artifact would have.
