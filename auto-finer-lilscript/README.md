# auto-finer-lilscript

Hypothesis log for the compile-time/bundle-size tuning workstream. The contract every entry is held
to is [`objective.md`](objective.md) — read it first; it is the verbatim brief plus the working
restatement.

Each `NNN-slug/` folder holds one hypothesis: what was predicted, how it was tested, what the
numbers said, and the verdict. **Falsified hypotheses are kept**, because the expensive mistake is
re-proposing an idea that was already measured and rejected.

## Index

| # | Hypothesis | Verdict |
|---|---|---|
| [001](001-where-does-compile-time-go/README.md) | Compile time is concentrated in two whole-artifact primitives (canonical codec, generated-JS re-analysis) | **Confirmed** for small artifacts; 004 corrected it for large ones |
| [002](002-content-addressed-memoization/README.md) | Both primitives are pure functions of their bytes and can be memoized on a content digest | **Landed.** −91% analyses, byte-identical output |
| [003](003-cheap-codec-screening/README.md) | A cheaper Brotli quality can *rank* candidates the way q11 does, so exact encodes can be reserved for a verified beam | **Measured, parked.** Technique works (q9 ranks the q11 winner first, 20x cheaper) but 004 showed the codec is only 7% of the artifact that actually hurts |
| [004](004-peephole-relex-tax/README.md) | The jQuery-scale bottleneck is not the codec | **Confirmed.** 95% is emission + the 135-fold peephole; the "uncapped fixed point" lead was real but not the cause |
| [005](005-idle-fold-guards/README.md) | 82% of fold invocations do nothing; skip them | **Split.** Decline-memo landed; entry guards **falsified** by a token census |
| [006](006-markdown-stack-loss-diagnosis/README.md) | The 10 markdown-stack losses are compiler-quality gaps | **Mostly falsified.** The three worst are harness/build bugs, not codegen |
| [007](007-level-13-sweet-spot/README.md) | Level 13 is the effort plateau and should be the default | **Confirmed.** Level 15 is 20x the CPU for 1.4% of the bytes. Default changed |
| [008](008-jquery-compressibility-gap/README.md) | jQueryLil's Brotli loss is a compressibility problem, not a volume problem | **Confirmed.** raw +2.7% but Brotli +11.7%; property mangling is a total no-op on this port |
| [009](009-search-starvation/README.md) | The search is starved, not too narrow — higher levels dilute rather than deepen | **Confirmed and landed.** Probe ladder retuned; jQuery −58 Brotli, acorn −8, level 13 now beats level 15 |
| [010](010-string-pool-alias-pricing/README.md) | The string pool admits aliases that lose bytes because it prices every alias as one character | **Confirmed, landed.** Small win (−3 Brotli on jQuery); the cost model was simply wrong |
| [011](011-string-pooling-under-compression/README.md) | The pooling *benefit* model credits every repeat at full width, which is only true for a raw objective | **Confirmed.** Threshold tuning is non-monotone and was rejected; jQueryLil drops pooling for **−35 Brotli** |
| [012](012-port-scoreboard/README.md) | Where does every `*Lil` port actually stand against upstream? | **Measured.** 21 comparable ports: 10 W / 11 L. Two `REPORT.md` verdicts were stale and flip to wins; jQueryLil's real gap is **+780**, not +4038 |
| [014](014-dirty-tree-scoreboard/README.md) | Are the reported losses even measured against committed artifacts? | **No.** 18 ports have uncommitted `dist` changes. Re-measured, the markdown stack is **8 W / 6 L, −23588 Brotli**, not 6 W / 10 L, +28435 |
| [031](031-admission-blocks-the-class-rewrite/README.md) | Why does the canonical peephole never land? | **Admission rejects it for having a `constructor`** — the class rewrite's own keyword, counted as a new static property. That is 018's missing mechanism, refused at a gate 018 never counted. But exempting it measures **+826 Brotli worse**, so it is reverted: the same rewrite is −171 applied to the finished artifact and +1154 applied where the compiler applies it |
| [030](030-the-build-undoes-the-compiler/README.md) | Is the port, not the compiler, why we do not get smaller? | **For micromarklil, yes — its build script.** esbuild with `minifySyntax:false` re-prints `!0` as `true`, discarding all 87 compact booleans the compiler chose. **−4538 raw, −229 Brotli**, 1963/1963 tests pass. Third time a tool *between* compiler and artifact has been the whole story |
| [029](029-specialisation-is-not-the-lever/README.md) | Is cloning what costs us the repetition class? | **No — every anti-cloning switch makes it bigger.** call-site specialisation off is +45, all three off +48, the folding switches are already on. On jquerylil `region-outlining` changes **nothing** and the documented phi lever is worth 5 bytes, not 87. Every config surface is now exhausted for this class |
| [028](028-unminified-lil-lane/README.md) | Why does `unified` emit 87 `var` against Terser's 12? | **It does not — the harness never minified our lane.** For the three `lilBundlePorts` the Lil side is a raw esbuild bundle measured against a Terser-minified official. **10634 Brotli of reported loss was the harness**: remark +10395 -> +5988, react-markdown +18552 -> +12923, unified +808 -> +264 |
| [027](027-tuning-is-exhausted/README.md) | Can config tuning close the two nearest losses? | **No.** 20 builds over remark-math (+137) and unified (+234) buy 10 bytes, and those 10 fail 5 tests. Beam width is worth 47 bytes on posthog and **zero** on unified; level 13 beats 15 on unified and loses to it on remark-math. What is left is volume: unified emits **47% more functions, each 29% bigger** |
| [026](026-the-missing-classes/README.md) | Where did 018's ten `class` declarations go? | **Generated, then corrupted — but that is not why they are missing.** The fold does destroy a valid ten-class artifact 36 times a build, and candidates *are* refused before admission counters see them. But disabling it changes nothing (0 classes either way, Brotli identical), so my causal claim was false |
| [025](025-brotli-repetition-gap/README.md) | Do we lose Brotli because our output compresses worse? | **No — because we emit more code.** Repeat-coverage gap predicts nothing (r=+0.13); raw excess predicts almost everything (**r=+0.94** over 15 ports). Every win emits less JS than Terser, every loss more, by 9.5–85%. Our compression ratios are already competitive |
| [024](024-optional-chain-floor/README.md) | Does the ES syntax floor actually stop forbidden syntax? | **Not for `?.`.** The check matched a `"?."` token the lexer never emits (it emits `?` then `.`), so the arm was unreachable at every edition. Fixed by adjacency. Latent only: every port targets ES2022 |
| [023](023-unparseable-class-expressions/README.md) | Why did `--measure` fail once all 16 ports verified? | **The compiler emitted unparseable JavaScript.** `emit_class` keyed its terminator off the keyword, not the shape, so `var u=class VFile{...}` came out bare and ran into the next statement. remark and unified had no working build. Fixed; 80/80 artifacts parse; **rehype and remark-gfm flip to WIN** |
| [022](022-harness-refresh/README.md) | Can the project's own scoreboard be regenerated? | **For 11 of 16.** Five ports' `package.json` drifted from the manifest and the harness fails closed — that is why REPORT.md is stale. The eleven that run: **every one improved or held, −3202 total, and remark-gfm flips LOSS → WIN** |
| [021](021-reflective-ffi-predicts-loss/README.md) | Why are the receivers dynamic in the first place? | **The losing ports are JavaScript transliterated through a host FFI.** Reflective calls per kloc order the scoreboard: markedlil **0.8 → −579 WIN**, micromarklil **178 → +4154**. But the obvious remedy was tested twice on a real file and **regressed both times** — partial typing cannot pay because the helper layer is `JsValue`-typed end to end |
| [020](020-unstable-transitivity/README.md) | The `unstable` closure propagates from every operand; narrowing it to fusible ones should cut the named-temporary excess | **Sound, and reverted.** Cuts unstable 12% but nets **+69 Brotli** (−56 micromark, −38 mobx, **+29 marked, +81 jQuery**). Fewer unstable values is not fewer bytes |
| [019](019-one-mechanism/README.md) | Are the eleven scoreboard losses eleven problems? | **No — one mechanism, confirmed on two unrelated families.** `JsValue` receivers escape untyped, so values are `unstable` and each takes its own named statement: +7.6%/+19.4% identifiers, +113%/+42% semicolons. Source typing prices at ~10%, SSA destruction at ~90% |
| [018](018-mobx-admission-regression/README.md) | mobxlil is +3577 with a frozen source — is that a compiler regression too? | **Reframed.** −7546 raw / −253 Brotli across `42c1ad0..edbdf3a`, but the admission gate is **load-bearing**: removing it emits unparseable JavaScript. The bytes are the cost of correctness, not a revertible mistake. Seven mechanisms falsified; 59s repro |
| [017](017-oxc-declaration-merge/README.md) | oxc merges `var a; a=b()` into one declaration and LilScript does not — is that the statement-density win? | **No. Implemented, tested, measured at zero, reverted.** LilScript's SSA destruction already emits initialized declarators; the target shape occurs 0–3 times per artifact |
| [016](016-marked-size-regression/README.md) | markedlil's committed artifact is smaller than what any current compiler produces — is there a regression? | **Yes: `593f048`, `src/compiler.rs`, +1568 raw (4.7%).** Statement merging is lost (+479 `;`, −388 `,`); six fewer candidates reach evaluation. Three mechanisms falsified; reported not patched |
| [015](015-does-this-work-help/README.md) | Do these compiler changes help the *shipped* sibling libraries, not just the in-repo benchmark ports? | **Caught a regression of my own.** One change cost **+282 Brotli** on jQueryLil; reverted. Everything else is byte-identical to HEAD there |
| [013](013-statement-density/README.md) | jQueryLil's remaining +780 is one mechanism: surplus assignment statements | **Confirmed and priced at −540 Brotli (69% of the gap).** Cause is the getter-hook assumption making 1681 values unstable. The flag that fixes it is Terser's `pure_getters`, which the baseline also has off — so the honest fix is a plain-data *type*, not a flag |

