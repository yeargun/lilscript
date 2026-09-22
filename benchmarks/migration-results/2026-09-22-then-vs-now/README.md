# Migration progress and compression: before the migration vs now

2026-09-22 · branch `finer/059-idiom-directed-naming` · current state committed and pushed as [`b79efb19`](https://github.com/yeargun/lilscript/commit/b79efb19)

## The short answer

**Plan progress.** Seven of the fourteen milestones (001–007) are implemented. None is formally closed yet, because each still needs an independent review against the evidence ledger. 008 is in progress, and its first batch landed today. 009–014 have not started.

**Compression.**

- **The default compiler route hasn't changed.** Output is within a few tenths of a percent of the pre-migration compiler on every library it can build. That's expected: the migration builds a replacement backend, which becomes the default only at 011.
- **The replacement backend is not better yet.** It's `--backend semantic`, opt-in today. It is close on zodlil (+0.4%) and markedlil (+3.3%), and still well behind on katexlil (+12.6%) and probelil (+28.6%). All four comparisons are Brotli, against the current default route. On gzip it is already 1.4% smaller than the default route on zodlil.
- **It compiles in seconds instead of minutes.** Part of that is because it doesn't yet run the Brotli-in-the-loop candidate search (milestone 010).
- **Today's work mattered.** It cut the semantic route's probelil output from 2,775 to 1,919 Brotli bytes (−31%), shrinking the gap to the default route from +86% to +29%.

**Two surprises.** The pre-migration compiler can't build two of the four libraries as they are today:
- probelil stops with an internal compiler error.
- katexlil's config uses an option (`name_ordering`) that the pre-migration compiler doesn't know.

The current compiler builds both.

## How this was measured

| | Pre-migration | Now |
|---|---|---|
| Commit | `06b8da20` (2026-09-11), the last commit before the migration plan's work | `b79efb19` (2026-09-22) |
| Binary SHA-256 | `e32a0e38…089e` | `b3651a04…b498` |
| Routes | default (the only route that commit has) | default, and `--backend semantic` |

- **Libraries.** markedlil and katexlil (the owner's reference pair), zodlil (the usual background check) and probelil (the migration's feature-dense probe). Each is at its current checkout, recorded in [`data/identities.json`](data/identities.json).
- **Scratch copies only.** Each library was copied into scratch space and built there; no port repository was modified.
- **What was compiled.** Each library's main artifact: `src/entry.lil` (probelil: `src/probe.lil`), with the port's own `lilscript.toml`, one compile per arm.
- **The semantic route's source patch.** Semantic katexlil applies the recorded two-line port patch (`extern JsValue parseInt; extern JsValue parseFloat;`). The other libraries build unmodified.
- **Sizes.** Measured with the repository's canonical codec (`lilscript-codec`: zlib 1.3.1 level 9, Brotli 1.1.0 quality 11 window 22). The competitor numbers from 2026-09-20 used the same codec.
- **Shipped files.** "Shipped file" sizes rebuild each library's published file from the compiler output exactly as its build script does: banner, katexlil's font-metrics data, export filter.
- **Pre-migration compiler builds.** The pre-migration compiler was built in a separate git worktree (`git worktree add … 06b8da20`), so the main checkout never left the latest commit.
- **Timing.** One run per arm on this 8-vCPU burstable host, with other compiles running at the same time. Times are indicative only.

## Results

**Brotli 11 bytes**

| Library | Pre-migration (06b8da20) | Now, default route (b79efb1) | Now, semantic route (b79efb1) | Default route: now vs before | Semantic vs default route now |
|---|---:|---:|---:|---:|---:|
| markedlil | 9,385 | 9,360 | 9,673 | −25 (−0.3%) | +313 (+3.3%) |
| zodlil | 29,753 | 29,682 | 29,796 | −71 (−0.2%) | +114 (+0.4%) |
| katexlil | 55,463\* | 55,404 | 62,397 † | −59 (−0.1%) | +6,993 (+12.6%) |
| probelil | fails | 1,492 | 1,919 | – | +427 (+28.6%) |

**gzip 9 bytes**

| Library | Pre-migration (06b8da20) | Now, default route (b79efb1) | Now, semantic route (b79efb1) | Default route: now vs before | Semantic vs default route now |
|---|---:|---:|---:|---:|---:|
| markedlil | 10,491 | 10,457 | 10,776 | −34 (−0.3%) | +319 (+3.1%) |
| zodlil | 34,963 | 34,932 | 34,442 | −31 (−0.09%) | −490 (−1.4%) |
| katexlil | 66,759\* | 66,746 | 75,558 † | −13 (−0.02%) | +8,812 (+13.2%) |
| probelil | fails | 1,605 | 2,118 | – | +513 (+32.0%) |

**raw bytes**

| Library | Pre-migration (06b8da20) | Now, default route (b79efb1) | Now, semantic route (b79efb1) | Default route: now vs before | Semantic vs default route now |
|---|---:|---:|---:|---:|---:|
| markedlil | 34,148 | 34,233 | 40,149 | +85 (+0.2%) | +5,916 (+17.3%) |
| zodlil | 124,426 | 124,409 | 126,400 | −17 (−0.01%) | +1,991 (+1.6%) |
| katexlil | 217,440\* | 217,382 | 274,301 † | −58 (−0.03%) | +56,919 (+26.2%) |
| probelil | fails | 4,034 | 5,446 | – | +1,412 (+35.0%) |

**Compile time on this host (seconds, wall clock, one run)**

| Library | Pre-migration | Now, default route | Now, semantic route |
|---|---:|---:|---:|
| markedlil | 234.2 | 222.2 | 1.8 |
| zodlil | 182.6 | 177.2 | 5.0 |
| katexlil | 327.2 | 361.6 | 7.5 |
| probelil | fails | 29.7 | 0.4 |

**katexlil, like for like** (the pre-migration compiler rejects the `name_ordering` line, which the current compiler accepts and ignores; the pre-migration arm drops it)

| Codec | Pre-migration | Now, default route | Change | Now, semantic route | Semantic vs default now |
|---|---:|---:|---:|---:|---:|
| Brotli 11 | 55,463 | 55,404 | −59 (−0.1%) | 62,397 | +6,993 (+12.6%) |
| gzip 9 | 66,759 | 66,746 | −13 (−0.02%) | 75,558 | +8,812 (+13.2%) |
| raw | 217,440 | 217,382 | −58 (−0.03%) | 274,301 | +56,919 (+26.2%) |
| compile seconds | 327.2 | 361.6 | | 7.5 | |

\* katexlil's pre-migration build drops one line from the shipped config, `name_ordering = "idiom-converged"`. The pre-migration compiler rejects that option; the current compiler accepts it and, as its own warning says, ignores it. So the pair is like for like.
† Semantic katexlil adds the recorded two-line port patch: `extern` declarations for `parseInt` and `parseFloat`.
On probelil, "fails" is the pre-migration compiler's internal error: `SSA value 3 has no emitted name in function sameParity`.

### Shipped files against the competitor bars

These are the files a user downloads, rebuilt from each arm's compiler output by the library's own stitching. Brotli 11 bytes; competitor numbers are from the [2026-09-20 receipt](../2026-09-20-competitors/), same codec.

| Shipped file | Terser 5.51.2 | esbuild 0.28.1 | rolldown 1.2.5 (Oxc) | Pre-migration | Now, default route | Now, semantic route |
|---|---:|---:|---:|---:|---:|---:|
| katexlil `katex.esm.js` | **63,044** | 63,767 | 63,253 | 64,842 | 64,901 | 71,731 |
| markedlil `marked.esm.js` | *11,562* | *11,903* | *11,606* | 9,469 | 9,423 | 9,737 |
| zodlil `zod.core.js` | *52,561* | *55,133* | *54,819* | 29,753 | 29,682 | 29,796 |

Only katexlil is like for like: it has the same public surface and the same amount of code as upstream.

- **katexlil.** The default route is still 1,857 bytes (2.9%) behind Terser; it was 1,798 behind before the migration. The semantic route is 8,687 behind.
- **markedlil and zodlil (italic bars).** These are not comparable. The upstream bundles publish 18 and 240 names, where these ports publish 8 and 2, so the smaller LilScript numbers there are not wins.

For katexlil, the compiled code alone and the shipped file move in opposite directions (−59 and +59). Both changes are inside the roughly ±100-byte Brotli noise this project has measured for single renames, so the default route is unchanged.

## Is the output correct?

**Semantic route, current binary.** Each port's own test suite was run in a scratch copy ([`data/semantic-port-tests.json`](data/semantic-port-tests.json)):

| Library | Tests | Notes |
|---|---|---|
| zodlil | 1,353 / 1,353 | |
| katexlil | 1,251 / 1,251 | Official KaTeX Jest suite included |
| markedlil | 27 / 29 | Both failures check the output's text, not its behavior (details below) |

The two markedlil failures:

1. **The closed-world build keeps option keys like `.gfm=` readable.** The semantic route doesn't rename properties yet. This case was already open at the end of 007 and is owned by 008/009.
2. **New today: the test requires every export to be written as an alias (`x as parse`).** The new output declares `let parse=…` and exports `export{parse,…}`. Importing both builds gives the same export keys, value types, function names and lengths as the 007 binary, and the text is shorter. The fix belongs in that assertion, or in a printer option if the alias form is wanted for its own sake.

**The 72-case census.** Script JS, module JS and native C all pass 72 of 72 on this binary, with no miscompiles ([`data/census-b79efb1.txt`](data/census-b79efb1.txt)).

**Compiler unit tests.** 2,991 of 3,000 pass. The nine failures are search tests that assert byte-exact golden outputs; today's shorter printing changed those outputs, so the expectations need re-deriving. That work is scheduled for the end of 008 batch 1.

**Default route.** It was not re-tested here. Its outputs moved by a few bytes, from legacy-route edits made during the migration period.

## Why the semantic route is still bigger

Token counts in the compiler output:

| Token | markedlil default | markedlil semantic | zodlil default | zodlil semantic |
|---|---:|---:|---:|---:|
| `\|0` | 56 | 148 | 27 | 458 |
| `let ` / `var ` | 6 / 104 | 243 / 0 | 118 / 458 | 839 / 36 |
| `return ` | 107 | 177 | 746 | 920 |
| `void 0` | 2 | 11 | 394 | 3 |
| `return c?a:b` | 12 | 0 | 39 | 3 |
| `c&&(…)` | 54 | 8 | 298 | 16 |
| `else` | 59 | 93 | 86 | 153 |

- **Integer normalization (`|0`).** The default route proves value ranges and drops most `|0`; the semantic route has no range facts yet. That is milestone 009's "exact/range propagation" family.
- **Declarations.** The default route gathers declarations into a few `var` lists. The semantic route merges only *adjacent* `let`s, so hundreds of separate declarations remain. This is 008 declaration compaction.
- **Branches.** The default route turns `if(c)return a;return b` into `return c?a:b` and `if(c)x=y` into `c&&(x=y)`; the semantic route mostly doesn't yet. This is 008/009 branch compaction.
- **probelil and katexlil.** The default route's inliner and constant folder do most of the work. For example, probelil's `print(withDefaults(seed))`, where `withDefaults(int a, int b = 10, int c = 100)`, becomes `console.log(a+110|0)`. The semantic route has no inliner or constant folder yet (009).

## Migration plan progress, in plain words

| # | Milestone | What it means | State |
|---|---|---|---|
| 001 | Baselines | Freeze what each library must keep doing (its tests) and what competitors produce, so every later claim can be checked | Implemented; awaiting review |
| 002 | Public boundaries | Decide exactly what must stay identical where a library meets its users: export names, callbacks, `this` and host values | Implemented |
| 003 | Policy and resources | One owner for configuration, work budgets and memory limits | Implemented |
| 004 | Semantic facts and checked edits | The new compiler's core: facts about the program, and edits that are checked before they apply | Implemented |
| 005 | Public service | The new route is reachable from the CLI (`--backend semantic`) and emits JS and native C | Implemented |
| 006 | Integration proof | The pieces work together on real code, with measured costs | Implemented |
| 007 | Language and ports | The new route compiles the whole language: census 72/72/72, 24 of 25 ports build and 18 pass their full suites | Implemented |
| 008 | Whole-program JS and delivery | Make the new route's output small, and deliver modules, chunks and lazy imports | **In progress**: batch 1 done today |
| 009 | Compression families | Inlining, constant folding, range facts and dead code as shared, checked families | Not started |
| 010 | Codec search | Search over alternatives scored by real Brotli, within a budget | Not started |
| 011 | Make it the default | The CLI and every port use the new route, and all suites pass | Not started |
| 012 | Speed and memory gates | Compile-time and memory targets | Not started; telemetry exists |
| 013 | Compression qualification | Beat Terser, esbuild and Oxc per codec on every library | Not started |
| 014 | Retirement | Delete the old route | Not started |

## What changed today (008 batch 1)

Semantic route on probelil, Brotli bytes, same source and config (the default route produces 1,492):

| Step | Brotli |
|---|---:|
| Start of 008 | 2,775 |
| No space between `+` and a sign unless it would form `++`; `(x??null)??y` is `x??y`; discarded results drop `void`/`\|0` | 2,729 |
| Private functions go straight into their binding | 2,192 |
| Reads of never-reassigned locals and parameters are repeated at each use instead of copied to a temporary | 2,086 |
| Single-use closures form in place (`b.map(a=>a*2\|0)`) | 2,013 |
| Printing: one `let` list, no braces around single statements, `else if`, no `;` before `}`, concise arrows; `==` between same-type primitives | 1,964 |
| No `\|0` on `.length`, `indexOf` or collection sizes when the config assumes unpatched builtins | 1,945 |
| Private functions spelled as arrows; callee-only closures lose their exact name | 1,919 |

## Where everything is

- **Commit.** The migration's whole working tree (source, docs, tools, evidence) is commit `b79efb19` on `finer/059-idiom-directed-naming`, pushed together with seven earlier unpushed commits.
- **Large payloads stay local.** Sixty-six evidence payload files over 1 MiB (228 MB of raw logs and JSON dumps) remain on the build host. They're listed with SHA-256 in [`../LOCAL-PAYLOADS.tsv`](../LOCAL-PAYLOADS.tsv), so their receipts still verify.
- **This report's data.** Raw compiler outputs and per-arm measurements are in [`data/`](data/).
- **The Azure build pool is gone.** Both worker scale sets (`lilscript-workers-v7`, `lilscript-workers`) no longer exist, so every build here ran on this host. Recreating them needs the owner's approval.

## Next

1. **Finish 008 batch 1.** Re-derive the nine golden-output tests. Settle markedlil's alias assertion: change the test, or offer the alias form.
2. **Continue 008.** Declaration merging and single-use forwarding through checked target edits, liveness and tree shaking, naming, helpers, then the delivery modes.
3. **009.** Range facts to drop `|0`, inlining and constant folding: where most of the remaining gap to the default route is, and the gap to Terser on katexlil.
