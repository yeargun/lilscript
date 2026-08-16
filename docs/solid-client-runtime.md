# SolidLil client-runtime evidence

The root-owned [`labs/solid-client`](../labs/solid-client) workspace contains
the Lilscript reactive engine, Solid-compatible JavaScript facades, pinned
upstream candidate gate, lifecycle/memory harnesses, and the separate `.lilx`
parser/lowerer.

## Exact pinned browser runtime

Baseline: `solid-js@1.9.13` at
`3be495cec52bf78d7cc61f054af00320ecf4058c`.

- 469/469 unchanged upstream candidate tests pass across 26 files.
- 135/135 Core, Web, and Store exports exist and have differential evidence.
- Browser value types and observable function arities match for all 135.
- A separate 112-case Lilscript corpus passes maximum and disabled
  optimization on the JavaScript target (224 executions).

The curated corpus is not a `112/469` percentage. It exercises the runtime from
Lilscript source under compiler variants; the unchanged candidate suite is the
broader public-entry compatibility gate. C/native do not participate because
they do not yet implement Solid's required JavaScript Promise and exception
semantics.

## Brotli-first reusable surfaces

Each row is one independently bundled open-world browser ESM entry. Lower is
better.

| Surface                    | Exports | Official Brotli | SolidLil Brotli | Official gzip | SolidLil gzip |
| -------------------------- | ------: | --------------: | --------------: | ------------: | ------------: |
| Core                       |      54 |         8,551 B |         8,433 B |       9,422 B |       9,340 B |
| Store                      |       8 |         4,286 B |         4,231 B |       4,722 B |       4,691 B |
| Client Web                 |      46 |        10,859 B |        10,643 B |      12,103 B |      11,939 B |
| Full Web compatibility¹    |      73 |        11,655 B |        11,667 B |      12,977 B |      13,074 B |

Core, Store, and the declared client Web target strictly win Brotli-11. Raw and
gzip bytes remain visible diagnostics; a Brotli-selected artifact is not
presented as simultaneously optimized for another codec. Client Web excludes
SSR and hydration. The full 73-export browser entry is retained as a separate
compatibility diagnostic and currently has a 12 B Brotli gap; it is not folded
into the client-only claim or treated as release-eligible. Evidence is published in
`artifacts/build-modes.json`, `artifacts/store-surface.json`,
`artifacts/web-client-surface.json`, and `artifacts/web-surface.json`.

¹ Includes server/hydration-facing compatibility exports outside the declared
client implementation target.

Open-world builds are the actual reusable distribution bundles above: unknown
external consumers may access every listed export, so those names remain
stable. The separate closed-world diagnostic owns the complete consumer graph;
unused exports may disappear and retained exports may be renamed. Closed-world
Vite/LSX application rows must not be compared as though they were package
distribution bundles. Core, Store, and Web distribution candidates are scored
after the actual entry has been tree-shaken and minified, so a whole-runtime
layout cannot win merely by compressing well before the shipped boundary.

## Ownership, unmount, and retained memory

The deterministic Solid/SolidLil lifecycle workload covers:

- root, signal, memo, effect, and reverse cleanup churn;
- idempotent unmount and stale disposers after internal slot reuse;
- keyed `mapArray` and positional `indexArray` cleanup;
- resource promises resolved after their roots are disposed; and
- stable owner/effect pool capacity.

The combined workload stabilizes at eight owner and sixteen effect slots. All
slots are free and the pending queue is empty afterward.

The resource gate is a real Playwright Chromium experiment, not a simulated
DOM proxy. It uses 32 randomized complete paired blocks, fresh incognito
contexts/pages per artifact, Chromium restarts after every balanced block
cycle, 500 untimed warmups, and 4,000 measured interactions. Cold samples use a
unique source URL to defeat code-cache reuse. CDP supplies main-thread
`TaskDuration`, forced collection, JavaScript/Oilpan heap, DOM/listener counts,
and summed Chromium-process RSS. No observations are removed or winsorized.

| Closed-world lane | Warm wall ratio (95% CI) | Warm CPU ratio (95% CI) | Eligibility |
| ----------------- | ------------------------: | -----------------------: | ----------- |
| Vite counter      | 0.659 [0.651, 0.668]      | 0.683 [0.677, 0.690]     | pass        |
| Closure ADVANCED  | 0.650 [0.643, 0.657]      | 0.674 [0.670, 0.679]     | pass        |
| Complete LSX      | 0.973 [0.964, 0.984]      | 0.979 [0.970, 0.988]     | pass        |

