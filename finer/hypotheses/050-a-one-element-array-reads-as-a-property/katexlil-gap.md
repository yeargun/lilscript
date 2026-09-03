# Where katexlil's remaining Brotli gap is (measured 2026-09-03, build X = 65062)

Terser's published lane is 63044. Every number below is `lilscript-codec` Brotli-11 on
whole artifacts, differences of whole-artifact measurements, never a sum of parts.

## The gap is not the symbol table, the names, or anything Terser knows

| decomposition | ours | terser | delta |
|---|---:|---:|---:|
| whole artifact | 65062 | 63044 | **+2018** |
| the 648 `defineSymbol` calls, deleted from each | 3635 | 3762 | **−127** |
| everything else | 61427 | 59282 | +2145 |
| pure code (that, minus the shared 57626-byte metrics table) | 51954 | 49908 | +2046 |

The symbol table is a **win**: we inline `"math","main","textord"` at 648 call sites where
Terser keeps `me,pe,Me` bindings, which is +6483 raw and −127 Brotli. Outlining them back
into variables was measured three ways and is a loss at every threshold below 20 uses
(+1012 at ≥2, +28 at ≥8, −15 at ≥20, inside the ±60 floor). Brotli prices the repeated
literal below the reference.

What the gap is *not*:

- **Naming.** Terser's own mangler over our artifact: **−24**.
- **Anything in Terser's compressor.** Full `compress{passes:3}+mangle` over our artifact:
  −774; after the string-concat fold landed, −605. Per-pass on our own output:
  `evaluate` −124 (all printer), `collapse_vars` −173, `conditionals` −59, `if_return`
  **+51**, `sequences` +28, `booleans` +30. The printer alone (`{defaults:false}`) is −124
  of that, and ~20 of *that* is our license banner.
- **Property mangling.** Symmetric: −2358 for us, −2355 for them.
- **Code structure.** With every string and regex literal counted as one character and the
  metrics table removed, our code is 135164 characters against 134604 — **+560**, 0.4% —
  and we spend **2228 fewer** identifier bytes.

## What it is

Pure code is +2861 raw for +2046 Brotli: **0.72 Brotli per excess raw byte**, the rate of
genuinely novel content, spread thin rather than pooled anywhere:

| class (literals as one char) | ours | terser | delta |
|---|---:|---:|---:|
| `=` | 5471 | 4184 | +1287 |
| plain assignments (`;x=`, `,x=`) | 2185 | 1135 | +1050 |
| grouping parens | 789 | 619 | +170 |
| `,` | 11510 | 11036 | +474 |
| `(` total | 9685 | 9264 | +421 |
| identifiers | 77197 | 79425 | −2228 |
| `:` | 2928 | 3440 | −512 |

We materialise ~1050 more intermediates than Terser's output holds, and pay for them in
`=`, commas and parens while saving on identifier bytes. Terser's `collapse_vars` — which
inlines exactly those — recovers only 49 Brotli beyond its printer, so the materialisation
is *not* what the codec is charging for. Per function we are at parity or ahead:
`overline`'s builder is 319 bytes against KaTeX's 327 after 050's fixes landed.

## Ranked leads

1. **Object completion and temp inlining into literals.** `d={p:1};d.k=[e,c]` stays split;
   `fold_fresh_empty_object_assign` only takes `{}`, and `fold_single_use_temporaries` only
   takes a *pure read* initialiser, not an object or array literal. ~15 `.children=` sites
   plus ~19 short arrays of identifiers; ceiling ~340 raw.
2. **Printer.** 54 single-statement braces, ~20 redundant parens, ~15 strings that would
   escape less under `'`, two `Infinity`. Ceiling −100 after the banner.
3. **Phi copies sunk into branches.** `makeStackedDelim` repeats `A="",j=0,e="Size1-Regular"`
   in ~30 arms where KaTeX initialises once before the chain: 618 bytes of repeated
   assignments against their 372. Local to one function; ~250 raw.
4. **Spread.** The port hand-writes `pushAll`/`spliceSpread`/`maxOfList` because the
   language has none; `o.splice(e,2,...s[k])` costs us a 120-byte IIFE. 8 `.apply` sites.

## Dead ends, measured

Redundant `+` coercions (`3*+h`, `0-+c`): raw −236, Brotli **0**. `let`→`var`: +18.
Statement joining to 49 top-level items: −76. Outlining repeated literals: above.
Larger terminal probe budget (256→1024): byte-identical.