## Standing reference

[`_refs/competitor-techniques.md`](_refs/competitor-techniques.md) — a technique-by-technique
inventory of `oxc_minifier` and `terser`, with a PRESENT/PARTIAL/ABSENT verdict for LilScript on each
and file:line evidence. Refreshed as a standing task, not tied to a hypothesis.

## Tools added by this workstream

- `src/timing.rs` — named effort buckets (`codec`, `analyze`, `emit`, `peephole`, `optimize`, `lex`,
  `closers`, `regions`, `scopes`, `bindings`, `idle_fold`, `active_fold`, plus fixed-point iteration
  counts), printed as one JSON line to stderr under `LILSCRIPT_TIMING=1`, followed by a per-fold
  idle-time table. Zero cost when unset.
- `src/artifact_memo.rs` — content-addressed memos for the canonical codec, generated-JS analysis,
  and per-fold declines. `LILSCRIPT_NO_MEMO=1` disables all of them plus the lexer cache, so any
  measurement can be A/B'd inside one binary.
- `LILSCRIPT_DUMP_CANDIDATES=<dir>` — writes every distinctly scored artifact for offline study.
- `bench.sh` — level sweep reporting CPU time, peak RSS, canonical raw/gzip/Brotli sizes, and the
  deterministic work counters.

