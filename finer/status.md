# Status

Standings, settled facts and ranked leads for the [finer loop](README.md). Volatile: rewritten
after every fleet measure and every verdict. Contract: [objective.md](objective.md). History:
[log.md](log.md). Updated 2026-09-01 after 039.

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
(objective.md §2) ran on 2026-09-01 on markedlil's artifacts: on the **committed** dist the Brotli
build beats the gzip build under gzip (10644 against 10732), an off-diagonal winner; on the working
tree, built later on one compiler, every build wins its own metric (raw 33537 / gzip 10574 / Brotli
9423 on the diagonal). The committed violation is a stale dist, not a live cost-model fault, and the
check is re-run on every fresh three-way build. SWC is named as a baseline and has no
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
- **A forward scan from an assignment is not a liveness check when the reader can be hoisted**
  (037). A function declared 61 KB before the class it reads runs after the alias is assigned. Two
  prototype-alias folds asked "read after?"; both now ask "observed anywhere the rewrite does not
  absorb?" with exact shadowing (`scope::binding_is_observed_outside_span`). The ten-shape audit
  `hoisted_readers_before_a_module_assignment_keep_it_live` is where the next elimination fold
  gets its hoisted reader.
- **Name spelling is not the jquery gap either** (039). A same-length relabel in Terser's
  frequency order moves −27; the emitter already spells from frequency alphabets. Terser's remaining
  −765 there is its *local* rename, and it exists only against the working-tree artifact.
- **Terser still finds −1498 `;` / +932 `,` / −299 `if(` / −344 `var ` in our micromark output**
  (013, 035). The statement-boundary transforms (`e;return x` → `return e,x` and four siblings) are
  absent; each is raw-neutral alone and pays only by letting other folds reach across.

## Landed by this workstream

Effort telemetry (`src/timing.rs`); content-addressed memos (−27% CPU, byte-identical); lexer
cache; fixed-point caps; default 15 → 13 with the probe-ladder retune, after which 13 beats 15 on
jQuery; class-expression terminators and the `?.` floor (023, 024); member bodies as scopes (033);
constructor-exempt admission on re-checked winners (031); bundler charset (034: −27689 raw / −1093
Brotli on two ports); whole-program alias liveness in the class rewrite and the unread-alias fold
(037: react-markdownlil drops its zero probe budget for −3351 Brotli, 120/120); build-script fixes on rehypelil and micromarklil (006, 030); the harness
minifies both lanes (028). Reverted after measurement: string-pool alias pricing (+282 on
jquerylil), the narrowed `unstable` closure (+69 net), the levels-14/15 probe raise (3x CPU for 192
bytes).

## Open leads, ranked by measured value

1. **jquerylil ships a miscompile** (039, folder 040 pending its mechanism): `createTween` carries
   `returnHr(r,n,t,e)||…` — `return` fused onto its operand, so the call is to an undeclared global
   and `$(el).animate(…)` throws `ReferenceError`. The callee was then inlined at its only *visible*
   use and its declaration deleted, and `split_fused_keyword_identifiers` refuses to repair a name
   that is no longer declared. In both the committed dist and the tree; the port's six compat tests
   never animate. Correctness outranks every byte below.
2. **`converge_local_names` starves on jquerylil** (039): the committed artifact is **1045 Brotli
   smaller** than the working-tree build of the same clean source (28225 against 29270), and Terser
   *loses* 274 on the committed one. The tree build has 90 distinct parameter-header spellings
   against 25; the pass is budget-gated at `compiler.rs:7764` and a template-literal bail at
   `rename.rs:37` disables it on micromarklil. Trace why it does not land: the same class as 036 and
   037, a stage that silently does not run.
3. **The loop runs on one host** (038): the owner's pool of Azure machines must carry fleet builds,
   sweeps and A/Bs (objective.md §9); a fleet A/B on this host is two hours, most of it jquerylil at
   level 15 on two cores. Inventory first — nothing in the repo names the machines.
4. **A plain-data object type** (013): −540 on jquerylil, 69% of its gap, without the
   `pure_getters` rules change. Language work; the largest single measured win found.
5. **Statement-boundary absorption** (035): the five `sequencesize_2` shapes, worth 95–359 per port
   only through the folds they unblock. Measure as a portfolio change.
6. **Pooling benefit model** (011): `count * length` is a raw-objective formula; under Brotli the
   repeats were already matches. About −35 on jquerylil and should generalize to every Brotli port.
7. **Budget allocation** (009, 036): 46 of 47 families starve at micromarklil's shipped config while
   the incumbent survives; `begin_fair_slice` splits evenly across families with very different hit
   rates.
8. **Markdown-stack composition** (006): remarklil bundles npm `vfile` (~1821); the three
   `lilBundlePorts` concatenate independently compiled modules.
9. **Fleet hygiene**: katexlil's `scripts/build.mjs` skips the compiler whenever `dist/` is newer
   than `src/` — the binary is not in its cache key — so **no compiler change has been measured on
   katexlil** since its last `--force` build and its +5800 is an old compiler's number; five ports
   fail to build under the fleet; motionlil's baseline needs a scope decision; sixteen ports need
   their sources committed before their numbers are compiler measurements; react-markdownlil's tree
   carries a 45-file source-graph migration with the 037 config change on top. `shipped-vs-compiled`
   fails on rehype-katexlil (2026-09-01): its build re-prints every `!0`/`!1` as `true`/`false`, the
   030 class — five sites, a small fix, and the gate stays red until it lands.
10. **Objective purity** (objective.md §2): give a second port raw and gzip configs so the check is
    not one port's; re-run on every fresh three-way build of markedlil.

## Known issues

- Shipped jquerylil throws on `animate` (lead 1) and `scrollTop(1)` throws a `TypeError` on both
  the committed and working-tree artifacts (039, unexamined).
- `js_peephole::tests::rejects_generated_syntax_above_the_configured_floor` passes again at
  f135ad3 (suite 1642/1642 on 2026-09-01); the ES2021 class-field issue recorded earlier is not
  reproducible and is dropped from this list until it recurs.
- `--explain` on the jQuery port fails with "selected JavaScript candidate changed the normalized
  ABI manifest" while the plain compile succeeds.
- `comparison/markdown-stack/REPORT.md` regenerates for only 11 of 16 ports; five `package.json`
  files drifted from the manifest (022).
