# Escalating algorithm challenges

Parent: [verification](README.md). Micro foundation:
[coverage matrix](coverage-matrix.md). Executable lane:
[`comparison/algorithms/`](../../../comparison/algorithms/). Evidence:
[algorithm suite](../evidence/algorithm-suite.md).

Micro cases answer “which local rule drifted?” They cannot answer whether the
compiler makes good whole-program choices once call graphs, representations,
dictionaries, mangling, and codec windows interact. Algorithm challenges are a
separate hard-gated corpus for that question. A hundred parameters of one constant
fold remain one semantic family; they do not count as a hundred structural designs.

## Pair contract

Each challenge starts from a web-relevant algorithm and a public behavioral
contract. The JavaScript author may use normal modern-JS idioms and compression
tricks that Terser, Oxc, esbuild, Vite, or Closure can exploit. The LilScript author
implements the same logic idiomatically with typed constructs and intrinsics. The
two sources need not be token translations, but they must expose equal boundaries,
consume the same deterministic runtime vectors, and produce the same values,
effects, errors, and ordering.

The JavaScript must be credible on its own. Do not inflate it with avoidable metadata,
legacy compatibility, verbose helpers, string enums when numbers suffice, or an API
surface LilScript does not pay for. Conversely, do not give LilScript closed-world
knowledge that enters JavaScript through a public or host boundary. Adapters and
extern declarations are counted in the artifact that needs them.

Every emitted candidate executes every vector before becoming size-eligible. Fixed
oracles are reviewed and checked in; a runner must never silently regenerate them
from the current reference program. For the current runtime-host boundary, the
runner checks both fixed stdout and the original JavaScript's exact ordered host
access trace, including host-function name, argument index, order, and call count.
This catches duplicated, dropped, or reordered opaque reads. One stdout sample is
insufficient for stateful, async, error, identity, API-descriptor, module, or browser
contracts.

## Structural tiers

Metadata records actual counts from the maintained sources, not aspirational labels.

| Tier | Minimum structure | Purpose |
|---|---|---|
| small structural | 3–7 functions, 1–2 modules, at least one runtime boundary | expose cross-function choices without a large diagnosis surface |
| medium | 8–19 functions, 2–5 modules or equivalent independent components | combine proofs, naming, layout, effects, and helper reuse |
| large | 20+ functions, 6+ modules where modules are part of the contract | cross codec windows and exercise whole-artifact layout/chunk decisions |

Every manifest also records call-graph depth, public/host boundary count, input-vector
count, and emitted raw-size band. Validation matches module stems, per-module function
names, and import edges across the LilScript and JavaScript trees. The parsed entry
graph must reach every module and at least the tier's minimum function count;
unreachable functions require an explicit DCE or export-reachability opportunity.
A tier is not complete because one enormous source file contains many trivial wrappers.

## Required opportunity matrix

Each row needs multiple independently designed challenges and at least one interaction
with another row.

| Opportunity | What strong JavaScript tools may do | What LilScript should prove |
|---|---|---|
| propagation and DCE | fold constants, eliminate branches/unused functions/exports | typed interprocedural facts and whole-graph reachability are at least as effective |
| names and properties | frequency mangle locals, top-level names, safe owned properties | typed ownership protects ABI names while shortening every owned name it can |
| inline, share, outline | choose call overhead against duplicated bodies and reusable helpers | codec-scored inlining/specialization/subsumption/outlining finds the artifact win |
| dictionary and order | repeat literals/tokens, reorder declarations, exploit gzip/Brotli windows | pooling, spelling, and function/layout search use complete encoded cost |
| aggregates and collections | scalar replace, choose arrays/objects, fuse known operations | nominal layout, escape proofs, intrinsics, and projection remove more runtime shape |
| control flow and state | compress switches, loops, early exits, exception/generator machines | structured regions and typed ranges admit shorter equivalent spellings |
| modules and exports | tree-shake, internalize, chunk, preserve required public names | linker knowledge and explicit ABI/lazy edges beat or match eligible bundlers |
| host and browser | preserve opaque effects and exact platform names | typed extern boundaries stop optimization precisely, without global pessimism |

Scale variants must add real work: more states, fields, callbacks, graph nodes, or
repeated semantic motifs. Inert padding and cloned dead text are forbidden. Useful
size bands include roughly 100–399, 400–999, 1–10 KiB, and above a meaningful codec
window; cohort concatenation may be reported but never excuses a per-case loss.

## Hard size contract

For these closed runtime scripts, the required frontier includes Terser, Oxc,
Closure `ADVANCED`, and esbuild script/IIFE candidates. Module graphs add a direct
dependency-pruned Closure `ADVANCED` graph, Vite/Oxc, Vite/Terser, and direct esbuild
bundle candidates. The three host functions are explicit Closure externs and the
browser environment supplies `console`; no public API is silently internalized.
A candidate that fails any stdout or host-access oracle is both a case failure and
ineligible for the size frontier.

For metric `m`, select the smallest valid JavaScript artifact independently:

```text
best_js[m] = min(valid baseline artifact size under m)
lil_raw.raw       <= best_js.raw
lil_gzip.gzip9    <= best_js.gzip9
lil_brotli.brotli11 <= best_js.brotli11
```

Strict-win expectations are used only where the challenge names a typed structural
advantage. Aggregate totals are diagnostic: no win in another case or metric can pay
for a losing row. Raw, gzip-9, and Brotli-11 can select different LilScript artifacts
and different JavaScript tools. The two non-objective sizes measured from each
LilScript artifact are diagnostic and may lose.

## Growth rule

New challenges progress in three steps:

1. Add a minimal structural case that isolates the expected whole-program choice.
2. Add interactions and runtime-varying vectors that invalidate a naive shortcut.
3. Add medium/large siblings that cross helper, pooling, naming, and codec-window
   thresholds.

A red case is retained. Triage first verifies pairing and tool eligibility, then
classifies missing analysis, missing transform, candidate-generation/search loss,
emission spelling, codec-selection loss, boundary tax, or deliberate runtime/config
tradeoff. Fix the compiler or explicitly narrow the supported contract; never weaken
the oracle, baseline, codec, or size gate merely to turn the report green.