Ratios are paired geometric means; lower is better. The 95% upper confidence
bound must remain at or below 1.03 for warm CPU and wall time. Cold wall and
first-interaction gates use a 0.25 ms absolute allowance because ratio tests are
unstable near zero. LSX's first interaction is slightly slower at the point
estimate (+0.080 ms CPU, +0.037 ms wall), but its upper bounds (+0.133 ms and
+0.072 ms) remain well inside that allowance.

| Lane | Live JS-heap delta | Disposed JS-heap delta | Live RSS delta | Disposed RSS delta |
| ---- | -----------------: | ----------------------: | -------------: | -----------------: |
| Vite |          -26,883 B |               +10,860 B |   -2,520,064 B |       -2,473,472 B |
| Closure |       -47,970 B |                -8,152 B |   -3,930,624 B |       -3,885,568 B |
| LSX  |          +32,246 B |               +85,918 B |   -8,017,920 B |       -7,829,504 B |

Those are paired mean candidate-minus-Solid deltas after four forced GCs and
two animation-frame turns. JavaScript heap, combined JavaScript+Oilpan heap,
and total process RSS all pass their predeclared 95% confidence gates. Oilpan
splits, backing storage, DOM nodes, and listener counts remain recorded as
diagnostics rather than being selectively omitted.

## Complete client-rendering LSX contract

Runtime parity does not imply JSX/LSX compiler parity, so the client LSX gate is
measured separately:

| LSX gate                    | Passed | In-scope |
| --------------------------- | -----: | -------: |
| Lowering verified           |     21 |       21 |
| Integrated runtime evidence |     21 |       21 |
| Server-coupled exclusions   |      2 |        2 |

The client differential includes user components and live props, host and
component spreads, nested control-flow composition, component rows in keyed and
indexed lists, Dynamic element/component/null and SVG selection, Portal
head/SVG/shadow-root variants, namespace attributes, branch disposal, and full
unmount. Keyed Show/Match raw-value callbacks, non-keyed live accessors, and
ordered match short-circuiting are also differential-tested. `For` accepts full
LilScript callback types and proves Solid-compatible prefix/suffix duplicate
identity, fallback ownership, removal cleanup, and unmount. Hydration and SSR
are explicitly excluded because they require a coordinated server runtime.
Suspense and ErrorBoundary are included, with construction and delayed errors,
reset/remount identity, multi-resource reveal, pending-content ownership,
pending unmount, cleanup, and minified-bundle evidence.

The current integrated fixture is also a production size lane. Both entries use
Vite 8/Oxc; SolidLil includes its DOM host ABI, and the behavior/unmount harness
must pass before the Brotli-first build gate is published. This is complete
evidence for the declared client-only contract, not an SSR/hydration claim.

| Integrated LSX fixture  | Brotli-11 |   Gzip-9 |      Raw |
| ----------------------- | --------: | -------: | -------: |
| Official Solid JSX      |  12,560 B | 13,866 B | 38,629 B |
| SolidLil LSX + host ABI |  10,741 B | 12,035 B | 34,209 B |

SolidLil is 14.5% smaller on the primary Brotli-11 metric for this exact
client-only fixture. Its Playwright lane is also statistically faster in the
warm loop and passes every cold, first-interaction, heap, RSS, lifecycle, and
teardown non-inferiority gate above.

The historical todolist remains useful archived application evidence:

| Artifact              | Brotli-11 |  Gzip-9 |      Raw |
| --------------------- | --------: | ------: | -------: |
| Solid JSX todolist    |   5,479 B | 6,077 B | 15,456 B |
| SolidLil LSX todolist |   3,722 B | 4,226 B | 10,590 B |

That row stays excluded from the verified aggregate because it is an archived
snapshot. The live integrated LSX fixture above is the reproducible benchmark.

## Reproduce

```sh
cd labs/solid-client
npm run test
npm run test:lilx
npm run test:upstream:candidate
npm run test:compat
npm run test:lifecycle
npm run build
npm run verify
npm run audit:api
npm run audit:lsx
npm run verify:store
npm run verify:web
npm run benchmark
npm run publish:web
```

The generated web evidence powers the Brotli-first selectable comparison on
`/libraries.html` and the filterable catalog on `/explorer.html`.