## Measurement discipline

This host is shared with unrelated CPU-heavy processes; identical work has varied **3x** in wall
clock between back-to-back runs. Consequently:

- **Deterministic work counters are the primary metric** (codec encodes, emissions, bytes scanned,
  families starved). They do not move with load.
- Wall clock is never quoted as a result. Where a time claim is unavoidable, it is **CPU time
  (user+sys), minimum of an interleaved A/B**, since contention can only add.
- Sizes always come from `lilscript-codec` (pinned zlib 1.3.1 / Google Brotli 1.1.0). Node's Brotli
  disagreed with the pinned encoder on **96 of 279** artifacts at quality 11 — see 003. Never score
  with Node's codecs.

## State at the end of this pass

**Landed in the compiler** (all byte-identical or byte-smaller; full suite 1629 pass):

| change | file | effect |
|---|---|---|
| Effort telemetry | `src/timing.rs` (new) | Makes the time/size trade measurable at all |
| Codec + analysis + fold-decline memos | `src/artifact_memo.rs` (new), `compiler.rs`, `js_peephole/mod.rs` | **−27% CPU** on jQuery, byte-identical output |
| Per-thread lexer cache | `js_peephole/token.rs` | lex bytes scanned −53% |
| Emission-path folds memoized | `compiler.rs` | The six folds on the hottest loop now skip proven declines |
| Linear enclosing-group index | `js_peephole/folds/declarations.rs` | Removes a quadratic backward scan from the most expensive fold (195 ms/call) |
| Fixed-point iteration caps | `optimizer.rs` | Closes the unbounded-loop gap oxc and terser both cover |
| **Default level 15 → 13** | `config.rs` | The headline: level 15 was 20x the CPU for 1.4% of the bytes |
| **Terminal probe ladder retune** (level 13 only) | `config.rs` | jQuery **−58** Brotli, acorn **−8**; level 13 now beats level 15 |
| ~~String-pool alias pricing~~ | `codegen_ir_js.rs` | **Reverted** — cost +282 Brotli on jQueryLil ([010](010-string-pool-alias-pricing/README.md)) |
| jQueryLil drops `string-pooling` | `ports/jquery/lilscript.toml` | **−35** Brotli, −28 gzip under that port's declared objective |

