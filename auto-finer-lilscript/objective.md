# LilScript objective (verbatim brief + working restatement)

## Verbatim brief from the project owner

> Listen, our goal with this language and compiler is simple. It's designed to get compiled into
> optimized JavaScript based on the objective, and it's usually we focus on high compression rates.
> Not high compression rates, but small bundle sizes in broadly gzip or raw. It's objective for which
> compression algorithm is selected. Can get changed. And the goal is, you know, giving weird property
> mangling everything, inlining functions, creating different closures, using different tricks,
> different JavaScript syntaxes, this that. Everything has effect only bundle size. Especially in GZIP
> and Brotli compressed objective one. So we can't always compile it into absolute global maximum.
> So there is a trade-off between time, compression rates, and performance. So basically, LilScript
> and the compiler shouldn't compromise performance much, it needs to be almost minimal and, you know,
> underlying libraries may have explicit and specific JavaScript behaviors that optimize their
> performance. For such things, LilScript needs to be expressive enough to get written in LilScript in
> comparison to same JavaScript, maybe? But our goal is like, LilScript is a typed language, and we can
> use structs, we can use classes. More often, or objects, but we basically give room to compiler to
> optimize them. You know, Google Closure Compiler advanced mode. How it flattens objects or mangles
> them, or converts objects into arrays with indices? These are all tricks we can employ, but the
> trade-off needs to be finer right now. And I'm seeing that compilation takes very long time right now.
> And as you know, there's a config knob, we have compression level, like optimization_level. When it's
> maximum, yes, it means we compromise on time and try to achieve very small bundle sizes, but by
> default for our projects, maybe make it 13, not 15. And 13 needs to be literally the sweet spot of
> time consumption for compilation and bundle sizes. And you need to keep rereading this while deciding
> on future migrations. We need to iterate language and compiler and at the end of the day, for lots of
> LilScript libraries we have, we forked actual npm libraries, we cloned them also, and we're trying to
> beat their minified sizes in Brotli. So we need to win in all such cases. jquerylil, markedlil,
> react-markdown's submodules LilScript versions, .. all, LilScript version needs to be better..
> sometimes LilScript code might be the reason why we compile into less optimized code, idk.
>
> regardless.. take your time, don't stop until it's completely implemented.
>
> and along the way, each change we have is hypothesis and create a folder for each, explain and gather
> progress and behaviours in that folder.. all in auto-finer-lilscript folder, create it and 001-blabla
> folder 002-baball .. kind of folders, and you must keep going. make sure that agents have not bloated
> context, occasionally give our objective.md (currently explained objective) to clean context subagents
> and let them reason accordingly..
>
> Right now since compilation takes infinitely long idk.. it's annoying.
>
> Along the way, without hypothesis, you must always visit terser's source code, oxc source code and
> understand how they work.. so we can learn tips tricks.. LilScript always tries to beat them, not tie.
> It's not always possible, sometimes we can lose but we really don't want that. at least tie.

## Working restatement (the contract this workstream is held to)

1. **Primary metric is served bytes under the artifact's declared objective.** `cost_model` selects
   raw / gzip(-9) / brotli(-11). Brotli is the usual objective. A win is bytes under *that* metric;
   incidental raw/gzip movement on a Brotli artifact is diagnostic, never a win or a regression.
2. **Any legal JS trick is fair game** as long as observable behavior is preserved: property mangling,
   object→array-index flattening (Closure ADVANCED style), aggressive inlining, closure re-shaping,
   alternate syntax spellings, string/number pooling, function merging.
3. **Runtime performance must not be materially compromised.** Generated code should stay ~as fast as
   hand-written idiomatic JS. `[javascript.performance] max_regression_percent` is the guard rail.
   Where a library needs a specific perf-critical JS shape, LilScript must be *expressive enough* to
   spell it — that is a language requirement, not a compiler one.
4. **Compile time is a first-class cost, not free.** `optimization_level` 0..15 is the effort ladder.
   - **13 is the default and must be the sweet spot**: near-15 bytes at a small fraction of 15's time.
   - 14/15 exist for people explicitly buying the last bytes with wall-clock.
5. **Competitive bar**: every `*Lil` port must be **≤** the upstream minified+Brotli size of the real
   npm library, and **≤** what Terser/Oxc/esbuild/Closure produce from equivalent input. Beat, don't
   tie. Ties are the acceptable floor; losses are bugs to be chased.
