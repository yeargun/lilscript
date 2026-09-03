# 051 — a typed-array element read is coerced as if it could overflow

**Status: CONFIRMED and priced, not yet landed. The i32 range proof that already elides `.length`
and `charCodeAt()` does not know a typed-array element's type, so every `Int32Array`/`Uint8Array`
read is emitted as `a[i]|0`. On cnlil that is 81 of 162 coercion sites and 33 Brotli, which takes
the port from +48 to +15 against upstream-through-Oxc.**

Lane: compiler. Objective: brotli (objective §2). Opened: 2026-09-03.

## Prior art

- **The elision already exists.** `CompressionDecision::SafeIntegerCoercionElision` is enabled for
  every priority except performance-first (`config.rs:969`), and reaches emission as
  `elide_safe_integer_coercions` (`config.rs:269`). cnlil compiles size-first, so it is on.
- **Oxc/Terser will not do this for us.** Re-minifying our own output leaves all 162 `|0` in place:
  a JavaScript minifier cannot know the array is an `Int32Array`, so the coercion is load-bearing
  as far as it can prove. This is information only a typed front end has.
- **V8** reads an `Int32Array` element as an i32 already; `|0` on it is an identity operation.

## Claim

An element read from `Int32Array`, `Int16Array`, `Int8Array`, `Uint16Array`, `Uint8Array` or
`Uint8ClampedArray` is in i32 range by construction, so a generated `|0` on it is redundant.
`Uint32Array` is **not** covered: it holds 0..2^32−1, where `|0` is observable
(`4294967295|0 === -1`).

## Repro

`finer/out/051/t.lil`, size-first level 13, identifiers unmangled:

    export int lenMinus(string s)            { return s.length - 1; }   →  n.length-1      elided
    export int charAt(string s, int i)       { return s.charCodeAt(i); } →  n.charCodeAt(r) elided
    export int mulTwo(int a, int b)          { return a * b; }          →  n*r|0           correct: can overflow
    export int readTyped(Int32Array a,int i) { return a[i]; }           →  n[r]|0          REDUNDANT
    export int addOne(Int32Array a, int i)   { return a[i] + 1; }       →  (n[r]|0)+1|0    inner REDUNDANT

The analysis is otherwise working. It is the element type it is missing.

## Result — measured on the cnlil artifact, pinned codec

| variant | raw | brotli | saved | vs upstream-through-Oxc |
|---|---:|---:|---:|---:|
| ours, licence banner stripped | 26724 | 9336 | 0 | +48 |
| + elide the 81 array-read coercions | 26562 | **9303** | 33 | +15 |
| + collapse the parentheses that frees | 26470 | 9319 | 17 | +31 |
| + elide every remaining coercion (ceiling, unsound) | 26400 | 9275 | 61 | −13 |
| upstream through Oxc | 26617 | 9288 | — | 0 |

Ten conformance spot checks pass with all 81 removed.

## The warning this folder is really for: partial elision can cost Brotli

Two measured inversions, both smaller in raw and **larger** compressed:

| change | Δraw | Δbrotli |
|---|---:|---:|
| elide 42 of 162 coercions (a conservative subset) | −84 | **+24** |
| collapse the parens freed by the array-read elision | −92 | **+16** |

`|0` is a short, highly repeated token that Brotli models almost for free; thinning it unevenly
costs more in broken repeats than it wins in bytes. So this elision must be **thorough or not at
all**, and any peephole that trades a repeated shape for fewer raw bytes belongs behind the
`cost_model = "brotli"` candidate scorer rather than applied unconditionally.

## Next

Teach the i32 range proof the typed-array element types, keeping `Uint32Array` out. Then the
standing rule applies: this is a compiler change, so it needs the fleet A/B across all 22 ports
and each port's own suite, not cnlil alone.

## Sibling lead: no statement fusion

Oxc extracts 27 Brotli from our own already-minified output by fusing statements — 48 semicolons,
18 `if(` headers, 17 braces and 15 `return`s become `x&&(a(),b())` and ternaries. We have no such
pass. Independent of the coercion work and worth its own folder.

## Audit: what the port was leaving unused (2026-09-03)

Asked whether the port fails to use compiler features it has. Measured, mostly negative, and the
negatives are worth keeping so nobody re-runs them.

**`pure` is inferred, so annotating buys nothing.** Marked all 48 cnlil function declarations
`pure` and iterated the rejections. 46 are provably pure and the compiler already knows it
(`language-v0.1.md:795`: "Purity is inferred for every function by interprocedural effect
analysis"). Every rejection is correct:

| rejected | why |
|---|---|
| `buildLiteralPool`, `initLiterals`, `resetMemo`, `memoPut`, `growTokens`, `growCheckpoints`, `internSpan` | mutate module-level arrays |
| `sortStrings` | insertion-sorts its argument in place |
| `isUnicodeWhitespace` | `RegExp.test` is an observable host call |
| `numericString` | `JS.number(value)` is a dynamic `JsValue` coercion |

The modifier is a checked contract, not an optimization unlock. Its real use here is as a
regression fence on functions that must stay effect-free.

**Property mangling has nothing left.** The port sets `mangle.properties = false`. Its output
carries 13 non-builtin property names over 45 occurrences totalling 110 bytes, and most are real
builtins (`imul`, `fill`, `floor`, `pop`). The `extern class` fields are already spelled `r`, `a`,
`n`, `a0`, `a1`, `a2`. Flipping the switch is not worth a rebuild.

**The two opt-in compression decisions stay off.** `effect-ternary` is documented as already
measured and lost (`configuration.md:492`), and canonical emission recovers the ternary anyway;
`global-alias-forwarding` is off by design because forwarding "can remove syntax while weakening
repeated byte shapes" (`config.rs:988`), which is the same Brotli inversion this folder measures.
The port omits `compression` entirely and so takes the full size-first default set. Its config is
already the right one.

**The banner was the cheapest byte in the project.** `scripts/build.mjs` prepended
`/*! @itslil/cn 0.2.4 | LilScript reimplementation of cn | MIT */` to both shipped entries. 65
bytes of text that compresses against nothing:

| shipped file | raw | brotli |
|---|---|---|
| `dist/index.js` | 26789 → 26724 | 9372 → **9336** |
| `dist/index.cjs` | 29229 → 29163 | 9931 → **9893** |

Removed. `LICENSE` and `NOTICE.md` are both in the package `files` list, so the MIT notice still
ships. This does not move the published site number, which measures with `legalComments: "none"`.

**Also measured: typing the prediction chain costs 51 Brotli.** Retyping `ArgumentEntry.n` from
`JsValue` to `ArgumentEntry?`, which removes a `JS.assume` and a truthiness test at each of the
three prediction hops, took the sanctioned boundary from 9456 to 9507 with all suites green.
Reverted pending a benchmark that shows it buys a lane. Source kept at
`finer/out/051/typed-n.default.lil`.
