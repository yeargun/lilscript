# Status

Standings, settled facts and ranked leads for the [finer loop](README.md). Volatile: rewritten
after every fleet measure and every verdict. Contract: [objective.md](objective.md). History:
[log.md](log.md). Updated 2026-09-01 after 037.

## Standings

Last full fleet measure: 2026-09-01 18:15, working tree, `lilscript-codec`, Brotli-11 (every port
declares `cost_model = "brotli"`). It predates the 034 charset fix landing in two dists, so those
two rows carry their post-fix numbers from the folders. Regenerate with
`node finer/tools/fleet.mjs --measure`.

**11 wins / 11 losses**, net about −89 KB Brotli. `src` is the count of modified source files in the
port at measurement: how much of the number is *not* the compiler.

| port | delta | src | attribution |
|---|---:|---:|---|
| react-markdownlil | +14166 | 19 | `terminal_codec_probe_limit = 0` hides a miscompile worth −3364 (037); the committed artifact was React-external glue and the tree inlines it — not the same program |
| motionlil | +9314 | 0 | fails to build under the fleet (`ERR_MODULE_NOT_FOUND`); baseline scope-suspect: `dist/full.js` against the real motion UMD |
| remarklil | +6752 | 46 | bundles unminified npm `vfile` instead of the sibling port (~1821, 006); micromark core |
| katexlil | +5800 | 11 | unexamined |
| micromarklil | +3321 | 51 | post-034 (26097). Micromark core: emitted volume; Terser still extracts −884 from our artifact (035) |
| remark-parselil | +2922 | 36 | micromark core |
| mdast-util-from-markdownlil | +2824 | 33 | micromark core — the three share it, so one fix moves ~9 KB |
| **mobxlil** | **+2771** | **0** | **clean source, real loss.** Was +3007 before 033; Terser extracts −856 from our artifact (035) |
| **jquerylil** | **+1825** | **0** | **clean source, real loss.** Beats upstream raw by 4489 and loses on compressibility; −540 is the plain-data type (013), −1136 is rename headroom (035) |
| unifiedlil | +234 | 6 | config exhausted (027); emits 47% more functions, each 29% bigger |
| remark-mathlil | +137 | 9 | config exhausted (027) |

Wins: rehype-katex −112118 (scope-suspect), zod −20103, rehype −2717 (post-034), hast-util-to-html
−1014, rehype-stringify −794, mdast-util-to-hast −752, remark-rehype −687, marked −579, remark-gfm
−402, remark-breaks −67, posthog −1.

No baseline and failing to build under the fleet: lil-solidjs, monacolil, playcanvaslil, solidlil.

Clean sources: jquerylil, markedlil, mobxlil, motionlil, posthoglil, zodlil. The other 16 ports are
mid-migration, so their numbers confound source, config and compiler (014).

## Objective lanes

Every port scores Brotli. Only markedlil ships raw (`marked.raw.js`), gzip (`marked.gzip.js`),
`bytes` and closed-world (`marked.closed.js`, `extern_fields = false`) builds; jquery keeps
`lilscript.public.toml` and `lilscript.app.toml` for open versus closed. The objective-purity check
(objective.md §2) has not yet been run as a standing task. SWC is named as a baseline and has no
pinned lane; Terser, Oxc, esbuild, Vite and Closure do
([baseline toolchains](../docs/knowledge/verification/baseline-toolchains.md)).

## Settled — not re-litigated without a new fact

- **Level 13 is the default because it is the best trade, not the best bytes** (007, 009). Level
  15 is 20x the CPU for 1.4%; the curve differs per port (remark-math prefers 15, unified 13,
  jquerylil earns 15 with `always` at 23x CPU for 4.3%). Measure per port; never transfer a curve.
- **Config tuning is exhausted on the near misses** (027, 029). Twenty builds buy ten bytes that
  fail tests; beam width is worth 47 on posthog and 0 on unified; every anti-cloning switch makes it
  bigger; `region-outlining` changes nothing on jquerylil.
- **We lose on volume, not on compressibility** (025). Raw excess predicts the loss at r=+0.92 with
  perfect separation; repeat coverage predicts nothing; `JsValue` density correlates −0.09.
