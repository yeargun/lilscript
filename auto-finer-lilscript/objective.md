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
6. **A loss may be the LilScript source's fault, not the compiler's.** When a port loses, check whether
   the `.lil` source forces a worse shape before blaming codegen.
7. **Method**: every change is a numbered hypothesis folder under `auto-finer-lilscript/NNN-slug/`
   containing the hypothesis, the experiment, the measurements, and the verdict — including
   falsified ones. Negative results are kept, not deleted.
8. **Standing homework (no hypothesis required)**: continuously read Terser and Oxc (`oxc_minifier`)
   sources to harvest techniques, and record what was learned.

## Standing target (updated)

The single number this workstream is judged on:

> **Every `*Lil` port's shipped artifact must be smaller than its upstream npm equivalent under the
> port's declared `cost_model` — usually Brotli-11. Beat, don't tie.**

Current state, **all 26 ports freshly rebuilt** with the current compiler, pinned codec:
**9 wins / 11 losses, −65794 Brotli overall.** Reproduce with `node auto-finer-lilscript/fleet.mjs`.

Read that alongside the dirtiness column: **16 ports have modified sources**, so their numbers move
with in-flight port work as well as with the compiler. The four ports with clean sources —
`jquerylil`, `markedlil`, `mobxlil`, `motionlil` — are the only ones where a compiler change can be
attributed, and they are where a fix should be proven before it is believed.

The eleven losses, worst first. `src` = modified source files, i.e. how much of the number is *not*
the compiler:

| port | delta | src | note |
|---|---:|---:|---|
| react-markdownlil | +14166 | 19 | committed artifact was glue-only (React external); the tree's inlines it. Not the same program |
| motionlil | +9314 | **0** | scope-suspect baseline: `dist/full.js` against the real motion UMD |
| remarklil | +6704 | 46 | |
| katexlil | +5800 | 11 | |
| micromarklil | +4154 | 51 | micromark family |
| **mobxlil** | **+3577** | **0** | **clean source — a real, attributable loss** |
| remark-parselil | +3235 | 36 | micromark family |
| mdast-util-from-markdownlil | +3175 | 33 | micromark family — these three share a core, so one fix moves ~10.6 KB |
| **jquerylil** | **+1825** | **0** | **clean source.** Beats upstream on *raw* by 4489; loses only on compressibility |
| unifiedlil | +248 | 6 | |
| remark-mathlil | +137 | 9 | |

The wins: rehype-katex (−112118, scope-suspect), rehype (−1735), hast-util-to-html (−1014),
rehype-stringify (−794), mdast-util-to-hast (−752), remark-rehype (−687), **marked (−579)**,
remark-gfm (−383), remark-breaks (−67).

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