6. **A loss is frequently NOT the compiler's fault. Investigate the whole chain before blaming
   codegen.** This is not a footnote — it has been the *entire* story three separate times, and each
   time the compiler's own output was already correct:

   - **The build script.** [030](030-the-build-undoes-the-compiler/README.md): micromarklil's
     `scripts/build.mjs` re-bundles the compiler's ESM through esbuild with `minifySyntax: false`.
     esbuild parses `!0`, understands it as the boolean, and prints the canonical `true` on the way
     out — discarding all 87 compact booleans the compiler chose. Worth **4538 raw / 229 Brotli**.
     [006](006-markdown-stack-loss-diagnosis/README.md) was the same class in rehypelil, worth
     **2517 Brotli**.
   - **The measurement harness.** [028](028-unminified-lil-lane/README.md): `run.mjs` minified only
     the *official* lane, so for `remark`, `unified` and `react-markdown` our unminified esbuild
     bundle was being compared against Terser output. **10634 Brotli of reported loss was not real.**
   - **The port's `.lil` source shape.** Upstream micromark spells eight character classes as one
     closure factory (`regexCheck(/[A-Za-z]/)`, emitting `d=E(/…/),h=E(/…/),…`); the port spells them
     as eight separate exported functions. Same behaviour, more code.

   So the order of investigation when a port loses is: **(a)** does the shipped artifact match what
   the compiler actually wrote (`dist/*.raw.js`)? **(b)** is the comparison like-for-like — same
   bundling, same minification, same graph on both sides? **(c)** does the `.lil` source force a
   worse shape than the original library's own source, which is cloned and available to diff
   against? Only then **(d)** blame the compiler.

   Note what does *not* hold: "our code is too `JsValue`-typed" was measured across 14 ports and
   correlates **−0.09** with the losses. Check, do not assume.
7. **Representation is a measured choice, not a preferred one.** Closure ADVANCED's tricks are on the
   table — flattening nested access into a single scope, mangling and renaming across the whole
   program, turning objects into arrays with integer indices. Two things follow, and they pull in
   opposite directions:

   - **Flattening often wins twice.** A nested access path costs bytes at every mention and a
     dereference at every read; hoisting it flat can be both smaller under a compressor and faster.
   - **Sometimes the class *is* the right answer.** Where the algorithm genuinely uses class
     behaviour, the `class` spelling is dramatically more compact than the
     `function` + `.prototype` table it would otherwise become — measured at **−7049 raw / −769
     Brotli** on mobxlil for ten classes ([032](032-export-resolver-false-negative/README.md)) and
     **−3758 raw / −194 Brotli** on micromarklil
     ([031](031-admission-blocks-the-class-rewrite/README.md)).

   So neither representation is the default. **The compressor decides**, through `cost_model`, over
   candidates the search actually proposes — which is the real failure mode found so far: both class
   rewrites above were *generated and then refused by a validator*, so the cost model never got to
   vote. A representation the search never proposes cannot be measured, and an artifact a validator
   wrongly refuses cannot be scored. Fix those before adding new representations.

   The bound on all of it is compile time (point 4): a representation search that cannot be afforded
   at level 13 is not a win, it is a level-15 option.
8. **Method**: every change is a numbered hypothesis folder under `auto-finer-lilscript/NNN-slug/`
   containing the hypothesis, the experiment, the measurements, and the verdict — including
   falsified ones. Negative results are kept, not deleted.
9. **Standing homework (no hypothesis required)**: continuously read Terser and Oxc (`oxc_minifier`)
   sources to harvest techniques, and record what was learned.

## Standing target (updated 2026-09-01)

The single number this workstream is judged on:

> **Every `*Lil` port's shipped artifact must be smaller than its upstream npm equivalent under the
> port's declared `cost_model` — usually Brotli-11. Beat, don't tie.**

**`fleet.mjs` is the authority**, because it measures each port's own `dist/` — the bytes that ship.
Where it and the markdown-stack harness disagreed, the fleet was right ([028](028-unminified-lil-lane/README.md)).
Reproduce with `node auto-finer-lilscript/fleet.mjs`; sweep one port's configs with
`node auto-finer-lilscript/sweep.mjs --ports <name>`.

Last full rebuild of all 26 ports: **11 wins / 11 losses**, 4 ports without a declared baseline.

Losses, worst first. `src` = modified source files, i.e. how much of the number is *not* the compiler:

| port | delta | src | note |
|---|---:|---:|---|
| react-markdownlil | +14166 | 19 | committed artifact was glue-only (React external); the tree's inlines it. Not the same program |
| motionlil | +9314 | **0** | scope-suspect baseline: `dist/full.js` against the real motion UMD |
| remarklil | +6752 | 46 | |
| katexlil | +5800 | 11 | |
| micromarklil | +3925 | 51 | was +4154; [030](030-the-build-undoes-the-compiler/README.md) recovered 229 |
| **mobxlil** | **+3007** | **0** | **clean source — a real, attributable loss** |
| remark-parselil | +3235 | 36 | micromark family |
| mdast-util-from-markdownlil | +3175 | 33 | micromark family — these three share a core, so one fix moves ~10 KB |
| **jquerylil** | **+1825** | **0** | **clean source.** Beats upstream on *raw* by 4489; loses only on compressibility |
| unifiedlil | +234 | 6 | was +912; level 13 + `always` is worth 416 |
| remark-mathlil | +190 | 9 | |