**Cumulative on the in-repo jQuery benchmark port at level 13: 30651 → 30555 Brotli (−96), while the
compile got ~27% cheaper and level 13 replaced level 15 as the default.**

On the *shipped* siblings, verified against a compiler built from `lilscript` HEAD with identical
source and config ([015](015-does-this-work-help/README.md)): **jQueryLil is byte-identical** — it
pins level 15, which this workstream deliberately left untouched, so the size work does not reach it.
Every size gain here lands on level-12/13 ports, and all fifteen of those have mid-migration sources
([014](014-dirty-tree-scoreboard/README.md)) so none can be cleanly measured yet. The compile-time
gains apply everywhere.

**Landed outside the compiler**: `rehypelil/scripts/build.mjs` had
`minifyWhitespace: format != "esm"` where every sibling port uses `true`, so the exact ESM file the
comparison harness measures shipped unminified. Verified on the checked-in artifact:
**−45977 raw / −2517 Brotli**.

## One change made and then reverted

The probe-ladder retune originally raised levels 14 and 15 as well (to 512 and 768), extrapolating
from "jQuery was still gaining at 768 probes". Measuring the pair rather than just the byte curve
showed level 15 going from **1829 to 5434 CPU-seconds** — ninety minutes on one artifact — to save
192 Brotli bytes. That is the trade [007](007-level-13-sweet-spot/README.md) exists to criticize, so
it was reverted; only the measured level-13 change was kept, and level 13's output is byte-identical
across the revert. Full detail in [009](009-search-starvation/README.md).

## Known issues, not caused by this workstream

- `js_peephole::tests::rejects_generated_syntax_above_the_configured_floor` fails. Verified failing
  at **clean HEAD** in a separate worktree, so it predates this work. The syntax floor is not
  rejecting ES2022 class fields under an ES2021 target.
- `--explain` on the jQuery port fails with *"selected JavaScript candidate changed the normalized
  ABI manifest"* while the same compile succeeds without `--explain`. This blocked reading jQuery's
  starvation telemetry directly in [009](009-search-starvation/README.md); acorn was used instead.
- `FORCE_COLOR` in the environment makes `node` colorize `console.log`, which fails every test that
  compares node stdout. Run the suite with `env -u FORCE_COLOR cargo test --release`.

## Read this before quoting any port size

Both `comparison/markdown-stack/REPORT.md` and [012](012-port-scoreboard/README.md) measure the
**uncommitted working tree** of the sibling repositories, and that tree is mid-migration:
**18 ports have modified `dist/`, and 16 of those also have substantially modified `src/`**
(micromarklil alone: 51 changed files plus a deleted `src/block.lil`). Sizes measured there confound
source changes, config changes, and compiler changes at once. See
[014](014-dirty-tree-scoreboard/README.md).