## The bottom-up pass, finished (2026-09-03)

Every function in both lanes, paired two ways and measured:

| pairing | pairs | our excess |
|---|---:|---:|
| same module and name, sharing a string literal | 211 | **−581 bytes** |
| unmatched remainder, matched by string-literal Jaccard ≥ 0.5 | 189 | **+37 bytes** |

Wherever a function can be paired at all, we are at parity or ahead. Splitting
the artifact by AST instead (no map, so it works on any lane) says the same:

| part | ours raw | ours Br | theirs raw | theirs Br | delta |
|---|---:|---:|---:|---:|---:|
| glue (everything outside a function) | 150604 | 30253 | 145223 | 30619 | **−366** |
| bodies (477 vs 457 functions) | 126659 | 34794 | 122741 | 32549 | **+2245** |

−366 + 2245 = +1879, which is the +1913 gap. So the whole of it is in function
bodies, spread as ~14 raw bytes per function over ~280 functions that the two
lanes partition differently — not in any function you can point at.

## What this predicts, and what it cost to check

The same shape holds on the other ports: Terser's own compressor over *our*
artifact recovers −333 of micromark's +3188 and −92 of remark-math's +182. The
gap is code shape, not a missing pass, on every port measured.

Two more results worth keeping:

- **remark-math is the same raw size as its Terser baseline** (6368 vs 6350)
  and still +182 Brotli. Its structure count is identical to the byte (5305 vs
  5306 characters with literals counted as one); we spend +72 `=` and +56 `,`
  and 159 fewer identifier bytes. Compressibility, not size.
- **The `/*! … MIT */` banner is 50–55 Brotli on a small port** — 39% of
  remark-math's gap and 26% of unified's — and the Terser baselines carry no
  comment. On katexlil the same banner is worth −91 (removing it makes the
  artifact *larger*). Whether the ports keep it is a licensing call, but the
  comparison is not like-for-like today.

## Rejected this session, each measured on a real build