Wins: rehype-katex (−112118, scope-suspect), zod (−20103), rehype (−1841),
hast-util-to-html (−1014), rehype-stringify (−794), mdast-util-to-hast (−752),
remark-rehype (−687), marked (−550), remark-gfm (−383), remark-breaks (−67),
**posthog (−1)**.

### What is settled, so it is not re-litigated

- **Configuration is exhausted on the near misses.** 20 builds over remark-math and unified buy 10
  bytes, and those 10 fail tests ([027](027-tuning-is-exhausted/README.md)). Beam width is worth 47
  bytes on posthog and *exactly zero* on unified — tune it per port, never raise it as a default.
- **Level 13 is the best *trade*, not universally the best bytes.** remark-math is 2287 at 15 and
  2336 at 13; unified goes the other way; jquery is 1402 bytes worse at 13. Measure per port.
- **Emitted volume predicts the losses at r=+0.924**, with perfect separation — every winner emits
  less JavaScript than the official, every loser more ([025](025-brotli-repetition-gap/README.md)).
  Compression *ratios* are already competitive; the problem is how much code we emit.
- **Specialisation is not the lever for the repetition class**
  ([029](029-specialisation-is-not-the-lever/README.md)): every anti-cloning switch makes it bigger.
- **Terser still finds 1578 Brotli / 16113 raw in our own micromark output** — statement→sequence
  merging (`;` −1498, `,` +932), branch→expression (`if(` −299), declaration merging (`var ` −344).
  We emit **607 `var` statements where Terser needs 38.** That is the largest known compiler-side
  lever and it is not yet taken.


## Open regressions, and what each is worth under its own objective

Two compiler regressions are precisely located and reproducible. Both were measured the same way:
frozen port source and config, one file reverted, deterministic output verified across runs and
thread counts.

| port | commit / range | raw | Brotli | status |
|---|---|---:|---:|---|
| markedlil | `593f048`, `src/compiler.rs` | −1568 | — | **fixed** in `41b88f2`; markedlil now 9506, its best ever |
| mobxlil | `42c1ad0..edbdf3a`, `src/compiler.rs` | **−7546 (12%)** | **−253 (1.5%)** | open — [018](018-mobx-admission-regression/README.md) |

The mobxlil one loses ten `class` declarations to `function` + `.prototype` + `setPrototypeOf`
tables. Note the asymmetry, which decides how urgent it is: **12% of raw and 1.5% of Brotli**,
because prototype tables are verbose but extremely repetitive. It is a serious regression for a
`cost_model = "raw"` project and a minor one under Brotli — the same 10:1 ratio measured for property
mangling in [008](008-jquery-compressibility-gap/README.md).

Seven candidate mechanisms have been falsified by instrumentation rather than argument; the counters
live in `src/timing.rs` (`admission`, `direct_validate`, `probe_dropped`) and are free when
`LILSCRIPT_TIMING` is unset.

## The default is 13; a port may still measure its way above it

Level 13 is the right *default* — that is what [007](007-level-13-sweet-spot/README.md) measures. It
is not a ceiling, and one port has earned its way past it: `jquerylil` ships
`optimization_level = 15` with `candidate_search = "always"`, and measured on its own source that is
worth **4.3% Brotli and 5475 raw bytes** for 23x the CPU. Dropping it to the default would widen its
gap against `jquery.min.js` from +780 to +3077.

**Do not transfer an effort curve between artifacts.** The 1.4%-for-20x plateau that justifies the
default was measured on the in-repo `benchmarks/popular/ports/jquery`; the shipped `jquerylil` port
is a different program and its curve is three times steeper.

## How to run the fleet

Ports are built and measured in parallel by `auto-finer-lilscript/fleet.mjs`. Each port gets a fixed
core slice (`taskset`) with `RAYON_NUM_THREADS` matched, because the compiler is itself Rayon-parallel
and unpinned concurrency makes every slice thrash.

```sh
node auto-finer-lilscript/fleet.mjs                    # build + measure everything
node auto-finer-lilscript/fleet.mjs --measure          # measure the working tree, no builds
node auto-finer-lilscript/fleet.mjs --measure --committed   # measure HEAD's artifacts
node auto-finer-lilscript/fleet.mjs --ports markedlil,jquerylil --slots 2
```

It reports each port's `src` dirtiness alongside its size, because **a size measured against a
mid-migration source tree is not a compiler measurement**. At the time of writing 16 of the ports
have modified sources; only `jquerylil`, `markedlil`, `mobxlil`, `motionlil` and the untracked-source
ports give a clean compiler comparison.

## Fixed measurement rules

- gzip = stock zlib 1.3.1, level 9, mtime 0. Brotli = official Google Brotli 1.1.0, quality 11,
  window 22. These are pinned in `Cargo.toml` and `comparison/large-libraries/matrix.json`; do not
  substitute another encoder for scoring.
- Timing is never a size gate, but it *is* the thing being optimized in the level-13 work.
- Semantics gate everything: an artifact that does not pass its behavior lane cannot be a win.
