# Status

Standings, settled facts and ranked leads for the [finer loop](README.md). Volatile: rewritten
after every fleet measure and every verdict. Contract: [objective.md](objective.md). History:
[log.md](log.md). Updated 2026-09-02 after 041 and the harvests that opened 042–044.

## Standings

Last full fleet measure: 2026-09-02 01:05, working tree, `lilscript-codec`, Brotli-11 (every port
declares `cost_model = "brotli"`), every port rebuilt with the binary at cc18452 except three:
jquerylil timed out at 90 min on its two-core fleet slot (row carries the 2026-09-01 18:15 build,
which still has the 040 fusion), katexlil never recompiles under the fleet (lead 9), motionlil fails
to build. Regenerate with `node finer/tools/fleet.mjs --measure`.

**11 wins / 11 losses**, net −92513 Brotli. `src` is the count of modified source files in the port
at measurement: how much of the number is *not* the compiler.

| port | delta | src | attribution |
|---|---:|---:|---|
| react-markdownlil | +10815 | 19 | was +14166: the zero probe budget is gone (037). The committed artifact was React-external glue and the tree inlines it — not the same program; the tree is a 45-file migration |
| motionlil | +9314 | 0 | fails to build under the fleet (`ERR_MODULE_NOT_FOUND`); baseline scope-suspect: `dist/full.js` against the real motion UMD |
| remarklil | +6782 | 46 | bundles unminified npm `vfile` instead of the sibling port (~1821, 006); micromark core. +30 from 037, and its HEAD build failed `api` and `closed` |
| katexlil | +5800 | 11 | unexamined, and never rebuilt: its build skips the compiler while `dist/` is newer than `src/` (lead 9) |
| micromarklil | +3321 | 51 | micromark core: emitted volume; Terser still extracts −884 from our artifact (035) |
| remark-parselil | +2922 | 36 | micromark core |
| mdast-util-from-markdownlil | +2824 | 33 | micromark core — the three share it, so one fix moves ~9 KB |
| **mobxlil** | **+2641** | **0** | **clean source, real loss.** Was +2771; −130 from 037's search re-deciding. Terser extracts −652 from our artifact (039) |
| **jquerylil** | **+1825** | **0** | **clean source, real loss, stale row.** The committed artifact is 1045 smaller than this tree build (039 → 041: the local rename); −540 is the plain-data type (013); shipped `animate` throws (040, fixed in the compiler, port not yet rebuilt) |
| unifiedlil | +241 | 6 | config exhausted (027); emits 47% more functions, each 29% bigger. +7 from 037, and its HEAD build threw on import |
| remark-mathlil | +137 | 9 | config exhausted (027) |

Wins: rehype-katex −112118 (scope-suspect), zod −20103, rehype −2618 (+99 from 037's search),
hast-util-to-html −1014, rehype-stringify −794, mdast-util-to-hast −752, remark-rehype −687, marked
−579, remark-gfm −402, remark-breaks −67, posthog −1.

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

1. **jquerylil still ships the 040 miscompile** until it is rebuilt: `returnHr(r,n,t,e)` in
   `createTween`, so `$(el).animate(…)` throws. The compiler is fixed (040: the printer's adjacency
   rule at the splice; every other port byte-identical), but the port's 90-minute level-15 `always`
   build has timed out on every two-core fleet slot today. Rebuild it on four or more cores, add an
   `animate` test to the port, ship. Correctness outranks every byte below.
2. **The local rename does not run on jquerylil, and it is −733** (041, patch held): the ledger
   never starves `converge_local_names`; it bails because the resolver calls a second `var t` in
   one function ambiguous and the pass demands a total resolution. Narrowed to unsound scopes it
   is −765 on the artifact and −733 through the pipeline, equal to Terser's locals-only rename,
   with 40 header spellings instead of 91 — but that build ships `a?b,c:d` from
   `fold_common_conditional_arms` (044), so the narrowing waits in
   `finer/out/041/narrow-the-bail.patch` behind the 044 fix, then a fleet A/B. Deterministic exit
   counters (`LILSCRIPT_TIMING=1`: `rename_*`, `cleanup_*`) now make the stage visible.
3. **The loop runs on one host** (038): the owner's pool of Azure machines must carry fleet builds,
   sweeps and A/Bs (objective.md §9); a fleet A/B on this host is two hours, most of it jquerylil at
   level 15 on two cores. Inventory first — nothing in the repo names the machines.
4. **A plain-data object type** (013 → 042, opened from a harvest): 013's −540 included DOM reads
   no honest type can free, so the ceiling is lower; first a no-syntax port experiment re-typing
   jquerylil's five compiler-owned bags (≤ −80 confirms), then `object<T>` and a callable `object`.
5. **Statement-boundary absorption** (035 → 043, opened from a harvest): of Terser's seven
   `sequencesize_2` shapes we have one, three partial and three keyword-refused; the `return E,V`
   fold runs only with `terminal_local_rounds > 0`, zero at three of four sites. Cheap claim first.
6. **Pooling benefit model** (011): `count * length` is a raw-objective formula; under Brotli the
   repeats were already matches. About −35 on jquerylil and should generalize to every Brotli port.
7. **Budget allocation** (009, 036): 46 of 47 families starve at micromarklil's shipped config while
   the incumbent survives; `begin_fair_slice` splits evenly across families with very different hit
   rates.
8. **Markdown-stack composition** (006): remarklil bundles npm `vfile` (~1821); the three
   `lilBundlePorts` concatenate independently compiled modules.
9. **Fleet hygiene**: katexlil's *working-tree* `scripts/build.mjs` (an uncommitted owner rewrite)
   skips the compiler whenever `dist/` is newer than `src/` — the binary is not in its cache key —
   so **no compiler change has been measured on katexlil** since its last `--force` build and its
   +5800 is an old compiler's number; five ports fail to build under the fleet; motionlil's baseline
   needs a scope decision; sixteen ports need their sources committed before their numbers are
   compiler measurements; react-markdownlil's tree carries a 45-file source-graph migration with the
   037 config change on top. `shipped-vs-compiled` is **green again** (2026-09-02): rehype-katexlil's
   four esbuild steps now keep `minifySyntax` on, the 030 fix, −103 raw / −9 Brotli on its ESM,
   63/63 tests — applied in the port's working tree on top of its owner's uncommitted build rewrite,
   so it lands with that rewrite.
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