- **Name allocation is not a defect** (035, 036). The disjoint module/local pools give 62 of 63
  top-level bindings two-character names; forcing shadowing on costs +432 because local occurrences
  outnumber module ones tenfold. `local_name_reserve` is at its optimum. What Terser's `mangle`
  still extracts from our finished artifacts (−884 / −856 / −1136) is unexplained past that.
- **A pure reprint of our output saves 0 bytes** (035). The printer is not the gap.
- **The admission gate is load-bearing** (018): without it the output is unparseable. Two of its
  validators refused the class rewrite by mistake (031, 032) and its scope model missed member
  bodies (033); the rewrite now lands on mobxlil (−235).
- **Between compiler and artifact is the first place to look** (006, 028, 030, 034). 13380 Brotli
  of reported loss was harness or build, not codegen, plus 1093 from escaped non-ASCII.
- **Specialisation, CSE and oxc's declaration merge are not levers** (017, 029). Implemented or
  toggled, measured at zero or worse.
- **Terser still finds −1498 `;` / +932 `,` / −299 `if(` / −344 `var ` in our micromark output**
  (013, 035). The statement-boundary transforms (`e;return x` → `return e,x` and four siblings) are
  absent; each is raw-neutral alone and pays only by letting other folds reach across.

## Landed by this workstream

Effort telemetry (`src/timing.rs`); content-addressed memos (−27% CPU, byte-identical); lexer
cache; fixed-point caps; default 15 → 13 with the probe-ladder retune, after which 13 beats 15 on
jQuery; class-expression terminators and the `?.` floor (023, 024); member bodies as scopes (033);
constructor-exempt admission on re-checked winners (031); bundler charset (034: −27689 raw / −1093
Brotli on two ports); build-script fixes on rehypelil and micromarklil (006, 030); the harness
minifies both lanes (028). Reverted after measurement: string-pool alias pricing (+282 on
jquerylil), the narrowed `unstable` closure (+69 net), the levels-14/15 probe raise (3x CPU for 192
bytes).

## Open leads, ranked by measured value

1. **037 — `identifier_is_read_after` false negative** drops `alias=Name.prototype`; fixing it lets
   react-markdownlil drop `terminal_codec_probe_limit = 0` for **−3364 Brotli**. A candidate fix is
   in the working tree (`src/js_peephole/folds/classes.rs`), unverified: run the suite and the
   port's 120 tests, then re-measure. Same family as 033; add the missing-alias shape to the paired
   corpus.
2. **A plain-data object type** (013): −540 on jquerylil, 69% of its gap, without the
   `pure_getters` rules change. Language work; the largest single measured win found.
3. **Statement-boundary absorption** (035): the five `sequencesize_2` shapes, worth 95–359 per port
   only through the folds they unblock. Measure as a portfolio change.
4. **Rename headroom** (035): Terser extracts −1136 from jquerylil's finished artifact although
   allocation is already optimal there. Locate what its `mangle` does that ours does not.
5. **Pooling benefit model** (011): `count * length` is a raw-objective formula; under Brotli the
   repeats were already matches. About −35 on jquerylil and should generalize to every Brotli port.
6. **Budget allocation** (009, 036): 46 of 47 families starve at micromarklil's shipped config while
   the incumbent survives; `begin_fair_slice` splits evenly across families with very different hit
   rates.
7. **Markdown-stack composition** (006): remarklil bundles npm `vfile` (~1821); the three
   `lilBundlePorts` concatenate independently compiled modules.
8. **Fleet hygiene**: five ports fail to build under the fleet; motionlil's baseline needs a scope
   decision; sixteen ports need their sources committed before their numbers are compiler
   measurements; katexlil is unexamined at +5800. `shipped-vs-compiled` fails on rehype-katexlil
   (2026-09-01): its build re-prints every `!0`/`!1` as `true`/`false`, the 030 class — five sites,
   a small fix, and the gate stays red until it lands.
9. **Objective purity** (objective.md §2): run the three-way build on markedlil and one micromark
   port; give a second port raw and gzip configs so the check is not one port's.

## Known issues

- `js_peephole::tests::rejects_generated_syntax_above_the_configured_floor` fails at clean HEAD:
  the syntax floor does not reject ES2022 class fields under an ES2021 target.
- `--explain` on the jQuery port fails with "selected JavaScript candidate changed the normalized
  ABI manifest" while the plain compile succeeds.
- `comparison/markdown-stack/REPORT.md` regenerates for only 11 of 16 ports; five `package.json`
  files drifted from the manifest (022).
