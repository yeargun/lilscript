# 014 — The scoreboard was measuring an uncommitted working tree

**Status: PARTLY CONFIRMED, and my first conclusion was too strong — see the correction at the end
before quoting any number from the middle of this document.**

The finding that holds: **`REPORT.md` and [012](../012-port-scoreboard/README.md) both measure an
uncommitted working tree**, so neither is a clean read of the project's state. The finding that does
*not* hold as stated: that re-measuring committed artifacts gives "the true state" and shows the
stack net ahead. For 16 of the 18 ports the *source* is also mid-migration, so the committed
artifacts are an older version of a different library.

## How this was found

A rebuild of `micromarklil` was attempted to test whether the compiler changes from this workstream
help a losing port. Before rebuilding, its `dist/micromark.esm.js` was snapshotted with `git show` —
and the committed file turned out to be **89252 bytes** while the working-tree file was **101746**.
The port had a 12 KB uncommitted regression, exactly the pattern already found in `jquerylil`
([012](../012-port-scoreboard/README.md)).

Checking every sibling: **18 of the ports have uncommitted `dist/*.esm.js` changes.**

| port | committed | working tree | delta |
|---|---:|---:|---:|
| react-markdownlil | 32615 | 150636 | **+118021** |
| micromarklil | 89252 | 101746 | +12494 |
| jquerylil | 83044 | 95435 | +12391 |
| katexlil | 283674 | 293156 | +9482 |
| unifiedlil | 7781 | 15145 | +7364 |
| hast-util-to-htmllil | 28245 | 30291 | +2046 |
| mdast-util-to-hastlil | 12823 | 14843 | +2020 |
| rehype-stringifylil | 28587 | 30572 | +1985 |
| remark-rehypelil | 13095 | 14922 | +1827 |
| markedlil | 34860 | 36510 | +1650 |
| remark-breakslil | 1262 | 2746 | +1484 |
| remark-parselil | 94859 | 94275 | −584 |
| remark-mathlil | 6770 | 6370 | −400 |
| remark-gfmlil | 34583 | 33559 | −1024 |
| mdast-util-from-markdownlil | 94640 | 92865 | −1775 |
| remarklil | 143308 | 138262 | −5046 |
| rehypelil | 236405 | 192557 | −43848 |
| rehype-katexlil | 436511 | 2466 | −434045 |

**Every size claim in `comparison/markdown-stack/REPORT.md` and in
[012](../012-port-scoreboard/README.md) is measured against these working-tree files**, not against
anything committed. The tree is mid-migration and internally inconsistent — some ports got smaller,
most got substantially larger.

## Re-measured against committed artifacts

Same pinned codec, same official-Terser baselines from `REPORT.md` (which are upstream artifacts and
are not affected by the dirty tree), Lil side taken from `git show HEAD:dist/…`:

| port | Terser Brotli | REPORT.md Lil | **committed Lil** | REPORT Δ | **committed Δ** | |
|---|---:|---:|---:|---:|---:|---|
| rehype | 55080 | 64992 | **52059** | +9912 | **−3021** | **LOSS → WIN** |
| remark-gfm | 11238 | 11617 | **10891** | +379 | **−347** | **LOSS → WIN** |
| unified | 4425 | 5409 | **2776** | +984 | **−1649** | **LOSS → WIN** |
| rehype-katex | 113063 | 84501 | 78896 | −28562 | −34167 | win, wider |
| hast-util-to-html | 9839 | 8811 | 8211 | −1028 | −1628 | win, wider |
| rehype-stringify | 9886 | 9141 | 8301 | −745 | −1585 | win, wider |
| mdast-util-to-hast | 5016 | 4290 | 4048 | −726 | −968 | win, wider |
| remark-rehype | 5061 | 4390 | 4170 | −671 | −891 | win, wider |
| remark | 32551 | 45880 | 38384 | +13329 | +5833 | loss, 56% smaller |
| micromark | 22776 | 27344 | 26157 | +4568 | +3381 | loss, smaller |
| katex | 63137 | 69669 | 66879 | +6532 | +3742 | loss, smaller |
| remark-parse | 23283 | 27021 | 27095 | +3738 | +3812 | loss, ~same |
| mdast-util-from-markdown | 23279 | 26852 | 26977 | +3573 | +3698 | loss, ~same |
| remark-math | 2150 | 2600 | 2352 | +450 | +202 | loss, smaller |

Two ports are excluded as **scope-suspect**, on the objective test that their Lil artifact is under
55% of the official raw size — which means the two sides are not bundling the same program:

| port | committed Lil raw | Terser raw | ratio |
|---|---:|---:|---:|
| react-markdown | 32615 | 117759 | **0.28** |
| remark-breaks | 1262 | 3045 | **0.41** |

`react-markdown`'s committed artifact would score −21126 and its working-tree artifact +16769. Both
are meaningless: the committed one marks React and the submodules external, the working-tree one
inlines them. Neither is a codegen comparison, so neither is counted.

### react-markdown, settled

Measuring every artifact the harness keeps, with the pinned codec:

| artifact | react | raw | Brotli-11 | vs official |
|---|---|---:|---:|---:|
| `official-terser.js` | **external** | 117759 | **31092** | — |
| `lil-graph.js` — *what `REPORT.md` uses* | **INLINED** | 216436 | 47861 | +16769 |
| `lil-graph-terser.js` | external | 165508 | 47520 | +16428 |
| **`react-markdownlil/dist/react-markdown.esm.js`** | **external** | 150642 | **45258** | **+14166** |

