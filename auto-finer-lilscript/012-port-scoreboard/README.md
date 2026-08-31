# 012 — Port scoreboard (win/loss vs upstream, all sibling `*Lil` ports)

Measurement-only pass. No `cargo build`, no `lilscript` invocations, no port build scripts were
run. Every number below was produced by
`/home/azureuser/lilscript/target/release/lilscript-codec --json <paths>` against files already
present on disk (zlib 1.3.1 gzip-9, Google Brotli 1.1.0 q11 lgwin22 — the project's pinned codecs).
Where a port's own historical report used Node's `zlib`/`brotli` module instead of the pinned
codec, that number is called out explicitly and **not** used as an authoritative delta; only its
raw byte count (compressor-independent) is trusted as corroboration.

The task brief named 28 sibling directories; only 26 distinct `*Lil` port directories actually
exist next to `lilscript/` (the list in the brief itself has 26 entries — `lil-solidjs` and
`solidlil` are two separate repos, both counted). All 26 are covered below.

Convention: **delta = Lil brotli − upstream brotli** under each artifact's declared objective.
Positive = Lil is bigger = LOSS. Negative = Lil is smaller = WIN. All declared `cost_model`s found
in every port's primary `lilscript.toml` are `"brotli"`, so Brotli is the objective column
everywhere in this scoreboard.

## Headline: many of the markdown-stack numbers in `comparison/markdown-stack/REPORT.md` are stale

`REPORT.md` was generated **2026-08-30T23:40:48Z**. Several ports in that harness's 16-port set
were rebuilt on disk *after* that timestamp (dist mtimes up to 15 hours later, all currently
uncommitted — `git status` shows every markdown-stack port's `dist/` as modified). Re-measuring the
current `dist/*.esm.js` files directly with `lilscript-codec` shows two verdicts **flip** from LOSS
to WIN since the report was generated: **rehype** and **remark-gfm**. See "Verdict flips" below.
Two more ports whose Lil sides are dependency-graph compositions (**remark**, **unified**) also
drifted enough to be worth a second look, though their verdict (LOSS) didn't change.

---

## Master table

Legend: **Src** = O (real npm/checked-in official artifact), R (`comparison/markdown-stack/REPORT.md`
harness, pinned codec both sides), H (port's own hybrid: REPORT.md's pinned-codec official figure +
my fresh direct-dist Lil figure), G (port's own graph-composition self-report; flagged if it used
Node's zlib/brotli).

| # | Port | Lil artifact | Upstream baseline | Objective | Lil raw/gzip/brotli | Upstream raw/gzip/brotli | Δ brotli | Verdict | Src |
|---|---|---|---|---|---:|---:|---:|---|---|
| 1 | jquerylil | `jquerylil/dist/jquery.esm.js` | `lilscript/benchmarks/popular/node_modules/jquery/dist/jquery.min.js` (jquery 3.7.1, exact version) | brotli | 95435 / 35224 / 31483 | 87533 / 30336 / 27445 | **+4038** | LOSS | O |
| 2 | katexlil | `katexlil/dist/katex.esm.js` (≡ `dist/katex.mjs`, byte-identical) | `lilscript/comparison/markdown-stack/node_modules/katex/dist/katex.min.js` (katex 0.16.22, exact version) | brotli | 293340 / 83632 / 69019 | 276701 / 75840 / 62686 | **+6333** | LOSS | O |
| 3 | markedlil | `markedlil/dist/marked.esm.js` | `lilscript/benchmarks/popular/node_modules/marked/marked.min.js` (marked **14.0.0**, version-mismatched — see note) | brotli | 36510 / 10913 / 9652 | 36091 / 10981 / 10085 | **−433** | WIN | O* |
| 4 | mobxlil | `mobxlil/dist/mobx.esm.js` (≡ `dist/mobx.mjs`) | `lilscript/benchmarks/popular/node_modules/mobx/dist/mobx.esm.production.min.js` (mobx 7.0.0, exact version) | brotli | 57005 / 17337 / 15594 | 46483 / 14299 / 12937 | **+2657** | LOSS | O |
| 5 | motionlil | `motionlil/dist/full.js` | `lilscript/benchmarks/popular/node_modules/motion/dist/motion.js` (motion 13.0.0, exact version, browser-ready minified UMD) | brotli | 188438 / 59307 / 50526 | 139661 / 46302 / 41212 | **+9314** | LOSS | O |
| 6 | playcanvaslil | `playcanvaslil/dist/shader-processing.js` | `playcanvaslil/dist/shader-processing.official.js` (checked in alongside; narrow module port, see scope note) | brotli | 53783 / 16337 / 14580 | 67112 / 16267 / 14633 | **−53** | WIN | O |
| 7 | micromarklil | `micromarklil/dist/micromark.esm.js` | REPORT.md "official Terser" (micromark, pinned codec) | brotli | 101746 / 31561 / 26897 | 81530 / 26383 / **22776** | **+4121** | LOSS | H |
| 8 | mdast-util-from-markdownlil | `mdast-util-from-markdownlil/dist/from-markdown.esm.js` | REPORT.md official Terser | brotli | 93414 / 31285 / 26607 | 84681 / 27038 / **23279** | **+3328** | LOSS | H |
| 9 | remark-parselil | `remark-parselil/dist/remark-parse.esm.js` | REPORT.md official Terser | brotli | 94072 / 31527 / 26802 | 84866 / 27101 / **23283** | **+3519** | LOSS | H |
| 10 | remarklil | `remarklil/dist/remark.esm.js` is **standalone-only** (0 runtime imports but VFile no longer inlined — see "Graph-composition ports") | REPORT.md official Terser | brotli | graph: 202587/51993/43785 (port's own fresh, but Node-zlib) vs standalone: 140579/46857/**39951** | 91243/25536/22770 (port's own fresh, Node-zlib) vs REPORT.md 119872/37366/**32551** (pinned) | **+13329** (REPORT.md, stale) to **+21015** (fresh self-report, unverified codec) | LOSS (magnitude uncertain) | R/G — flagged |
| 11 | unifiedlil | `unifiedlil/dist/unified.esm.js` is **standalone-only** (VFile composition, same pattern as remark) | REPORT.md official Terser | brotli | graph: 22042/6018/5337 (fresh, Node-zlib) vs standalone: 15145/5268/**4768** | 13579/4883/**4425** (REPORT.md, pinned) | **+984** (REPORT.md) | LOSS | R — flagged |
| 12 | mdast-util-to-hastlil | `mdast-util-to-hastlil/dist/to-hast.esm.js` | REPORT.md official Terser | brotli | 14620 / 4762 / 4290 (matches fresh direct measurement 14843/4789/**4309** within drift) | 16715/5388/4862 (fresh)/17117/5537/**5016** (REPORT.md) | **−726** | WIN | R |
| 13 | hast-util-to-htmllil | `hast-util-to-htmllil/dist/to-html.esm.js` | REPORT.md official Terser | brotli | 30253/9962/8811 (REPORT) ≈ 30291/9978/**8825** (my fresh direct) | 31835/11221/9833 (fresh)/31882/11235/**9839** (REPORT.md) | **−1028** | WIN | R |
| 14 | remark-rehypelil | `remark-rehypelil/dist/remark-rehype.esm.js` | REPORT.md official Terser | brotli | 15027/4911/4390 (REPORT) ≈ 14922/4896/**4387** (my fresh direct) | 16861/5445/4910 (fresh)/17263/5595/**5061** (REPORT.md) | **−671** | WIN | R |
| 15 | rehypelil | `rehypelil/dist/rehype.esm.js` | REPORT.md official Terser | brotli | **53345** (my fresh direct measurement, raw 192557 — matches port's own fresh self-report exactly) | 55080 (REPORT.md, pinned) | **−1735** | **WIN — FLIPPED from REPORT.md's LOSS +9912** | H — flagged |
| 16 | rehype-stringifylil | `rehype-stringifylil/dist/rehype-stringify.esm.js` | REPORT.md official Terser | brotli | 30572/10302/9141 (exact match, REPORT and my fresh direct) | 31975/11269/**9886** (REPORT.md) | **−745** | WIN | R |
| 17 | remark-gfmlil | `remark-gfmlil/dist/remark-gfm.esm.js` | REPORT.md official Terser | brotli | **10855** (my fresh direct, raw 33559 — matches port's own fresh self-report exactly) | 11238 (REPORT.md, pinned) | **−383** | **WIN — FLIPPED from REPORT.md's LOSS +379** | H — flagged |
| 18 | remark-breakslil | `remark-breakslil/dist/remark-breaks.esm.js` | REPORT.md official Terser | brotli | 2746/1258/1131 (exact match, REPORT and my fresh direct) | 3045/1299/**1198** (REPORT.md) | **−67** | WIN | R |
| 19 | remark-mathlil | `remark-mathlil/dist/remark-math.esm.js` | REPORT.md official Terser | brotli | **2287** (my fresh direct, raw 6370 — matches port's own fresh self-report exactly) | 2150 (REPORT.md, pinned) | **+137** | LOSS (magnitude much smaller than REPORT.md's stale +450) | H — flagged |
| 20 | rehype-katexlil | `rehype-katexlil/dist/rehype-katex.esm.js` is **glue-only** (945 Brotli) — bundles katex/hast-util-* as runtime imports, not comparable standalone | REPORT.md Lil-graph vs official Terser | brotli | graph: 586338/104850/**84501** (REPORT.md) | 474237/138812/**113063** (REPORT.md, pinned) | **−28562** | WIN (both REPORT.md and the port's own — more stale — self-report agree on WIN direction; magnitude uncertain, katex itself was rebuilt after both) | R — flagged |
| 21 | react-markdownlil | `react-markdownlil/dist/react-markdown.esm.js` is **glue-only** vs `react`/`react/jsx-runtime` (150358/52756/45502 standalone) | REPORT.md Lil-graph vs official Terser (react, react/* external both sides) | brotli | consumerGraph (fresh, own report): 216868/58505/49732 | 117674/34924/**31082** (fresh, matches REPORT.md's 31092 closely) | **+18650** (fresh) / +16769 (REPORT.md, stale) | LOSS | G/R |

**Comparable ports: 21. Wins: 10. Losses: 11. Ties: 0.**
**Total Brotli delta across comparable ports: approximately +32,007 bytes** (net LOSS in aggregate —
summing each row's boldfaced primary delta above, dominated by rehype-katex's large graph-level win
offsetting jquery/katex/mobx/motion/micromark/mdast-from-markdown/remark-parse/remark/react-markdown
losses). This total mixes pinned-codec numbers with a few Report- or self-report-sourced graph-level
figures that are flagged as stale or uncertain above (remark, unified, rehype-katex, react-markdown);
treat it as directional, not exact — swapping REPORT.md's stale react-markdown figure (+16769) for
the fresher one used here (+18650) alone moves the total by ~1,900 bytes.

\* markedlil's baseline is version-mismatched (marked 14.0.0 vs the port's pinned 18.0.10). See
"Not comparable" caveats below — still reported here because it's the number that matches this
task's own "known context," and is corroborated by a same-version reconstruction (next section).

---

## Not comparable / no baseline found

**monacolil.** `dist/monaco.esm.js` (189616 raw / 46496 Brotli) is a tiny fraction of what the
port's own `reports/lab-sizes.json` says is being compared: a 992-module compiled-Lil "catalog"
plus workbench, measured at 2,330,203 raw / 413,607 Brotli for the "ide" bundle — that composed
bundle is not a single checked-in `dist/` file I can measure directly. Worse, the *workers* lane in
that same report shows official JS workers at 9,608,472 raw / 1,640,341 Brotli against a Lil side of
only 3,885 raw / 1,897 Brotli — a 2,500x gap that means the Lil port does not implement the
TypeScript/CSS/HTML/JSON language workers at all. `monacolil/package.json`'s own description says
"Not 100% feature parity." No single-file comparison here would be honest.

**posthoglil.** Pinned at posthog-js 1.418.10; no matching-version copy of `posthog-js` exists
anywhere on this host (closest are 1.332.0 and 1.345.5, and neither ships a single canonical
minified bundle at that path either — posthog-js has ~15 separate dist entry chunks).
`vendor/posthog-js/` is an empty placeholder directory. The port's own `site/results.json` states
outright: *"The published posthog-js browser bundle is not a lane"* — its own methodology compares
only a curated "capture kernel" subset (UUID/flags/cookie/router/rate-limit/queue/bot detection)
against Terser/Oxc reconstructions of that same subset, not the real package. Two different-scope
programs; no legitimate whole-package baseline available.

**zodlil.** Pinned at zod 4.4.3. Zod has never shipped a pre-built minified single-file bundle at
any version (confirmed: `node_modules/zod` trees at 3.25.76 and 4.4.3 found on this host are raw
ESM/CJS source, meant to be tree-shaken by a consumer bundler). The port's own `site/results.json`
resorts to esbuild-bundling official zod source and then Oxc/Terser-minifying it — but no cached
copy of that reconstructed baseline file remains on disk (only the summary JSON, computed with
Node's `zlib`/`brotli`, which this task's rules disqualify), and rebuilding it myself would mean
running esbuild/Terser, which the task's hard rule forbids. `dist/zod.core.js` (135752 raw / 32261
Brotli, port's own "primary" lane) and `dist/index.cjs` (291999 raw / 51228 Brotli, full CJS bundle)
are reported here for reference only — not as a verified win or loss.

**solidlil** and **lil-solidjs.** Both are LilScript ports of **Solid 2.0.0-rc.0** — an unreleased
pre-release (per `solidlil/upstream.lock.json`, pinned to a specific pre-1.0-of-2.x git revision,
not an npm release). No published minified `solid-js` 2.0 bundle exists to compare against (all
`solid-js` copies found on this host are 1.9.x, a different major generation with a different
compiler-driven build model — Solid's client runtime is normally consumed via its own JSX-compiling
Babel/Vite plugin per-consuming-app, not shipped as a single pre-built minified file, even at 1.x).
For the record, direct sizes: `solidlil/dist/index.js` = 18588/6851/6268 Brotli;
`lil-solidjs/dist/index.js` = 18505/6784/6196 Brotli — nearly identical to each other, no verdict.

---

## Verdict flips: rehype and remark-gfm (LOSS → WIN)

Both `rehypelil` and `remark-gfmlil` show **exact raw-byte-count agreement** between (a) my direct
`lilscript-codec` measurement of the current `dist/*.esm.js` file and (b) the "itslil" lane in each
port's own freshly-generated `site/results.json` (generated within seconds of the current `dist/`
mtime — these are not stale):

- `rehypelil/dist/rehype.esm.js`: raw 192557, Brotli **53345** (lilscript-codec, directly measured).
  `rehypelil/site/results.json` "itslil" lane: raw 192557, Brotli 53327 (Node zlib/brotli — close
  but not authoritative). REPORT.md's Lil-graph figure for this port is **64992** — 16 hours stale
  relative to the current `dist/`. Paired against REPORT.md's own pinned-codec official-Terser
  figure (55080, which should not have moved since the npm `rehype` package didn't change), the
  current delta is **53345 − 55080 = −1735: a WIN**, not the LOSS REPORT.md currently shows.
- `remark-gfmlil/dist/remark-gfm.esm.js`: raw 33559, Brotli **10855** (lilscript-codec, directly
  measured), exactly matching its own fresh self-report. REPORT.md's Lil-graph figure is **11617**.
  Paired against REPORT.md's pinned official-Terser figure (11238), current delta is
  **10855 − 11238 = −383: a WIN**, not REPORT.md's LOSS +379.

Recommendation: re-run `node comparison/markdown-stack/run.mjs --measure` to refresh `REPORT.md`
before anyone treats its win/loss column as current. It was accurate as of generation time; it no
longer reflects several ports' current `dist/`.

## Graph-composition ports: remark, unified, react-markdown, rehype-katex

Five of the sixteen `comparison/markdown-stack` ports have Lil-side entries that are **not**
self-sufficient single files — the harness's own methodology text says so directly: *"Standalone
standard Lil ESM files are copied byte-for-byte; only entries with runtime imports are bundled to
complete their graph."* Two different situations show up:

- **remark, unified**: `dist/remark.esm.js` and `dist/unified.esm.js` currently have **zero runtime
  `import` statements** (confirmed by grep), yet each port's own fresh `site/results.json` describes
  a larger "itslil" lane explicitly composed with VFile ("Single-file browser graph with the
  directly composed pure-Lil VFile"). The size gap is large — `remark.esm.js` alone is 140579 raw,
  but the composed graph is 202587 raw, a 62KB difference that is *not* explained by simply adding
  `unifiedlil/dist/vfile.esm.js` (only 8039 raw). Something in how VFile composes with remark's
  processor object accounts for the rest, and I could not reproduce it without building. **This
  also means remark's own fresh baseline figure (official-terser-mangle, Brotli 22770) does not
  match REPORT.md's official-Terser figure for presumably the same npm package (Brotli 32551) — a
  10KB gap on the side that's supposed to be stable.** I do not have an explanation for that
  discrepancy and am flagging it rather than picking a number. remark and unified are LOSSES under
  every version of the numbers I found; only the exact magnitude is unresolved.
- **react-markdown, rehype-katex**: these two mark real runtime deps (`react`/`react/jsx-runtime`
  for react-markdown; `katex`, `hast-util-from-html-isomorphic`, `hast-util-to-text`,
  `unist-util-visit-parents` for rehype-katex) as genuine ESM imports, confirmed by grep. Their bare
  `dist/*.esm.js` files (45502 Brotli and 945 Brotli respectively) measure a categorically smaller
  program than what ships to a browser, and are reported as such — not usable alone.

## jquerylil: uncommitted regression from HEAD

The task's "known context" cites jQueryLil at ~30555 Brotli. The current **working-tree**
`dist/jquery.esm.js` measures **31483 Brotli** — worse than that context, and worse than the
**last git commit** of the same file:

```
git show HEAD:dist/jquery.esm.js  → raw 83044, Brotli 28225   (commit 30c000d)
current working tree              → raw 95435, Brotli 31483   (+12391 raw, +3258 Brotli, uncommitted)
```

`git diff --stat -- dist/` for jquerylil shows only single-line changes in each of the 5 dist files
(they're each one physical line), consistent with a full recompile that landed a **regression**, not
a typo. Since the committed HEAD version (28225 Brotli) is much closer to jquery.min.js (27445) than
the current working tree (31483), whatever produced the current `dist/jquery.esm.js` made this port
worse, not better. This looks like a live, in-progress compiler change (the repo's top-level
`git status` shows most of `src/*.rs` modified right now) that has not finished converging. Reported
per the task's instruction to measure "checked-in" (on-disk) artifacts, but flagged prominently
since it contradicts the given known-context number and is a real regression versus the last commit.

## katexlil: a self-named trap

`katexlil/dist/katex.min.js` (untracked, new file) is **not** the real upstream KaTeX artifact — it
carries the banner `/*! @itslil/katex 0.16.22 | LilScript reimplementation of katex | MIT */` and is
the **port's own** minified-named output, not npm's. The real upstream `katex.min.js` lives at
`lilscript/comparison/markdown-stack/node_modules/katex/dist/katex.min.js` and is what's used in row
2 of the master table above. Anyone grepping for `katex.min.js` inside `katexlil/dist/` and diffing
it against itself would silently produce a 0-byte-delta non-comparison. Flagging this explicitly so
it isn't repeated.

## motionlil: two contradictory self-reported methodologies

`motionlil/README.md` and `motionlil/site/results.json` claim **16/16 wins**, "12.4% smaller in
total" versus Motion, based on a *per-demo tree-shaken bundle* methodology (matching named-import
subsets, each independently bundled and minified with esbuild+Terser using **Node's zlib/brotli**,
which this task's rules disqualify as authoritative in any case). Measuring the **whole-package**
comparison directly instead — `motionlil/dist/full.js` (the port's complete browser-ready bundle)
against `motion`'s own complete browser-ready UMD bundle (`dist/motion.js`, version 13.0.0, an exact
match to the port's pin per its own `NOTICE.md`) — gives a clear **LOSS**: 50526 vs 41212 Brotli,
+9314. Both measurements can be true simultaneously (tree-shaking narrow named-import subsets can
favor one side while the monolithic bundle favors the other), but they point in opposite directions
on "does motionlil beat Motion," which is exactly the kind of disagreement this task asked to be
surfaced rather than silently resolved in the port's favor.

## markedlil: no genuine same-version official artifact exists

marked stopped shipping a pre-built `marked.min.js` after roughly v15 (bundler-based consumption is
now the recommended path). `markedlil/node_modules/marked` (the exact matching pin, 18.0.10) has
only unminified `lib/marked.esm.js` (43018 bytes) and `lib/marked.umd.js` (43897 bytes) — no min
file. The number used in row 3 of the master table (marked 14.0.0's real, checked-in `marked.min.js`
at `lilscript/benchmarks/popular/node_modules/marked/marked.min.js`, 10085 Brotli) is
version-mismatched but matches this task's own "known context" figure exactly, and is corroborated
independently: `markedlil`'s own harness produces a same-version (18.0.10) Oxc-mangled
parse-only reconstruction at `markedlil/.tmp/lanes/parse-oxc-mangle.js`, measured directly with
`lilscript-codec` at **10092 Brotli** — within 7 bytes of the version-mismatched real artifact, and
still a clear WIN either way (9652 < 10085 and < 10092). Note that reconstruction itself excludes
marked's `use()`/Hooks/`walkTokens` API surface, so it undercounts the true official baseline
somewhat; the win margin is likely a little smaller than either number alone suggests, but the
direction (WIN) is not in doubt.

---

## Anomalies

1. **jquerylil regression** — see dedicated section above. The single most concrete, high-confidence
   finding in this pass: the current uncommitted `dist/jquery.esm.js` is 3258 Brotli bytes worse than
   the last git commit of the same file.
2. **rehype and remark-gfm verdict flips** — see dedicated section above. REPORT.md currently shows
   both as losses; both are wins under a same-instant fresh measurement.
3. **`katexlil/dist/katex.min.js` names itself after the file it should be compared against** but is
   actually the Lil port's own output. Anyone automating this comparison by filename alone would
   silently compare the port against itself.
4. **Nearly every port's `dist/` is uncommitted** — `git status` at the repo root and in essentially
   every sibling port shows modified/untracked `dist/*` files, consistent with an in-progress,
   unfinished compiler change (many `src/*.rs` files modified in `lilscript/` itself, right now).
   This measurement reflects that in-progress on-disk state, per the task's instruction to measure
   checked-in artifacts without recompiling — but it means every number in this report is a snapshot
   of a moving target, not a stable release state.
5. **remark's own official-baseline figure doesn't match REPORT.md's for the same npm package** —
   32551 Brotli (REPORT.md, pinned codec) vs 22770 Brotli (remarklil's own fresh self-report, Node
   codec) for what should be the same "official remark@15.0.1 graph, Terser-mangled." A 10KB gap on
   a side that shouldn't have moved (the npm package didn't change) suggests the two harnesses bundle
   a different scope of remark's dependency graph, not just an encoder difference. Unresolved; flagged
   rather than guessed at.
6. **monacolil's worker bundle** is 3885 bytes where the official package's equivalent is 9.6MB — a
   2500x gap that means the comparison is not measuring the same feature set, not that LilScript found
   a 2500x compression win. See "Not comparable" above.
7. **playcanvaslil** is the only port where "official" and "Lil" artifacts live side-by-side, already
   built, in the exact same `dist/` directory as pre-existing sibling files — the only case in this
   whole scoreboard where zero interpretation was required to find a legitimate, scope-matched
   baseline. Its win margin is also the thinnest of any WIN in the table (53 Brotli bytes out of
   ~14.6KB, 0.36%) — worth treating as a near-tie for planning purposes even though it's a WIN.

---

## Exact commands used

```sh
/home/azureuser/lilscript/target/release/lilscript-codec --json <file1> <file2> ...
```

Every path cited in the master table and the prose above is an absolute path already given inline;
none were recomputed or re-derived — each was read directly off disk and measured with the command
above. No `node`, `esbuild`, `terser`, `vite`, or `oxc` invocation was made by me at any point in
this pass; where those tools' *prior* output happened to still be cached on disk (e.g.
`markedlil/.tmp/lanes/parse-oxc-mangle.js`, `rehypelil/_site/official.js`,
`remark-gfmlil/_site/official.js`), I measured the existing file with `lilscript-codec` directly
rather than regenerating it.

---

# Correction — the jquerylil row measures a regressed working tree

Row 1 measures `jquerylil/dist/jquery.esm.js` **as it currently sits in the working tree**, which is
a modified, uncommitted file. Measuring the committed version instead changes the verdict's size by a
factor of four:

| `jquerylil/dist/jquery.esm.js` | raw | gzip-9 | Brotli-11 | vs official |
|---|---:|---:|---:|---:|
| working tree (uncommitted, what row 1 used) | 95435 | 35224 | 31483 | +4038 |
| **`git show HEAD:dist/jquery.esm.js`** | **83044** | 31530 | **28225** | **+780** |
| official `jquery.min.js` (jquery 3.7.1) | 87533 | 30336 | 27445 | — |

Three things follow, and they matter more than the row itself:

1. **The real jQueryLil gap is +780 Brotli, not +4038.** It is still a LOSS, but a near-tie rather
   than a rout.
2. **jQueryLil already beats official on raw by 4489 bytes** (83044 vs 87533). It is emitting *less
   code* than Terser and losing only on how well that code compresses — see the census in
   [008](../008-jquery-compressibility-gap/README.md), which found 8-gram uniqueness 0.703 against
   Terser's 0.643 and semicolon density at **2.1x**. That is a single, well-localized mechanism:
   surplus statements carrying surplus names.
3. **There is a live 3258-byte Brotli regression uncommitted in `jquerylil/dist/`.** Those files were
   written at 03:43 on 2026-08-31; the `auto-finer-lilscript` workstream started at 08:14, so this
   is not from those compiler changes. It should be investigated before it is committed.

Reproduce with:

```sh
git -C ~/jquerylil show HEAD:dist/jquery.esm.js > /tmp/head.js
~/lilscript/target/release/lilscript-codec --json \
  ~/jquerylil/dist/jquery.esm.js /tmp/head.js \
  ~/lilscript/benchmarks/popular/node_modules/jquery/dist/jquery.min.js
```

The aggregate "+32,007 bytes" total above therefore overstates the true deficit by at least 3258
bytes from this row alone, before the rehype/remark-gfm verdict flips are folded in.
