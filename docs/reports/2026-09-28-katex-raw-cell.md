# Why the react-markdown stack's katex row (and three other cells) don't beat the original minified JS

2026-09-28. The question: on react-markdownlil's stack table, why doesn't our katex beat KaTeX's
minified JavaScript in all three of raw, gzip and Brotli? The answer is a bottom-up look at the
bytes. Every number here is lilscript-codec (Brotli 1.1.0 q11, zlib 1.3.1 -9, raw bytes). Scratch
work, scripts and artifacts are in `~/lilscript-work/out/rmk-katex-0928/`.

## 1. katex: the cells

| file | raw | gzip-9 | Brotli-11 |
|---|---:|---:|---:|
| katex's own `katex.min.js` (what KaTeX ships, CDN) | 276,701 | 75,840 | 62,686 |
| the stack table's bar: SWC 1.16.2 over KaTeX's Flow sources | 259,210 | 73,168 | 60,778 |
| @itslil/katex, released (5e47c1b) | **261,015** | 72,094 | 60,056 |
| same sources, today's compiler (b1-6 ≈ e4a44a7b) | **262,392** | 72,063 | 60,085 |

- Against KaTeX's own minified file we already win all three cells: −15,686 raw, −3,746 gzip and −2,630 Brotli.
- The table's bar is stronger than anything KaTeX ships. It is SWC over the Flow sources, which we built ourselves. Against it we win gzip (−1,074) and Brotli (−722) and lose raw by +1,805 (+0.7%). At today's head the raw loss is +3,182, because B1 changed the level-13 walk.

## 2. Bottom-up: where the raw bytes differ (released file vs the SWC bar)

Bytes by token class (acorn tokens plus AST identifier roles; `tokclass.mjs`):

| class | ours | SWC | Δ |
|---|---:|---:|---:|
| string literals | 88,024 | 55,122 | **+32,902** |
| template literals | 0 | 22,166 | −22,166 |
| numbers | 37,020 | 44,315 | −7,295 (font metrics as LilScript data) |
| object keys | 16,338 | 17,978 | −1,640 |
| keywords | 9,769 | 11,262 | −1,493 (`this` 5 vs 520; no `switch`/`case`) |
| punctuation | 57,546 | 58,850 | −1,304 |
| property names | 26,662 | 25,588 | +1,074 |
| whitespace | 2,246 | 1,756 | +490 |
| binding names | 17,771 | 17,447 | +324 |
| **total** | 261,015 | 259,304 | +1,711 |

Strings and templates together are **+10,736**. By value (`strings.mjs`), 228 short values are
repeated more often in ours: **+11,534 bytes in 1,751 extra occurrences**. Most of that is `"math"`
(577 vs 33), `"main"` (401 vs 3), `"textord"` (222 vs 27), `"text"`, `"mathord"`, `"accent-token"`
and so on. These are the arguments of the symbol table's 646 `defineSymbol` calls:

```
ours: Nb("math","main","rel","≡","\\equiv",!0);Nb("math","main","rel","≺","\\prec",!0);…
SWC:  ek(ez,eM,"rel","≡","\\equiv",!0),ek(ez,eM,"rel","≺","\\prec",!0),…
```

The port declares them as names (`src/symbols.lil`: `math = "math"; main = "main"; …`). The compiler
puts the literal back at every read: `forward_root_constants` in `src/js/root_constants.rs` says
"Every literal moves, whatever its length and whatever the objective". `pool_strings`, the only
thing that could give a name back, runs only under the raw objective, and katexlil sets it to false.

Everything else nets out in our favour: the numbers, class dissolution (5 `this` against 520) and
the object keys. The symbol-table constants alone turn a win into the loss.

## 3. The measurement: names are smaller in every codec

Splicing the 15 constants back as names into the compiler's output (`splice-symconst.mjs`; the
30-formula corpus renders identically in both display modes):

| file | raw | gzip-9 | Brotli-11 | vs SWC bar |
|---|---:|---:|---:|---|
| released + names | 251,493 | 71,746 | 60,046 | −7,717 / −1,422 / −732 |
| head (b1-6) + names | 252,870 | 71,739 | 59,986 | **−6,340 / −1,429 / −792** |
| Δ from the names alone (head) | −9,522 | −324 | −99 | |

Printing strings that contain newlines as template literals, as SWC does, is another −257 raw.
Its Brotli change is noise (−99..+26), so it is a raw-only lever.

## 4. Prior art: how the others spell a constant string

| tool | rule | source |
|---|---|---|
| Terser | inline only if `literal ≤ name + (name+2+literal)/reads` | `lib/compress/inline.js:300-314` |
| esbuild | inline `const` strings of ≤ 3 characters | `internal/js_ast/js_ast.go:1672-1677` |
| SWC | inline if read once or ≤ 3 characters; otherwise keep the name and remember the value for comparisons (`lits_for_cmp`) | `swc_ecma_minifier/src/compress/optimize/inline.rs:276-291` |
| Closure (current) | inline every well-defined immutable value, so AliasStrings stays off ("usually hurts gzip") | `InlineVariables.java:505-510`, `AliasStrings.java:40-41` |
| LilScript | Closure's rule, stated as canonical form | `src/js/root_constants.rs:11-17` |

