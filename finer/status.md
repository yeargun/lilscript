# Status

Standings, settled facts and ranked leads for the [finer loop](README.md). Volatile: rewritten
after every fleet measure and every verdict. Contract: [objective.md](objective.md). History:
[log.md](log.md). Updated 2026-09-02 after 042; the build pool (038) is live and the source-map branch is being integrated.

## Standings

Last full fleet measure: 2026-09-02 07:00, the 041/044 A/B (`finer/out/044/scoreboard.new.json`),
`lilscript-codec`, Brotli-11 (every port declares `cost_model = "brotli"`), every port rebuilt with
the binary that landed 041 and 044, jquerylil and markedlil on four cores. katexlil never recompiles
under the fleet (lead 9); motionlil fails to build. Regenerate with `node finer/tools/fleet.mjs
--measure` once no agent's fleet pass is rewriting `dist/`.

**11 wins / 11 losses**, net −96377 Brotli (−3864 from 041). `src` is the count of modified source files in the port
at measurement: how much of the number is *not* the compiler.

| port | delta | src | attribution |
|---|---:|---:|---|
| react-markdownlil | +10815 | 19 | was +14166: the zero probe budget is gone (037). The committed artifact was React-external glue and the tree inlines it — not the same program; the tree is a 45-file migration |
| motionlil | +9314 | 0 | fails to build under the fleet (`ERR_MODULE_NOT_FOUND`); baseline scope-suspect: `dist/full.js` against the real motion UMD |
| remarklil | +4688 | 46 | was +6782: −2094 from 041's local rename. Bundles unminified npm `vfile` instead of the sibling port (~1821, 006); micromark core |
| katexlil | +2542 | 0 | was +5800 (046/047): −1175 from rebuilding on the current binary, −943 from the unicode table → generator loop, −1233 from the late cleanup's whole-artifact candidate finally being admitted (047). Closed = open until the port is typed; the rest of the number is the `JsValue` shape (046) |
| micromarklil | +3321 | 51 | micromark core: emitted volume; Terser still extracts −884 from our artifact (035) |
| remark-parselil | +2922 | 36 | micromark core |
| mdast-util-from-markdownlil | +2824 | 33 | micromark core — the three share it, so one fix moves ~9 KB |
| **mobxlil** | **+2641** | **0** | **clean source, real loss.** Was +2771; −130 from 037's search re-deciding. Terser extracts −652 from our artifact (039) |
| **jquerylil** | **+1196** | **0** | **clean source, real loss.** Was +1825: −663 from 041's local rename, on a build that also carries 040's fix (`animate` runs; the tree `dist/` holds it, uncommitted). 40 header spellings remain against Terser's 24; −540 is the plain-data type (013 → 042) |
| unifiedlil | +241 | 6 | config exhausted (027); emits 47% more functions, each 29% bigger. +7 from 037, and its HEAD build threw on import |
| remark-mathlil | +137 | 9 | config exhausted (027) |

Wins: rehype-katex −112118 (scope-suspect), zod −20103, rehype −3384 (−766 from 041),
hast-util-to-html −1014, rehype-stringify −794, mdast-util-to-hast −754, remark-rehype −687, remark-gfm
−679 (−277 from 041), marked −641 (−62 from 041, every lane on the purity diagonal), remark-breaks
−67, posthog −1.

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
- **A silently skipped stage is now the first suspect** (036, 037, 041, 043, 044): four of the last
  eight folders found a pass that never ran — starved, bailed on a total-resolution check, unbudgeted
  at level 13 — or a validator that admitted what node refuses. `LILSCRIPT_TIMING=1` reports the
  rename's and the late cleanup's exits; read them before claiming a lever is missing.
- **Statement fusion is not a lever** (043). Terser's `sequences`, ablated from its defaults, is
  worth at most 3 Brotli on four ports; our `return E,V` fold lands 0 of 162 eligible sites because
  the late cleanup finds a zero ledger on 15 of 19 entries (the reserve accounting, not the ceiling),
  and by hand it is −19. The 013/035 line of −1498 `;` / +932 `,` / −299 `if(` / −344 `var ` is
  stale: today −54 / −219 / −9 / −25. What Terser's `compress` band on micromark actually is:
  `collapse_vars` +280 and `unused` +296 when removed — single-use assignment collapsing.

