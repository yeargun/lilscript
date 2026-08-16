# SolidLil Store Brotli drift analysis

Date: 2026-08-14

## Current outcome

The drift was real, but it was not a Store algorithm regression. It came from
scoring compiler layouts at the complete reactive-runtime boundary and then
shipping a different artifact: a tree-shaken, facade-combined, Vite/Oxc-minified
Store entry. Brotli compression is not compositional. The best function order
before tree shaking need not remain best after most functions disappear and the
survivors acquire different neighbors.

The build now performs final-artifact selection. It compiles several viable
runtime candidates, bundles each through the actual Store entry, minifies each
candidate, measures the final chunk with the configured canonical Brotli-11
scorer, and publishes the smallest passing artifact with its provenance.

| Open-world Store bundle | Brotli-11 | Gzip-9 | Raw |
| ----------------------- | ---------: | -----: | --: |
| Official `solid-js/store` | 4,286 B | 4,722 B | 15,793 B |
| SolidLil selected artifact | **4,231 B** | 4,691 B | 15,634 B |
| SolidLil minus official | **-55 B** | -31 B | -159 B |

Brotli-11 is the selection objective. Gzip and raw are reported diagnostics,
not additional objectives for the same compiler output.

## Current final-artifact candidates

Every row below uses the same Store source, public eight-export API, facade,
tree-shaking, Oxc minification, and canonical codec scorer. Only the compiler
candidate changes.

| Candidate | Final Store Brotli-11 | Gzip-9 | Raw |
| --------- | --------------------: | -----: | --: |
| `production-12` | **4,231 B** | 4,691 B | 15,634 B |
| `source-15` | 4,237 B | 4,695 B | 15,668 B |
| `production-15` | 4,253 B | 4,695 B | 15,679 B |
| `layout-18` | 4,253 B | 4,695 B | 15,679 B |

The level-15 production runtime is the smallest complete runtime candidate,
but level 12 wins after the Store boundary removes unused runtime code. The
selector therefore recovers 22 Brotli bytes relative to blindly reusing the
level-15 runtime. This is exactly the boundary mismatch that caused the earlier
drift.

The selected candidate and every rejected candidate are recorded in
`labs/solid-client/artifacts/distribution-selection.json` and copied into
`store-surface.json`. The evidence includes compiler/config/runtime/artifact
hashes and the exact codec implementation.

## Why whole-runtime scoring drifted

The old pipeline effectively did this:

1. compile the complete reactive runtime;
2. choose its best Brotli layout;
3. later tree-shake it for Store;
4. append the JavaScript facade and minify; and
5. measure a final chunk the compiler never saw.

Function-layout search changes both adjacency and identifier allocation. When
Store removes most runtime functions, the repeated byte sequences that made one
layout attractive can disappear. Code from the facade also becomes adjacent to
different runtime fragments. A 50-byte improvement in the pre-link runtime can
therefore become a regression in the final compressed distribution.

The unchanged official baseline was a useful control throughout the incident:
its size stayed fixed while only SolidLil moved. The Store facade and behavior
also stayed stable, and raw movement was tiny. Those facts ruled out a Store
feature or algorithm change and pointed to candidate selection/layout policy.

## Historical controlled attribution

Before final-artifact selection existed, the drift was reproduced by toggling
only compiler search policy on the then-current toolchain:

| Historical compiler mode | Complete runtime Brotli | Final Store Brotli |
| ------------------------ | ----------------------: | -----------------: |
| Production candidate search | **4,358 B** | 4,201 B |
| Candidate search off | 4,450 B | **4,183 B** |
| Production minus search-off | **-92 B** | **+18 B** |

The compiler improved the complete runtime by 92 bytes while making the shipped
Store artifact 18 bytes larger. Feature isolation identified
`function-layout-variants` as the trigger:

| Historical optimization level | Complete runtime Brotli | Final Store Brotli |
| ----------------------------: | ----------------------: | -----------------: |
| 12 | 4,394 B | **4,185 B** |
| 13 | **4,345 B** | 4,223 B |
| 14 | 4,358 B | 4,201 B |
| 15 | 4,358 B | 4,201 B |

Those numbers describe the pre-selector toolchain and are retained as causal
evidence, not as current release measurements. The current measurements are the
candidate table above.

## This was not a compatibility or lifecycle regression

- The exact eight-export Store API still matches `solid-js/store`.
- Immutable/mutable stores, path updates, reconciliation, producer updates,
  tracking, and disposal remain differentially verified.
- The unchanged upstream Solid suite passes 469/469 for both the official and
  SolidLil runtime resolutions.
- Lifecycle stress passes 5,000 root cycles, 500 collection cycles, and 100
  late disposed-resource resolutions with all owner/effect slots released.
- The movement is reproduced by changing only compiler candidate policy.

## Codec selection contract

The open-world configuration declares:

```toml
[javascript]
cost_model = "brotli"
```

That artifact is selected by Brotli. A gzip deployment must compile and select
a separate artifact with `cost_model = "gzip"`; raw-size output likewise needs
its own objective. Different codecs may legitimately prefer different function
orders, spellings, or representations.

Evidence rows must therefore record both the selected cost model and all output
sizes. They must not imply that one compiler output was optimized for Brotli,
gzip, and raw simultaneously.

## Reproduction identity

- Compiler SHA-256:
  `46eeb54e8ee2d737b17fb988494ff968747908fa8fe3ffa8ace676618a2c8189`
- Codec scorer SHA-256:
  `48340e70c3a53ae5f58cee5dbb48ca5553b2feff6f29f086fc8060bd036594c8`
- Selected Store artifact SHA-256:
  `44103291b772cd54e29ca7524d7396941efa1dcd0efaf5151b7c48a8545a1bb5`
- Selection evidence:
  `labs/solid-client/artifacts/distribution-selection.json`
- Store evidence:
  `labs/solid-client/artifacts/store-surface.json`
- Compiler configuration:
  `labs/solid-client/config/open-world.toml`