The plan already expects this: architecture §646 says "short constants by rule, other values by
coupled choice" (M7.4, step 10). The measurement says a *static* size rule is enough here, because
all three codecs agree.

## 5. Prototype and fleet check

Branch `rootconst-size-rule` (worktree `~/lilscript-work/wt/rootconst`, one file, not merged): a
string root constant keeps its name when `k·P > k·2 + 2 + P + 2`, under every objective.
`LILSCRIPT_ROOT_CONSTANT_NAMES=0` restores the old behaviour for A/B with one binary.

Same binary, Brotli objective, rule on vs off (compiler output, Δ raw / gzip / Brotli):

| port | the rule compiled in | the same names spliced into the old output (nothing else moves) |
|---|---|---|
| katexlil | −6,931 / +74 / +130 | −10,360 / −403 / +1 (the 15 symbol-table constants alone: −9,522 / −324 / −99) |
| micromarklil | −2,139 / +176 / +146 | −1,975 / +44 / +170 |
| remark-mathlil | −354 / +42 / +40 | −384 / +29 / +27 |
| markedlil | 0 / 0 / 0 | – |

- With the rule compiled in, the delivered katex file is 255,461 / 72,127 / 60,247. That already wins all three cells against the SWC bar (−3,749 / −1,041 / −531), but it gives back 64 gzip and 162 Brotli against today's build.
- Part of that is the rule moving later choices: 111 ternaries became `if/else` and there are 55 more object keys. This is the terminal-challenger law again.
- Part of it is real. For micromark's token types (`"lineEnding"` ×36 …), the names cost Brotli even when nothing else moves (+170). The literal text is what the codec matches there.

So a static rule is right for raw only. Under gzip and Brotli, the name-or-literal decision belongs to the walk: one choice site per module's constants, judged on the final artifact. It keeps names for katex's symbol table (all three codecs smaller) and literals for micromark's token types. That is M7.4 with M9's choice sites, which is what the plan already says ("short constants by rule, other values by coupled choice").

## 6. The other red cells on the react-markdown page

**remark-math: loses all three.** Released 6,607 / 2,506 / 2,248 against Terser 6,350 / 2,324 /
2,097. Today's compiler without the banner gives 6,704 / 2,478 / 2,233, which still loses by
+354 / +154 / +136. Terser run over *our* output reaches 6,363 / 2,405 / 2,175 and still loses. So
the compiler's statement shapes are only part of it (Terser finds −341 raw and −58 Brotli). The rest
is port structure:
- 25 `|0` int coercions;
- 55 statement `if`s against the bar's 4;
- 63 `return`s against 36;
- 21 `x=function(` against 1;
- handlers written with `JS.methodRest` + `args[i]`, where upstream has plain parameters.

The 81-byte licence banner is about 50 Brotli of the gap on a file this small, and the Terser bar
carries none.

**remark: not like for like.** The only bar is upstream's browser graph, with no entity table.
Even after taking off the ~8 KB table it loses by about 5.7 KB, and its dist was built by an old
compiler (4dc4e33) and held on 2026-09-27.

**micromark / from-markdown / remark-parse** win against the like-for-like Node graph (table on both
sides). The win comes from our front-coded entity table. The browser-graph lanes in their detail tabs
(12,945 Brotli, DOM decoder, no table) are about 1 KB below our code without the table, so the
tokenizer code itself is still slightly larger than Terser's. That matches the earlier
"micromark = compressibility" diagnosis.

**The real app with KaTeX (+1,254 / +1,307 Brotli).** In that app KaTeX is npm's `katex` on *both*
sides. Our katex is not in it. The loss is duplication:
- npm `rehype-katex` (and `@itslil/rehype-katex`) turns KaTeX's HTML back into hast through
  `hast-util-from-html-isomorphic` → `hast-util-from-dom` → `hastscript` → `property-information`
  (18.4 KB raw / 5.5 KB Brotli minified alone);
- @itslil/react-markdown already carries its own compiled copy inside its bundle;
- upstream dedupes the two, and our monolith cannot.

Adding math and KaTeX costs us +5,010 Brotli more than upstream, about the size of that duplicate.

## 7. What wins all three cells

1. **katex:** M7.4 as a codec-judged choice. The name-or-literal spelling of a module's root
   constants becomes one walk site; under the raw objective the size rule decides it statically.
   Expected result from the isolated splice: 252,870 / 71,739 / 59,986, i.e. −6,340 / −1,429 / −792
   against the bar. The static prototype (branch `rootconst-size-rule`) must not land as is (§5).
   - Each objective's own build already wins its own cell (scorecard 2026-09-27: raw objective
     223,540 vs Oxc 258,754; gzip objective 72,060 vs 73,164; Brotli objective 60,281 vs 60,740).
     What is missing is one delivered file that wins all three, and the stack table shows only
     the Brotli build.
   - Today's head is +1,377 raw against the release on the same sources: B1's walk no longer
     reaches the structural recipes, and B1b is changing level 13 again.
2. **remark-math:** rewrite the port idiomatically (typed handler parameters, no `int` arithmetic
   where upstream uses numbers, expression-shaped branches). Settle the banner question.
3. **Real app with KaTeX:** @itslil/rehype-katex should build hast from KaTeX's own tree
   (`__renderToDomTree`) instead of parsing its HTML. That drops hastscript, property-information
   and hast-util-from-dom from the app. It should also use @itslil/katex.