Only **`jquerylil` and `markedlil`** have unchanged source *and* config, so only those two support a
clean compiler comparison today.

## Where the ports actually stand

[012](012-port-scoreboard/README.md) measured all 26 siblings against upstream with the pinned
codec. **10 wins / 11 losses across 21 comparable ports**; 5 have no legitimate baseline. Three
results are worth pulling out:

- **markedlil wins**: 9652 Brotli against `marked.min.js`'s 10085, **−433 (4.3%)**. Every objective
  has its own build and each one wins its own metric.
- **jQueryLil is a near-tie, not a rout**: the *committed* dist is 28225 Brotli against official's
  27445 — **+780** — and it **beats official on raw by 4489 bytes**. The larger figures quoted
  elsewhere came from the in-repo benchmark port and from a regressed working tree.
- **`comparison/markdown-stack/REPORT.md` is stale**: re-measuring current dist flips **rehype**
  (−1735) and **remark-gfm** (−383) from losses to wins. The report should be regenerated before
  any of its numbers are quoted.

**Three live problems to look at before anything else.** `jquerylil/dist/` carries an uncommitted
**+3258 Brotli regression** and `markedlil/dist/` a **+135** one — both with *unchanged source and
config*, both built before this workstream started. [015](015-does-this-work-help/README.md) rules
this workstream out as the cause and localizes it further: markedlil's committed artifact (9517) is
smaller than what *either* HEAD or this workstream produces fresh (9579 / 9571), so the regression
is in the compiler's own history **before HEAD** and is bisectable — markedlil compiles in seconds
with frozen source and 9517 is a concrete target. Separately, motionlil's self-reported "16/16 wins"
is measured with Node's zlib/brotli against per-demo tree-shaken bundles, which a direct
whole-package comparison contradicts by 9314 Brotli bytes.

## Highest-value open leads, in order

1. **Give LilScript a plain-data object type** ([013](013-statement-density/README.md)). Measured at
   **−540 Brotli on jQueryLil, 69% of its entire deficit.** jQueryLil's `JsValue` bags mean every
   `o[k]` is a potential getter, which makes 1681 values "unstable" and forces each into its own
   assignment statement — that is the 2.06x statement density. The `assume_pure_property_reads` flag
   recovers it, but so does Terser's `pure_getters`, which the baseline also leaves off; winning that
   way would change the rules. A *type* the compiler can prove wins legitimately. Largest measured
   win found anywhere in this workstream, and it is language work, not codegen work.
2. **Regenerate `comparison/markdown-stack/REPORT.md`.** It is stale: two of its ten losses
   (**rehype**, **remark-gfm**) are already wins on current dist. Its numbers should not be quoted
   until it is rebuilt. Cheap, and it changes the headline scoreboard.
3. **Fix the pooling *benefit* model** ([011](011-string-pooling-under-compression/README.md)).
   `count * literal_length` is only correct for a raw objective; under gzip/Brotli the repeats were
   already matches. Raising the threshold is *not* the fix — the curve is non-monotone. Worth ~35
   Brotli on jQueryLil and it should generalize to every Brotli-objective port.
4. **Budget allocation, not budget size** ([009](009-search-starvation/README.md)):
   `CodecBudget::begin_fair_slice` splits work evenly across families with wildly different hit
   rates. The probe-ladder retune bought bytes by adding budget; spending the existing budget better
   should buy more.
5. **The markdown-stack composition question** ([006](006-markdown-stack-loss-diagnosis/README.md)):
   `remark`'s bundle pulls in unminified npm `vfile` instead of the sibling Lil port (~−1821
   Brotli), and the three `lilBundlePorts` need a methodology decision before their large reported
   losses mean anything — they concatenate independently-compiled modules with no post-minification,
   which is not a codegen comparison.
6. **Property mangling cannot fire on jQueryLil** — turning it off is byte-identical. Now priced:
   even mangling *every* property name is worth **at most 1277 Brotli** and realistically far less,
   against a large `.lil` retyping project ([008](008-jquery-compressibility-gap/README.md)). Ranked
   last on that basis, not first — which is where it started.
