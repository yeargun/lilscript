# 006 — Why the markdown-stack losers lose

**Status: DIAGNOSIS COMPLETE. No code changed (read-only task). Two concrete, near-zero-cost fixes
identified; several structural (harness-methodology) gaps identified separately from genuine
compiler gaps.**

## Hypothesis / question

`comparison/markdown-stack/REPORT.md` shows 6 wins / 10 losses, +28435 Brotli bytes total. For the
worst losers (react-markdown +16769, remark +13329, rehype +9912, unified +984) the Lil **raw**
output is 1.3–1.8x Terser's raw output, which is too large a gap to be a codegen-margin problem —
it suggests the two sides are not always measuring the same program. Question: for each of the top
losers, is the loss a genuine compiler/source gap ("apples-to-apples"), or does the Lil graph
contain code/formatting the official Terser graph does not ("not comparable")? And do the 6
winners vs 10 losers differ systematically in config?

## Method

Read-only. No `cargo build`, no `.lil` recompiles. Inspected:

- `comparison/markdown-stack/README.md`, `manifest.json`, `run.mjs` — harness contract and build
  logic.
- `comparison/markdown-stack/REPORT.md` — the scoreboard being explained.
- `comparison/markdown-stack/.work/<port>/lil-graph.js` and `official-terser.js` — the **exact
  artifacts that produced the REPORT.md numbers** (verified: recompressing each with Node's
  `zlib.brotliCompressSync` at quality 11 / lgwin 22 reproduces REPORT.md's Brotli column exactly,
  e.g. `.work/rehype/lil-graph.js` → 64992, matching the report; this validates every isolated
  measurement below against the harness's own codec).
- `lilscript.toml` for all 16 sibling ports at `/home/azureuser/<port>lil/lilscript.toml`.
- `/home/azureuser/<port>lil/scripts/build.mjs` (the port's own esbuild bundling step) and
  `package.json` (`dependencies`, `exports`) for all 16 ports.
- `docs/knowledge/compilation/candidate-search.md`, `docs/configuration.md`,
  `docs/knowledge/config/javascript-priority.md`, `docs/knowledge/config/optimization.md` for what
  the differing config knobs actually do.

Sibling port repos live at `/home/azureuser/<name>lil/` (siblings of `lilscript/`, resolved via
`manifest.json`'s `port.defaultSibling: "../<name>lil"` relative to `comparison/markdown-stack/`),
**not** inside `comparison/`.

**A caveat surfaced during this diagnosis**: several sibling dist files on disk right now (e.g.
`rehypelil/dist/rehype.esm.js`, currently 243694 bytes; `rehype-katexlil/dist/rehype-katex.esm.js`,
currently 2466 bytes with `katex` marked `external`) do **not** match the `.work/` artifacts that
produced REPORT.md (e.g. `.work/rehype/lil-graph.js` is 372587 bytes; `.work/rehype-katex/lil-graph.js`
is 586338 bytes and has KaTeX's tables bundled in, no `external` import). The repo's top-level git
status also shows uncommitted changes across the compiler (`src/codegen_ir_js.rs`, `src/lower.rs`,
`src/optimizer.rs`, etc.) and across every `labs/vue-client` build script, so active work is
in flight elsewhere on this host. **All figures below use `.work/*` (the artifacts that actually
produced REPORT.md), not the live `dist/` files**, per the task's instruction to read checked
artifacts rather than recompile. This drift is itself flagged in the follow-up list.

## Config diff table: winners vs losers

All 16 `lilscript.toml` share `priority = "size-first"`, `cost_model = "brotli"`,
`assume_pristine_builtins = true` (except unified: `false`), `strip_console = true`,
`mangle.identifiers/properties/extern_fields = true`, `mangle.exports = false`. The columns that
actually differ:

| Port | Result | `optimization_level` | `candidate_search` | `function_spelling` | Other |
|---|---|---:|---|---|---|
| micromark | loss | 13 | `production` | *(omit → non-arrow)* | `local_name_reserve=24` |
| mdast-util-from-markdown | loss | 13 | `production` | *(omit)* | `pool_strings=false` |
| remark-parse | loss | 13 | `production` | *(omit)* | |
| remark | loss | 13 | `production` | *(omit)* | `[optimization] identical_function_folding=false` |
| unified | loss | 13 | `production` | `"function"` | `assume_pristine_builtins=false`, `local_name_reserve=48` |
| react-markdown | loss | 13 | `production` | `"function"` | `local_name_reserve=48` |
| mdast-util-to-hast | **win** | 15 | `always` | `"arrow"` | |
| remark-rehype | **win** | 15 | `always` | `"arrow"` | |
| remark-breaks | **win** | 15 | `always` | `"arrow"` | |
| hast-util-to-html | **win** | 12 | `off` | `"arrow"` | |
| rehype-stringify | **win** | 12 | `off` | `"arrow"` | |
| rehype-katex | **win** | 6 | `off` | `"arrow"` | |
| rehype | loss | 12 | `always` | `"arrow"` | `assume_pure_property_reads=true` |
| remark-gfm | loss | 15 | `always` | `"arrow"` | |
| remark-math | loss | 15 | `always` | `"arrow"` | |
| katex | loss | 13 | `always` | `"arrow"` | `terminal_codec_probe_limit=256`, `pool_strings=false` |

**The one config split that is a perfect (if confounded) predictor**: the six ports using
`candidate_search = "production"` — micromark, mdast-util-from-markdown, remark-parse, remark,
unified, react-markdown — are **exactly** the six that never override `function_spelling` to
`"arrow"` (unified and react-markdown say `"function"` explicitly; the other four leave it at the
default, which `docs/knowledge/config/javascript-priority.md:51` documents as "public functions
stay constructible" — i.e. non-arrow). That group is **0 wins / 6 losses**. The other ten ports all
explicitly set `function_spelling = "arrow"` (public exports drop `new`/`prototype`) and split
**6 wins / 4 losses**.

This is a real, cheap, testable lead — but it is **confounded**, not a clean causal story. Two
things argue against "just flip these knobs":
1. `candidate_search` itself does not move monotonically with wins: the `"off"` group (least
   search — one configured emission, per `docs/knowledge/compilation/candidate-search.md:107-111`)
   wins 3/3 (hast-util-to-html, rehype-stringify, rehype-katex), while the `"always"` group (most
   search) only wins 3/7 (rehype, remark-gfm, remark-math, katex all lose *despite* `"always"`).
   More search is not obviously buying wins on its own.
2. The `"production"` group is exactly the six **largest/most complex** graphs in the set (parsers,
   tokenizers, or — for remark/unified/react-markdown — bundled multi-package graphs; see below).
   It is at least as plausible that `candidate_search`/`function_spelling` were left at defaults on
   these six *because* they are expensive to iterate on (objective.md's "compilation takes very
   long time" complaint), not that the default config is what causes the loss. Both readings are
   live; distinguishing them needs an actual experiment (see follow-ups), which was out of scope
   here (no recompiles).

## Per-port findings

### react-markdown (+16769 Brotli, worst loss) — **not comparable**

`react-markdown` is one of three ports in `manifest.toolchain.graph.lilBundlePorts` (`run.mjs:625`,
`manifest.json`), meaning the Lil side is **not** a single LilScript compilation. It is five
*separately* LilScript-compiled sibling artifacts (`unifiedlil/dist/vfile.esm.js`,
`unifiedlil/dist/unified.esm.js`, `remark-parselil/dist/remark-parse.esm.js`,
`remark-rehypelil/dist/remark-rehype.esm.js`, `react-markdownlil/dist/react-markdown.esm.js`),
each already independently mangled/minified with its **own** short-identifier alphabet and its
**own** fully-preserved public export surface (because each is *also* independently shipped and
measured as its own port), that `run.mjs`'s `buildGraphs()` then hands to esbuild with
`bundle: true` and **no minify options at all** (`manifest.json` → `toolchain.esbuild.options` has
no `minifyWhitespace`/`minifyIdentifiers`/`minifySyntax` keys, and the harness's own comment at
`run.mjs:989` says "no Lil graph is post-minified" — by design). The official side, in contrast,
gets ONE esbuild bundle of the full **un-minified npm source graph**, which is then run through ONE
Terser pass (`module:true, compress:true, mangle:true`) that sees the entire program at once.

Byte-bucketing `.work/react-markdown/lil-graph.js` (216436 bytes, matches REPORT.md exactly) by
esbuild's per-file `// path` banners:

| Chunk | Bytes | Brotli-11 (isolated) |
|---|---:|---:|
| `remark-parselil/dist/remark-parse.esm.js` (as bundled) | 144587 | 30251 |
| `react-markdownlil/dist/react-markdown.esm.js` | 26936 | 8247 |
| `remark-rehypelil/dist/remark-rehype.esm.js` (as bundled) | 22941 | 5104 |
| `unifiedlil/dist/vfile.esm.js` (as bundled) | 11669 | 2795 |
| `unifiedlil/dist/unified.esm.js` (as bundled) | 10299 | 2868 |

Compare each *as-bundled* chunk to its own **standalone** measurement (same file, but reached via
`copyFileSync` with zero esbuild reprocessing, per `run.mjs:628`):

| Chunk | Standalone raw / Brotli | As-bundled-in-react-markdown raw / Brotli | Inflation |
|---|---:|---:|---:|
| remark-parse | 95544 / 27021 (`.work/remark-parse/lil-graph.js`) | 144587 / 30251 | **+49043 raw / +3230 Brotli** |
| remark-rehype | 15027 / 4390 (`.work/remark-rehype/lil-graph.js`) | 22941 / 5104 | **+7914 raw / +714 Brotli** |
| vfile+unified | 21316 / 5409 (`.work/unified/lil-graph.js`) | 21968 / 5663 | +652 raw / +254 Brotli |

Just from re-parsing/re-printing already-mangled, already-compact code (each already-compact
single/few-line Lil artifact gets exploded into thousands of pretty-printed lines — verified by line
count: the remark-parse chunk goes from a 2-line compact artifact to 3876 lines once esbuild
re-prints it) **plus** deconflicting colliding single/double-letter top-level identifiers across the
five independently-mangled modules (visible directly in the reprinted source: `a2`, `a3`, `j3`, `b3`
— numeric suffixes esbuild appends when two Lil modules independently picked the same short name),
react-markdown's Lil graph picks up roughly **+57600 raw / +4200 Brotli bytes that have nothing to
do with LilScript's codegen quality** — that is, isolated estimates already account for ~25% of the
total +16769 Brotli loss before considering the remaining structural gap: **none of the five bundled
modules gets to tree-shake against the others.** Each preserves its full public export surface
(`mangle.exports = false` in every `lilscript.toml`, by design, since each is independently shipped)
so unused exports of `remark-parse`/`remark-rehype`/`unified` that react-markdown itself never calls
cannot be eliminated — exactly what Terser's single whole-graph pass does for the official side.

**Verdict: not comparable.** The measured "loss" is real bytes-on-the-wire, but it is not testing
LilScript codegen against Terser codegen on the same program; it is testing "N independently-mangled
Lil artifacts naively concatenated, unminified" against "1 whole-program Terser pass." The harness
explicitly disclaims post-minification of Lil graphs as a design choice (`run.mjs:989`), so this is
a known, documented methodology gap for the three `lilBundlePorts`, not a hidden bug — but it means
the +16769 number should not be read as "LilScript's compiler is 1.6x worse than Terser's" for this
port.

### remark (+13329 Brotli, 2nd-worst loss) — **not comparable, plus one real source-side bug**

Also a `lilBundlePorts` member (2 files: `remarklil/dist/remark.esm.js` +
`node_modules/vfile/**` — see below). Same reprint/rename mechanism as react-markdown, sized
directly:

- `remarklil/dist/remark.esm.js`, as it appears inside `.work/remark/lil-graph.js`: 178799 raw /
  41480 Brotli (isolated). The current on-disk compact form of the same file,
  `/home/azureuser/remarklil/dist/remark.esm.js` (124929 bytes, 2 lines): 124929 raw / 36959 Brotli.
  **+53870 raw / +4521 Brotli** purely from esbuild's non-minifying reprint of one already-compact
  file (sizes are close enough — 124929 vs the ~178806 the arithmetic implies — that this is a
  reliable proxy even though the on-disk file may have moved slightly since the report).

- **Separately, and unlike every other port**, `remarklil/package.json:3` declares a real npm
  runtime dependency: `"dependencies": {"vfile": "^6.0.0"}`. `run.mjs`'s `graphAliases()`
  (`run.mjs:554-570`) only rewrites import specifiers for `port.id === 'react-markdown'`; there is
  no equivalent alias for `remark`. So when `remark.esm.js`'s `import ... from 'vfile'` resolves
  through esbuild's bundler, it resolves to the **real, unminified npm `vfile` package** sitting in
  `remarklil/node_modules/vfile` — not to `unifiedlil`'s own LilScript-authored, already-mangled
  vfile port (`unifiedlil/dist/vfile.esm.js`), which **does exist** and is already being used
  correctly by react-markdown's bundle via the `@itslil/unified/vfile` alias
  (`manifest.json` → `toolchain.reactGraph.portAliases`). Measured directly:

  | | raw | Brotli-11 (isolated) |
  |---|---:|---:|
  | Real npm `vfile`+`vfile-message`+`unist-util-stringify-position` bundled into remark | 26501 | 4620 |
  | `unifiedlil/dist/vfile.esm.js` (the Lil-authored equivalent, already exists) | 11509 | 2799 |

  **+14992 raw / +1821 Brotli** for pulling in the real npm package instead of the sibling port's
  own compiled replacement.

Between the two isolated effects (~4521 + 1821 = ~6342 Brotli, order-of-magnitude, not strictly
additive under joint compression) roughly half of remark's +13329 Brotli loss is explained by (a) the
same bundling-reprint tax as react-markdown and (b) an actual, fixable dependency-resolution gap
that is specific to `remark` — react-markdown's harness config already shows the fix (`vfile` should
resolve to `unifiedlil`'s port, same as react-markdown does).

**Verdict: not comparable** for the bundling-reprint portion (same reasoning as react-markdown); the
`vfile` dependency piece is closer to **apples-to-apples loss, but attributable to `remarklil`'s
package.json / the harness's alias config rather than to LilScript codegen** — this is exactly the
"loss may be the LilScript source's fault" case objective.md rule 6 calls out, just at the
package-dependency layer instead of inside a `.lil` file.

### rehype (+9912 Brotli, 3rd-worst loss) — **apples-to-apples loss, but with a one-line, non-compiler fix available**

`rehype` is **not** in `lilBundlePorts` — its Lil lane is a byte-for-byte copy of
`rehypelil/dist/rehype.esm.js` (`run.mjs:628`, the `!lilBundled` branch), so this is a genuine
single-program-vs-single-program comparison; no graph-composition issue. `.work/rehype/lil-graph.js`
(372587 bytes, matches REPORT.md) is entirely LilScript-authored code — its two internal markers
(`.tmp/build/parse5/index.js`, `.tmp/build/rehype.raw.js`) are the compiler's own output for
`rehypelil/src/index.lil`, compiled as one unit (confirmed against `rehypelil/scripts/build.mjs`,
which invokes the compiler once on `src/index.lil` and writes the raw output to
`.tmp/build/${file}.raw.js` before any esbuild step — there is no real npm `parse5` in the runtime
graph; `parse5` only appears as a devDependency used by `check-parse5-*.mjs` test/corpus scripts).

**But**: `rehypelil/scripts/build.mjs:64` calls esbuild's `bundleCompiled()` with
```js
minifyWhitespace: format != "esm",
```
Every other port in this stack (checked: micromark, mdast-util-from-markdown, remark-parse, remark,
unified, mdast-util-to-hast, hast-util-to-html, remark-rehype, rehype-stringify, remark-gfm,
remark-breaks, remark-math, katex, react-markdown) unconditionally passes
`minifyWhitespace: true` for its ESM build. `rehypelil` is the only one that special-cases
`format === "esm"` to **skip** whitespace minification — meaning the file the harness treats as the
canonical, already-optimal Lil artifact (`dist/rehype.esm.js`) is shipped **pretty-printed**, not
compacted.

Verified directly by running only `esbuild.transform({minifyWhitespace: true, minifyIdentifiers:
false, minifySyntax: false})` (no other change — same identifiers, same syntax) over the checked-in
`.work/rehype/lil-graph.js`:

| | raw | Brotli-11 |
|---|---:|---:|
| As measured (REPORT.md) | 372587 | 64992 |
| Same file, whitespace-only minified | 289086 | 59559 |
| **Delta from this one flag** | **-83501 (-22.4%)** | **-5433 (-8.4%)** |

rehype's total loss vs official Terser is +9912 Brotli. Fixing this one line in
`rehypelil/scripts/build.mjs` (or wiring the harness to whitespace-minify standalone Lil artifacts
the same way it currently lets `rehype-katexlil`'s build script do — see caveat below) would cut the
loss to roughly **+4479 Brotli**, more than halving it, with **zero compiler or `.lil` source
changes**.

Caveat: `rehype-katexlil/scripts/build.mjs:97` *also* sets `minifyWhitespace: false` for its ESM
build and that port is the largest **winner** in the set (-28562) — so whitespace minification is
not sufficient on its own to flip a loss to a win, and it is not the case that every unminified ESM
artifact loses. It just means rehype is leaving real, free bytes on the table that other ports are
either not leaving (because their own build scripts do minify whitespace) or are compensating for
by a wide margin elsewhere.

**Verdict: apples-to-apples loss** (single program vs single program, no dependency leakage), but a
meaningful fraction of the measured gap (>50%) is a `rehypelil`-build-script defect, not a LilScript
codegen deficiency. The remaining ~+4479 Brotli (after the whitespace fix) would be the real
compiler/source signal to chase next.

### unified (+984 Brotli, smallest of the bundled-port losses)

Also in `lilBundlePorts` (2 files: `vfile.esm.js` + `unified.esm.js`). Same mechanism as
react-markdown/remark but at much smaller scale because only two independently-mangled modules
collide: bundled-vs-standalone comparison shows only +652 raw / +254 Brotli (isolated) of
reprint/rename tax — roughly a quarter of the total +984 Brotli loss. The rest is unexplained by
this diagnosis; candidate causes not run down here: unified's `assume_pristine_builtins = false`
(the only port in the set with this off — `unifiedlil/lilscript.toml`), `function_spelling =
"function"` (default/non-arrow, in the `candidate_search=production` group), or a genuine
vfile-API-surface-retention cost (vfile's full public surface must be kept since unified's own
public API re-exports it). **Verdict: partially not comparable** (small bundling tax measured
directly) **plus unknown remainder**.

### katex (+6532 Brotli) — likely apples-to-apples, not deeply investigated

Standalone (`0` runtime imports in `dist/katex.mjs`; `commander` in `package.json` is a
build-tool devDependency, not a runtime one). `.work/katex/lil-graph.js` is 6 lines — already
compact, no `minifyWhitespace` bug (its `build.mjs` sets `true` throughout). `lilscript.toml` already
carries a hand-tuned `terminal_codec_probe_limit = 256` with a comment recording an earlier probe
that moved this exact port from 63146→62505 Brotli, so this port has already had at least one
config-tuning pass. Raw ratio is a comparatively mild 1.11x (267745→295886), unlike the 1.6-1.8x
ratios of react-markdown/remark. **Verdict: unknown, leaning apples-to-apples** — no graph-
composition or build-script issue found; the gap is plausibly a genuine codegen/source gap on
KaTeX's large parsing/rendering tables, not chased further here (task scope caps at top-3 losers in
depth).

### micromark, mdast-util-from-markdown, remark-parse, remark-gfm, remark-math (small/mid losers)

All five are standalone (0 runtime imports; `@types/*`/`micromark-util-types` deps are types-only),
so none has react-markdown/remark's graph-composition problem. None of their `build.mjs` scripts has
rehype's `minifyWhitespace` bug (all confirmed `true`). micromark, mdast-util-from-markdown, and
remark-parse are the three smallest of the six `candidate_search = "production"` /
non-arrow-`function_spelling` group described above. remark-gfm and remark-math are in the
`"always"`+arrow group along with 3 winners and rehype/katex, so their losses (+379, +450 — the two
smallest in the whole set) are not explained by the config split at all. **Verdict: apples-to-apples,
unknown root cause** for all five — no measurement artifact found; likely genuine, currently-small,
codegen/source gaps.

## Verdict summary

| Port | Delta | Verdict |
|---|---:|---|
| react-markdown | +16769 | **not comparable** — bundling of 5 independently-mangled Lil artifacts, no cross-module tree-shaking or minification (harness design, `lilBundlePorts`) |
| remark | +13329 | **not comparable** (bundling tax) **+ apples-to-apples source/config bug** (real npm `vfile` instead of the sibling Lil port) |
| rehype | +9912 | **apples-to-apples**, but >50% is a `rehypelil/scripts/build.mjs` whitespace-minify bug, not codegen |
| katex | +6532 | **unknown, leaning apples-to-apples** — not deeply chased |
| micromark | +4568 | **apples-to-apples, unknown** |
| remark-parse | +3738 | **apples-to-apples, unknown** |
| mdast-util-from-markdown | +3573 | **apples-to-apples, unknown** |
| unified | +984 | **partially not comparable** (small bundling tax measured) **+ unknown remainder** |
| remark-math | +450 | **apples-to-apples, unknown** |
| remark-gfm | +379 | **apples-to-apples, unknown** |

## Ranked follow-up actions, cheapest first

1. **Fix `rehypelil/scripts/build.mjs:64`**: change `minifyWhitespace: format != "esm"` to
   `minifyWhitespace: true` (matching every other port's build script). Zero compiler risk, zero
   semantic risk (whitespace-only). Measured effect: -83501 raw / -5433 Brotli on the exact checked
   artifact, cutting rehype's loss from +9912 to roughly +4479. Re-run
   `node comparison/markdown-stack/run.mjs --measure` afterward to confirm and update REPORT.md.
2. **Give `remark` the same `vfile` alias react-markdown already has.** Either add `remark` to
   `manifest.toolchain.reactGraph`-style alias handling in `run.mjs`'s `graphAliases()` (currently
   gated to `port.id === 'react-markdown'` only, `run.mjs:555`) so `vfile` resolves to
   `unifiedlil/dist/vfile.esm.js`, or change `remarklil`'s own `.lil` source / build to depend on
   the sibling Lil vfile port instead of declaring `"vfile": "^6.0.0"` as an npm runtime dependency
   in `remarklil/package.json`. Measured effect: ~-14992 raw / -1821 Brotli (isolated estimate).
3. **Re-run the measurement harness before trusting the current rehype-katex and rehype numbers.**
   Live `rehype-katexlil/dist/rehype-katex.esm.js` (2466 bytes, `katex` marked external) and
   `rehypelil/dist/rehype.esm.js` (243694 bytes) already disagree substantially with the `.work/`
   artifacts that produced REPORT.md (586338 and 372587 bytes respectively). Confirm what changed
   and whether it's a legitimate improvement or an accidental scope change (e.g. rehype-katex
   excluding all of KaTeX from its own measured artifact would make its win look far larger than it
   really is, and would itself need a "not comparable" verdict against the official side, which
   still bundles real npm katex).
4. **Run the `function_spelling = "arrow"` + `candidate_search = "always"` experiment on one of the
   six `"production"`/non-arrow losers** (remark-parse is the best test subject: standalone, no
   bundling confound, mid-sized so compile time is bounded) to determine whether the config split
   found in the diff table above is causal or just correlated with "these are the graphs nobody has
   tuned yet." If arrow spelling is semantically safe for these public entries (worth checking:
   none of remark-parse/micromark/mdast-util-from-markdown's public exports appear to require `new`
   — they are plain factory/parse functions per the upstream unified ecosystem's own conventions,
   but this was not verified against each `.lil` source in this pass), this is a config-only change
   with no compiler work required.
5. **For react-markdown/remark/unified specifically, decide what "beat Terser" should even mean
   for a `lilBundlePorts` graph.** Two honest options: (a) accept the current harness rule (no
   Lil-side post-minification) as the metric and treat these three losses as expected/structural
   rather than compiler bugs, since Terser's single-pass whole-graph optimization is doing
   fundamentally more work than concatenating N independently-optimized artifacts ever can; or
   (b) change the harness so that for these three ports, the *inputs* to a single LilScript
   compilation are the union of the relevant `.lil` sources (remark + its runtime deps compiled as
   one program, the way `rehype-katexlil`+`katexlil` already appear to be co-compiled for the
   **winning** rehype-katex artifact — see the caveat above), which would let LilScript's own
   whole-program optimizer do the cross-module dead-code elimination and identifier-alphabet sharing
   that Terser gets "for free" from bundling raw source before minifying. (b) is the only real fix
   to the >16000/>13000 Brotli losses; (a) just relabels them as out-of-scope. This is a design
   decision, not something to guess at further in a diagnostic pass.
6. **Chase the remaining, currently-unexplained losses** (katex +6532, micromark +4568,
   remark-parse +3738, mdast-util-from-markdown +3573, and the ~+150 residual unified after its
   bundling tax) with real profiling once 1-2 land, the same way
   `auto-finer-lilscript/004-peephole-relex-tax` profiled jQuery — these were out of this task's
   "top 3" scope and no artifact-bucketing was done for them beyond confirming they are apples-to-
   apples single-program comparisons with no build-script or dependency-leakage bug.