Local function declarations (`var f=function(){}` -> `function f(){}`, the shape
Terser's output has and ours does not): micromark **+108**, remark-math +11.
Namespace member aliasing (`X.MathNode` -> `_n0`, 218 sites, raw −1644): **+68**.
Splitting a completed literal declarator out of its list: +169. Stepping the
object fold by comma element: same. Non-empty array literals absorbing their
pushes, and inert literals that read a member: fire nowhere in the pipeline.
`function_spelling = "function"`: +95. Inlining a hoisted numeric constant: −1.

## The other losing ports say the same thing (2026-09-03)

The bottom-up pass was repeated on micromark, which is the biggest cluster of
losses (micromark, remark-parse, mdast-util-from-markdown and remark share one
source style) and, unlike katexlil, is an already-*typed* port — `int[] stack =
[]`, `list.push(...)`, native array literals, 132 typed declarations.

| micromark | ours | upstream Terser | delta |
|---|---:|---:|---:|
| whole | 25964 | 22696 | +3268 |
| glue | 11004 | 10790 | +214 |
| bodies (112 vs 105 functions) | 14875 | 11869 | **+3006** |

Terser's own compressor over our artifact recovers −333 of that. Function
pairing: 56 pairs come to **+455**, i.e. ~8 bytes each. So a typed port loses
the same way an untyped one does — typing is not what separates the lanes.

Two costs that *are* specific to the typed port, both measured:

- **`|0` on int arithmetic**: 130 occurrences against upstream's 3. Removing
  every one is **−44 Brotli** (raw −260). Range analysis that proved an index
  stays in int32 would recover it.
- Explicit `+""` key coercions: 14 sites.

And one thing that is not a code difference at all: three of those ports
(remark-parse, mdast-util-from-markdown, remark) build with
`candidate_search = "production"` and no `terminal_codec_probe_limit`, where
katexlil uses `"always"` with 256. Strengthening both is −26 and −30 on the
first two; remark measures +628, but that delta is present with this session's
folds *skipped* as well, so it predates them and belongs to whoever owns that
port's last compiler bump.

## Fleet standing with this session's compiler, and one flip it caused

Measured against the pinned upstream Terser baselines, every port rebuilt with
the compiler at `8f2fcd0`:

**8 wins / 9 losses of 17 measured.** Wins: zod −20061, rehype −2953,
hast-util-to-html −1014, remark-gfm −904, rehype-stringify −794,
mdast-util-to-hast −750, remark-rehype −697, remark-breaks −55. Losses:
posthog +27, remark-math +129, unified +214, katexlil +1820, mobx +2638,
mdast-util-from-markdown +2794, remark-parse +2896, micromark +3188,
remark +5316.

Three ports have moved against the 2026-09-02 05:07 scoreboard for reasons
that are *not* this session's folds — rebuilding each with
`LILSCRIPT_SKIP_FOLDS` set to all four gives the same number:

| port | scoreboard | today | with our folds skipped |
|---|---:|---:|---:|
| rehype | 51696 | 52127 | 52127 |
| remark | 37239 | 37867 | 37869 |

Those two belong to a compiler generation between the scoreboard and this
session's first commit.

**posthog is ours.** Bisecting the compiler across every commit since the
scoreboard (1f195fa, e0c1c22, 9518ed4, 7871329, cc3715e, e112c94, b6da284,
c40800e, 1780fa5, b181873 all give 5621) lands on `3f1b1f6` — the object-literal
absorption fold — which takes it to 5649 and flips a −1 win into a +27 loss.
The fold does not make posthog's code worse: the artifact diverges at byte 90
with different names and a different function layout, and its token counts
*improve* (`=` 521 -> 518, `,` 640 -> 630). It moved the terminal search into a
different basin, which that port's own config comment already describes ("beam
width is not monotone: 12 gives 5668, 22-26 give 5621, 32/48/64/96 regress to
5720-5755… it selects which basin the terminal search lands in"). Re-tuning does
not recover it: beam 20/22/26/28 give 5698/5674/5649/5649 and
`local_name_reserve` 24/32/64 give 5700/5721/5720, all at or above 5649.

The fold stays, because the fleet verdict is what decides: katexlil −104,
remark-gfm −182, micromark −43, mobx +0, posthog +28 — **net −301**. But one
port lost its win to it, and that is worth more than 28 bytes to know.

## Trying to give posthog its win back

The absorption fold costs posthog 28 bytes by moving the terminal search into a
worse basin, not by making its code worse. The project's answer to that is a
scored late family — offer the rewrite to each beam member and keep it only
where the codec agrees — which is how `shape_declarations` and the regex
spelling already work. Built and measured:

| port | fold in the session | fold as a scored late family |
|---|---:|---:|
| posthog | 5649 (**loss** +27) | **5621 (win −1)** |
| katexlil | 64957 | 65026 |
| micromark | 25964 | 25984 |
| remark-gfm | 10334 (win −904) | 10502 (win −736) |
| mobx | 15575 | 15575 |

Nine wins against eight losses instead of eight against nine, for 229 bytes
spread over three ports that all keep their standing. On the fleet's own metric
that is the better artifact — but **the late candidate is a wrong program**:
katexlil's screenshot corpus differs on 123 of 130 items and 589 official tests
fail, reproduced with main's own compiler as well as the source-map worktree.
Run standalone on the finished artifact the same fold makes zero rewrites and
changes nothing, so what it mis-handles is a mid-cleanup beam candidate, whose
shapes the session version never sees.

The wrong program was not the absorption. Diffing the broken artifact against
the good one found KaTeX's lexer reading

    var b=a[6];return b?b:a[3]||a[2]?"\\ ":" "

where it owed `var b=a[6];if(b)return b;b=a[3];return b?b:a[2]?"\\ ":" "` —
`(a[3]||a[2])?"\\ ":" "` answers `"\\ "` where it owed `a[3]`. That is
`fold_assigned_truthy_ternaries`, which rewrites `(b=E)?b:F` to `E||F` and takes
F from `complete_primary_end`: a conditional continuing past that primary binds
looser than the `||`. The same defect as `fold_ident_ternary_to_or` in 1780fa5,
in its sibling, and latent — the beam simply never selected the candidate
carrying it until the scored family started offering candidates that did.

Both landed in `55303e2`. With the sibling guarded the late family is correct
(screenshot corpus identical, 1230 official tests and 123 snapshots pass) and
the fleet is **nine wins against eight losses**: posthog 5621 (−1, back to a
win), remark-gfm 10502 (−736, still a win), katexlil 64993, micromark 25984,
and remark-breaks / mdast-util-to-hast / remark-rehype unmoved.

Also confirmed while measuring: the narrow wins hold either way (remark-breaks
−54, mdast-util-to-hast −750, remark-rehype −697), and unified (+214) and
remark-math (+129) are unmoved by any of it.