- **A validator defect can silently un-peephole a whole port** (047). On katexlil the resolver
  took `,_(j),j={}` after an elided block-terminal semicolon for declarators, `_` became
  ambiguous, and every peepholed candidate — the search's whole-artifact peephole and the late
  cleanup's canonical candidate alike — failed admission; the port shipped essentially
  un-peepholed output and nothing said so. Fixed: −1233 Brotli. `LILSCRIPT_TIMING=1` now reports
  `cleanup_canonical_{err,same,boundary,refused,unprobed,pushed}` and `cleanup_shaped_*`; a port
  whose canonical candidate is refused is the first thing to check on a loss.
- **Main's admission has no terminal parser gate** (047): a fold that emits a wrong program ships on
  main and is refused on feature/source-maps (4e799a8's Oxc gate). Compiler changes are measured
  with a feature/source-maps binary until the branch lands.
- **A raw-motivated fold goes in as a scored candidate after the local rename, or not at all**
  (047): applied unconditionally per declaration, three declaration folds moved remark-gfm +41
  by name churn alone; as a late family before the rename, micromark +64; after it, +13 / +4.
- **Function by function we already beat Terser on katexlil; the loss is collective** (047,
  `examples.md`): every outermost body compressed alone sums to 70113 against Terser's 71113,
  but concatenated 35278 against 32431 — Terser's near-identical builders compress against each
  other and ours do not (78 same-size matched pairs: 6377 vs 3471 in context). Search off is
  +3241, so the search is not the variety; Terser's passes on our artifact recover 576 of the
  2847, so it is not a spelling either. It is the port's transliteration spelling one idiom
  several ways; the lever is the port, written once per idiom.
- **`JS.push` is not a port smell** (047): spelled as a method invoke it is +696 on katexlil; the
  intrinsic is what the array families fold.

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

0. **Redundant number coercions** (047, `finer/out/047/examples.md`): `+(a-b)` where the operand is
   already a number — 64 sites on katexlil against Terser's 3 — and `-1` spelled `0-1` (2 sites).
   Generic emission folds with an obvious proof; the 115 `+member` coercions beside them are the
   port's untyped fields. Budget is settled: 512 and 1024 probes are byte-identical to 256 there.

1. **The arrow candidate can spell a `this` method as an arrow** (042's finding): shipped
   jquerylil's `scrollTop(1)` TypeError is exactly that, and the same whole-artifact
   `function-spelling` flip is source-dependent at level 15 (+99 / +141 on two 16-line source
   changes; states that lose it are 3.5 KB raw from the incumbent's family). Two compiler items,
   correctness first: admission refuses an arrow spelling for any function that uses `this`
   (`arguments`, `new.target` too), then the family is made reachable from every seed. Then ship
   jquerylil: its tree `dist/` holds the fixed-compiler build (28641, `animate` runs) and its `src/`
   carries 042's five struct views (−57), both uncommitted on a clean source.
2. **Landed by 041 and 044**: the local rename narrowed to refuse only unsound scopes, −3864 across
   the fleet; the ternary-arm precedence fix and its admission check. What 041 leaves open is the
   template-literal bail (micromarklil never converges: 16 backticks) and the duplicate-`var`
   emission itself.
3. **The loop runs on one host** (038): the owner's pool of Azure machines must carry fleet builds,
   sweeps and A/Bs (objective.md §9); a fleet A/B on this host is two hours, most of it jquerylil at
   level 15 on two cores. Inventory first — nothing in the repo names the machines.
4. **A plain-data object type** (013 → 042): the no-syntax experiment re-typed jquerylil's five
   owned bags for −57 (25 of 405 stores): a typed read stops blocking but is still not deferrable
   (`op_can_defer`), so the gain is read–read adjacency and scales with read count. The language
   follow-ups (`object<T>`, a callable `object`) would sit under the search's own noise until the
   spelling family above is deterministic; parked behind it.
5. **Single-use assignment collapsing** (013 → 043's redirection): Terser's `collapse_vars` and
   `unused` are +280 / +296 on micromark, +56 / +132 on mobx, +136 / +94 on jquery when ablated —
   the largest compiler-side class left with a measured ceiling. Prior art to read before opening:
   `tighten-body.js:278-1000`, `drop-unused.js:113`, Oxc `minimize_statements.rs:1149`, ours
   `copies.rs:1040, 1185`. Confirms at ≥ −150 on micromarklil and ≥ −50 on mobx and jquery.
5b. **Three ports ship through an esbuild post-minifier** (043): micromarklil, playcanvaslil and
   rehype-katexlil bundle with `minifySyntax` on. On micromarklil that step is −311 against the
   same bundle with it off, of which 030's `!0` re-print is 266; esbuild's *own* transforms — 157
   `return` fusions and 109 `if(` rewrites — are worth ≈ −45 over the compiler's file. Small
   headroom, and a doctrine question for the owner: objective.md §7 forbids a post-minifier in the
   compiler, and these builds are one.
6. **Pooling benefit model** (011): `count * length` is a raw-objective formula; under Brotli the
   repeats were already matches. About −35 on jquerylil and should generalize to every Brotli port.
7. **Budget allocation** (009, 036): 46 of 47 families starve at micromarklil's shipped config while
   the incumbent survives; `begin_fair_slice` splits evenly across families with very different hit
   rates.
8. **Markdown-stack composition** (006): remarklil bundles npm `vfile` (~1821); the three
   `lilBundlePorts` concatenate independently compiled modules.
9. **Fleet hygiene**: katexlil's build now carries the compiler binary in its cache key (046;
   the −1255 that 041/044 were worth on it had never been measured) and refuses instead of
   regex-rewriting the compiler's output; its site is generated by `scripts/measure-site.mjs`
   with a Playwright benchmark, after a "12× faster" claim that did not reproduce (both runtimes
   put the port within ±15% of upstream on the shared corpus). Five ports fail to build under the fleet; motionlil's baseline
   needs a scope decision; sixteen ports need their sources committed before their numbers are
   compiler measurements; react-markdownlil's tree carries a 45-file source-graph migration with the
   037 config change on top. `shipped-vs-compiled` is **green again** (2026-09-02): rehype-katexlil's
   four esbuild steps now keep `minifySyntax` on, the 030 fix, −103 raw / −9 Brotli on its ESM,
   63/63 tests — applied in the port's working tree on top of its owner's uncommitted build rewrite,
   so it lands with that rewrite.
10. **Objective purity** (objective.md §2): give a second port raw and gzip configs so the check is
    not one port's; re-run on every fresh three-way build of markedlil.

## Known issues

- `fold_while_trailing_increments` had two defects on an `if/else` body whose arms both end in
  the counter increment (047): it lifted the last one into the `for` header and left the other —
  a bare `else}` (F, a syntax error the feature/source-maps gate refuses) and, with braces, a
  double increment (L, a wrong program no gate refuses; katexlil's screenshot corpus caught it).
  Both fixed in the lift's body-level guard. A wrong-program fold is only ever caught by a port's
  tests: they are part of every A/B.

- No `instanceof` on `JsValue` in the language (047): ports carry an `isPrototypeOf` helper
  (`ga(a,Y)` on katexlil, 26 sites); as `instanceof` it is −60 Brotli. A language item.

- `bundle.mode = "preserve-modules"` fails on katexlil at level 8: "function 393 (`<unnamed>`,
  closure) has no emitted name (live=true inlined=true …)" at `functions/hbox.lil:12` — a closure
  the whole-program pass inlined has no name to export from its chunk. Blocks a per-module
  bottom-up comparison; the function-level pairing (`scripts/function-pairs.mjs` in katexlil)
  is the workaround.

- katexlil (046): `mangle.exports = true` is refused with "generated JavaScript callable ABI
  mismatch" (the check compares the mangled export names against the unmangled manifest), so
  objective.md §4's app-world floor cannot be measured on it; `optimization_level = 0` fails with
  "unresolved generated export binding … `ka as buildMathML`" on the committed source (levels 8 and
  13 compile). Both reproduce with the feature/source-maps binary; neither is examined.
- katexlil is a `JsValue` transliteration (046): typed classes for `domTree`, `mathMLTree`,
  `Settings`, `Options`, `Parser` and a thinner `host.lil` are the port's lever; until then closed
  world equals open world on it.

- Shipped jquerylil throws on `animate` (lead 1) and `scrollTop(1)` throws a `TypeError` on both
  the committed and working-tree artifacts (039, unexamined).
- `js_peephole::tests::rejects_generated_syntax_above_the_configured_floor` passes again at
  f135ad3 (suite 1642/1642 on 2026-09-01); the ES2021 class-field issue recorded earlier is not
  reproducible and is dropped from this list until it recurs.
- `--explain` on the jQuery port fails with "selected JavaScript candidate changed the normalized
  ABI manifest" while the plain compile succeeds.
- `comparison/markdown-stack/REPORT.md` regenerates for only 11 of 16 ports; five `package.json`
  files drifted from the manifest (022).
