# 022 — Refreshing the official markdown-stack report, as far as it will go

**Status: PARTIALLY REFRESHED (11 of 16 ports) and the blocker for the other five is identified.**

## Why

[014](../014-dirty-tree-scoreboard/README.md) established that
`comparison/markdown-stack/REPORT.md` (generated 2026-08-30) measures a mid-migration working tree,
and predicted from direct `dist/` measurement that at least two verdicts were stale. Rather than keep
quoting my own out-of-band numbers, the right move is to re-run the project's own harness.

## The harness will not fully run

`node comparison/markdown-stack/run.mjs --measure` fails closed:

```
Error: remark: package scripts differ from the manifest
```

Checked one port at a time, **five of sixteen block it**:

| port | `--check-inputs` |
|---|---|
| remark, unified, rehype, rehype-katex, react-markdown | **`package scripts differ from the manifest`** |
| the other eleven | ok |

Those five ports' `package.json` scripts have drifted from `manifest.json` since it was pinned. **That
is why REPORT.md is stale: nobody can regenerate it.** Reconciling those five files is a small,
mechanical job and it unblocks the project's own scoreboard.

## The eleven that do run

Measured with the harness's own pinned lanes and codec — `lil-graph` against `official-terser`, which
is the comparison REPORT.md makes:

| port | REPORT.md | fresh | change | verdict |
|---|---:|---:|---:|---|
| micromark | +4568 | +4154 | −414 | LOSS |
| mdast-util-from-markdown | +3573 | +3175 | −398 | LOSS |
| remark-parse | +3738 | +3235 | −503 | LOSS |
| katex | +6532 | +5800 | −732 | LOSS |
| remark-math | +450 | +137 | −313 | LOSS |
| **remark-gfm** | **+379** | **−383** | **−762** | **LOSS → WIN** |
| mdast-util-to-hast | −726 | −752 | −26 | WIN |
| hast-util-to-html | −1028 | −1014 | +14 | WIN |
| remark-rehype | −671 | −687 | −16 | WIN |
| rehype-stringify | −745 | −794 | −49 | WIN |
| remark-breaks | −67 | −70 | −3 | WIN |
| | | | **−3202** | **6 W / 5 L** |

**Every port improved or held**, for a net **−3202 Brotli** against the published numbers, and
**remark-gfm flips from loss to win** — confirming, through the project's own harness, what
[014](../014-dirty-tree-scoreboard/README.md) predicted from direct `dist/` measurement.

## What this is and is not

This is the harness's own verdict on the eleven ports it can still verify, so it supersedes both
REPORT.md and my out-of-band `dist/` figures for those rows. It says nothing about the other five —
including `rehype`, the port [006](../006-markdown-stack-loss-diagnosis/README.md) found a
`minifyWhitespace` build bug in and [014](../014-dirty-tree-scoreboard/README.md) predicted would
also flip.

Reproduce:

```sh
node comparison/markdown-stack/run.mjs --measure --json out.json \
  --only micromark,mdast-util-from-markdown,remark-parse,mdast-util-to-hast,\
hast-util-to-html,remark-rehype,rehype-stringify,remark-gfm,remark-breaks,remark-math,katex
```
