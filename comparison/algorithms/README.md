# Escalating algorithm compression corpus

This is the structural layer above `comparison/cases/`. Each folder under
`cases/` contains an independently written LilScript and JavaScript implementation
of the same algorithm, fixed deterministic input vectors, fixed stdout oracles, and
metadata describing structural scale and the optimization opportunities under
pressure. Every manifest also states its optimization hypothesis and a per-metric
`le` (parity) or `lt` (strict typed advantage) expectation; this initial checkpoint
uses conservative parity expectations.

Inputs are supplied at runtime by `host.cjs` through the unresolved functions
`algorithmInt`, `algorithmString`, and `algorithmCount`. This prevents either side
from replacing the complete workload with a precomputed answer. Metadata therefore
records `boundary=runtime-host-script`; the controlled host boundary is identical
on both sides and every vector still has a checked-in fixed oracle.

The hard gate is per case and per metric:

- the original JavaScript, every minified JavaScript candidate, and each LilScript
  lane must reproduce every checked-in stdout oracle and the original JavaScript's
  exact ordered host-access trace, catching duplicated, dropped, or reordered extern calls;
- raw, gzip-9, and Brotli-11 choose their minimum valid JavaScript candidates
  independently;
- three independent LilScript compilations use the raw, gzip, and Brotli configs;
  the raw artifact gates only raw, the gzip artifact only gzip-9, and the Brotli
  artifact only Brotli-11 against that metric's independently selected JS minimum;
- each Lil artifact's other two measured sizes are diagnostic and may lose;
- aggregate wins never offset an individual loss.

Terser, the Oxc minifier exposed by pinned Rolldown, Google Closure Compiler
ADVANCED, and both script-preserving and closed-IIFE esbuild variants compete.
Terser, Oxc, and esbuild target ES2022; Closure emits its newest named mode,
ECMASCRIPT_2021, which is a strict ES2022 subset. Module graphs additionally compete
direct Closure ADVANCED with dependency pruning, direct esbuild, and real Vite/Oxc
and Vite/Terser production bundles. Reports include
exact options, versions, source/config/runner/host/extern/artifact hashes, structural
metadata, all candidate sizes, semantic eligibility, and metric winners.
Cases tagged `safe-property-mangling` also admit a Terser candidate restricted to
the JavaScript source's `_`-prefixed private fields; dynamic dictionary keys are not
silently assumed renameable.

Structural tiers are enforced from the checked metadata: small cases contain three
to seven functions, medium cases eight to nineteen, and large cases at least twenty.
Large cases additionally require at least six modules. Module and function counts are
checked by matching module names and per-module function names across both source
trees. The parsed JavaScript entry call graph must reach every module and at least
the tier's minimum function count; unreachable functions require an explicit DCE or
export-reachability opportunity. Declared call depth is checked over that reachable
graph, and the corpus must retain all three tiers. The current
checkpoint has eleven independently designed cases: five small, five medium, and one
large event-analytics graph with 22 declared functions, 20 entry-reachable functions,
and six entry-reachable modules. Its two unreachable functions are the explicitly
tagged diagnostic export and that export's otherwise-unused helper. Module graphs are tested both
through direct minified ES2022 bundles and as an unminified IIFE fed to the other
candidate lanes.

```sh
nvm use
npm ci --prefix benchmarks/popular
node comparison/algorithms/run.mjs
node comparison/algorithms/run.mjs --only state-machine
```

Unless `LILSCRIPT` and `LILSCRIPT_CODEC` select an explicit compiler/scorer pair,
the runner first builds fresh release `lilscript` and `lilscript-codec` binaries
using `${CARGO:-cargo}`. A single override is rejected so provenance cannot mix
unrelated builds. Generated artifacts and summaries are ignored; the case folders
and configs are durable inputs.
