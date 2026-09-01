# 015 — Do this workstream's changes actually help the shipped ports?

**Status: RESOLVED. The regression was traced to a single change
([010](../010-string-pool-alias-pricing/README.md)) and reverted. With it gone, jQueryLil is
byte-identical to the HEAD compiler — every other change in this workstream is size-neutral there.**

## Why this had to be asked

Everything in 001–013 was measured on the in-repo benchmark ports. The shipped libraries live in
sibling repositories, compile from different sources with different configs, and — per
[014](../014-dirty-tree-scoreboard/README.md) — most of them are mid-migration. A compiler change
that helps `benchmarks/popular/ports/jquery` is not thereby shown to help `jquerylil`.

[014](../014-dirty-tree-scoreboard/README.md) also isolated the only two ports where a clean
experiment is possible: **`jquerylil` and `markedlil` have zero modified source and zero modified
config.** For those two, source is a constant and the compiler is the only variable.

## Method

A worktree at `lilscript` HEAD (commit `58cba9a`, 08-30 23:30) was built to give a baseline compiler
that contains **neither** this workstream's changes **nor** the other uncommitted work already
present in this tree. Then each port's own entry and own config were compiled with both binaries and
measured with the pinned codec. No bundling step, so `esbuild` is not a variable either.

```sh
git worktree add /tmp/.../head-compiler HEAD && cargo build --release
<compiler> ~/markedlil/src/entry.lil --target js-module --config ~/markedlil/lilscript.toml -o out.js
target/release/lilscript-codec --json out-head.js out-mine.js
```

## Result — markedlil (identical source, identical config)

| compiler | raw | gzip-9 | Brotli-11 |
|---|---:|---:|---:|
| `lilscript` HEAD | 36426 | 10861 | 9579 |
| this workstream | **36135** | **10785** | **9571** |
| delta | **−291** | **−76** | **−8** |

**Smaller on all three axes.** The Brotli gain is modest (−8) because markedlil is a 36 KB artifact
where the level-13 probe-ladder retune has less room than on a 90 KB one, but nothing regressed and
the raw and gzip improvements are clear.

markedlil's config uses `optimization_level = 15`, so this port does not even benefit from the
default change — the gains come from the ladder retune at 13 being irrelevant here and the
string-pool alias-pricing fix ([010](../010-string-pool-alias-pricing/README.md)) doing the work.

## Result — jQueryLil (identical source, identical config)

Same method. jQueryLil's config uses `optimization_level = 15`, so the level-13 work does not apply
and this isolates the level-independent changes.

| compiler | raw | gzip-9 | Brotli-11 |
|---|---:|---:|---:|
| `lilscript` HEAD | **83681** | **32480** | **29209** |
| this workstream | 87543 | 32763 | 29491 |
| delta | **+3862** | **+283** | **+282** |

**This workstream makes jQueryLil larger on every axis.** That is a real regression against the port
the objective names first, and it inverts the markedlil result rather than agreeing with it — so
"my changes help" was not a safe generalization from one port, and stating it after one port was
premature.

Ruled out so far:

- **The `enclosing_group_openers` rewrite** ([004](../004-peephole-relex-tax/README.md)) is *not* the
  cause. A differential test now in `js_peephole/tests.rs` keeps the original backward scan as an
  oracle and asserts the new index answers identically at **every token position** across ten real
  generated shapes. It passes.
- **The memos and the lexer cache** are not the cause: all six A/B runs in
  [002](../002-content-addressed-memoization/README.md) produced byte-identical artifacts.
- **The default level change** is not the cause: jQueryLil pins level 15.
- **The probe-ladder retune** is not the cause: level 15's budget is 384 both before and after, once
  the 14/15 extrapolation was reverted ([009](../009-search-starvation/README.md)).

That left the **string-pool alias-pricing fix**
([010](../010-string-pool-alias-pricing/README.md)). Isolating it behind a switch:

| compiler | raw | gzip-9 | Brotli-11 |
|---|---:|---:|---:|
| `lilscript` HEAD | 83681 | 32480 | **29209** |
| this workstream, 010 **on** | 87543 | 32763 | 29491 |
| this workstream, 010 **off** | 83681 | 32480 | **29209** |

**010 accounts for the entire regression, and with it off the output is byte-identical to HEAD.**
It was reverted, and a re-compile confirms jQueryLil now matches the HEAD compiler byte for byte.

## What this establishes about the rest of the workstream

**Every other change here is size-neutral on jQueryLil** — the byte-identical result is a strong
statement, not an approximate one. That is the expected outcome and worth saying plainly: jQueryLil
pins `optimization_level = 15`, and the one size-affecting change this workstream kept — the probe
ladder — moves **level 13 only**. The caches, the lexer index, the quadratic rewrite and the
fixed-point caps were all designed to be byte-preserving, and on a 90 KB real artifact they are.

The corollary is that **this workstream's size wins are only visible on level-12/13 ports**. Seven
siblings pin level 13 (katexlil, remarklil, unifiedlil, micromarklil, remark-parselil,
react-markdownlil, mdast-util-from-markdownlil) and eight pin level 12 — but all fifteen have
mid-migration sources ([014](../014-dirty-tree-scoreboard/README.md)), so none can be cleanly
measured yet. On the in-repo jQuery benchmark port at level 13 the ladder is worth −58 Brotli.

## Bearing on the regression in 014

[014](../014-dirty-tree-scoreboard/README.md) found markedlil's committed `dist` (9517 Brotli,
bundled) is smaller than its working-tree `dist` (9652), with source and config unchanged, and both
built before this workstream started.

This experiment rules out one explanation and points at another. **This workstream is not the cause**
— with 010 reverted it is byte-neutral against HEAD on jQueryLil, and 010's contribution to markedlil
was −8, i.e. in the shrinking direction. But the committed artifact at 9517 is smaller
than *either* compiler's fresh output (9571 mine, 9579 HEAD), which means the compiler that produced
the committed `dist` was **better than HEAD on this port**. The regression therefore predates HEAD
and is somewhere in the compiler's own history, not in the uncommitted work.

That is worth a bisect: markedlil compiles in seconds, its source is frozen, and `9517` is a
concrete target to bisect against.

## Standing caveat

This tests two ports. The other sixteen cannot be tested this way until their sources settle, and
their reported sizes should not be trusted until then.