**`REPORT.md` compares a React-inlining Lil graph against a React-external Terser baseline**, which
overstates the loss by **2603 Brotli**. The fair row is the shipped dist against `official-terser`:
both externalize React, both carry the full react-markdown graph, and the loss is **+14166** — real,
comparable, and the largest single row on the scoreboard.

Worth noting in passing: running Terser *over* the Lil graph (`lil-graph-terser.js`, +16428) is
**worse** than LilScript's own dist (+14166). The compiler beats a post-pass on its own output.

## Result

| | wins | losses | total Brotli |
|---|---:|---:|---:|
| `REPORT.md`, working tree | 6 | 10 | **+28435** |
| **committed artifacts, comparable ports only** | **8** | **6** | **−23588** |

## Findings

1. **The stack is net *ahead*, not behind.** −23588 Brotli across comparable ports, against the
   +28435 deficit the report shows. The headline was an artifact of measuring uncommitted files.
2. **Three genuine verdict flips**: rehype, remark-gfm, unified. The rehype flip is the one already
   predicted in [006](../006-markdown-stack-loss-diagnosis/README.md) from the `minifyWhitespace`
   build-script bug — the committed artifact evidently predates that regression.
3. **The six remaining losses are the real work**: `remark-parse` (+3812), `mdast-util-from-markdown`
   (+3698), `katex` (+3742), `micromark` (+3381), `remark` (+5833), `remark-math` (+202). These are
   all comparable single-program comparisons with raw ratios between 1.05 and 1.20 — we emit
   5–20% more code than Terser for the same library, which is a different failure mode from
   jQueryLil's (where we emit *less* raw and lose only on compressibility).
4. **`markedlil` is unaffected as a verdict** but its committed artifact is 1650 bytes smaller than
   the working-tree one measured in 012; the win is wider than reported.

## What has to happen before any of these numbers are quoted again

Both `comparison/markdown-stack/REPORT.md` and [012](../012-port-scoreboard/README.md) measure the
dirty tree and should be regenerated from a clean checkout. Until then the committed-artifact table
above is the better estimate, with the two scope-suspect ports excluded rather than counted.

The uncommitted regressions themselves need triage first: `react-markdownlil` (+118 KB),
`micromarklil` (+12 KB), `jquerylil` (+12 KB) and `katexlil` (+9 KB) are large enough that they are
either a deliberate scope change or a compiler regression, and which one it is changes what the
scoreboard means.


---

# Correction — the committed artifacts are an older library, not a cleaner measurement

The table above compares committed `dist/` against upstream and reads as though it were the same
program measured properly. It is not. Checking source as well as output:

| | ports with modified `src/` | modified `lilscript*.toml` | modified `dist/` |
|---|---:|---:|---:|
| **jquerylil** | **0** | **0** | 5 |
| **markedlil** | **0** | **0** | 6 |
| micromarklil | 51 | 2 | 10 |
| rehypelil | 56 | 0 | 7 |
| remarklil | 46 | 2 | 6 |
| remark-parselil | 36 | 2 | 5 |
| mdast-util-from-markdownlil | 33 | 2 | 5 |
| react-markdownlil | 19 | 2 | 8 |
| remark-gfmlil | 19 | 2 | 5 |
| *(9 more, all with modified sources)* | 2–15 | 0–3 | 5–14 |

**Sixteen of the eighteen ports have substantially modified sources** — micromarklil alone has 51
changed files and a deleted `src/block.lil`. For those, committed `dist` was built from committed
source by an older compiler, so a committed-vs-upstream comparison confounds three variables at
once. It is a measurement of the last released version, not a cleaner measurement of the current
one. The 8 W / 6 L table above should be read that way and no more.

## What survives, and it is the useful half

**`jquerylil` and `markedlil` have zero source changes and zero config changes — only regenerated
`dist/`.** For exactly those two ports the comparison is clean: identical inputs, different build
output.

| port | committed | working tree | regression | vs upstream (committed) | vs upstream (worktree) |
|---|---:|---:|---:|---:|---:|
| jquerylil | 28225 | 31483 | **+3258** | +780 | +4038 |
| markedlil | **9517** | 9652 | **+135** | **−568 WIN** | −433 win |

**Both regressed with identical source and config, for a combined +3393 Brotli.** markedlil's win
against upstream narrowed from 568 bytes to 433; jQueryLil's loss grew five-fold.

Timestamps place both builds **before** this workstream began:

- `markedlil/dist/marked.esm.js` — 08-31 **01:40**
- `jquerylil/dist/jquery.esm.js` — 08-31 **03:43**
- this workstream's first compiler build — 08-31 ~08:20
- `lilscript` HEAD commit — 08-30 23:30

So they were produced by the **uncommitted compiler changes already present in this tree** when this
workstream started (`codegen_ir_js.rs`, `lower.rs`, `semantic.rs`, `parser.rs`, `optimizer.rs`), not
by anything done here. That is a testable claim and it is worth testing: two ports with unchanged
inputs got bigger, which is the signature of a codegen regression rather than a scope change.

## The one number that is unambiguous

**markedlil beats upstream `marked.min.js` on Brotli at both revisions** — 9517 committed and 9652
in the tree against 10085 — so the port the objective names by name is a win either way. The
regression narrows the margin; it does not cost the win.
