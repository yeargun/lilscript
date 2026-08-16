# LilScript Solid laboratory

This repository compares an official SolidJS v1.9.13 application with an
equivalent application backed by executable LilScript runtime and typed DOM
slices. The LilScript application creates its element tree, reactive text
bindings, attributes, and delegated event handlers in `.lil` source; a small
host ABI maps dynamic node and event handles to browser objects. Statically
named browser operations use LilScript's direct typed host-object ABI; the
reactive document attribute path emits
`document.documentElement.setAttribute(...)` without a wrapper.
The official source is pinned, unmodified, in `upstream/solid`; the current port
coverage and blockers are recorded in [PORT_STATUS.md](PORT_STATUS.md).

## Setup

```sh
nvm use
npm install
npm run setup
npm run check
```

The lab uses the compiler from this monorepo root. Set
`LILSCRIPT_ROOT=/absolute/path/to/repository` for another source checkout, or
`LILSCRIPT_COMPILER=/absolute/path/to/lilscript` to use a prebuilt executable
without invoking Cargo. When overriding the compiler, also set
`LILSCRIPT_CODEC=/absolute/path/to/lilscript-codec`; every size report fails
closed unless the shared scorer is available. The default setup builds both
binaries and records the scorer's statically bundled upstream zlib C 1.3.1 and
official Google Brotli C 1.1.0 identities.

## Development

```sh
npm run dev:lilscript  # http://127.0.0.1:5180
npm run dev:solid      # http://127.0.0.1:5181
```

The local Vite plugin resolves the complete `.lil` import graph through the
native compiler on every transform and triggers a full-page hot reload after a
LilScript edit. JavaScript/JSX uses the official Solid Vite plugin. The current
host ABI core lives in `apps/lilscript/src/host.js`; application and
reactive/DOM behavior live in LilScript rather than pre-rendered JavaScript
markup. Exact-name Web API declarations remain unmangled, while handle shims
are retained for operations with dynamic property names, heterogeneous DOM
identity, event registration tables, and reconciliation. Apps using `Dynamic`,
`dynamicShow`, `dynamicFor`, `dynamicIndex`,
`dynamicElement`, or `dynamicComponent` also import `host-regions.js`, keeping
comment-anchor reconciliation bindings out of bundles that do not use them.
Reactive properties, boolean attributes, class toggles, and style bindings use
the separately opt-in `host-properties.js`. Event strategy is also explicit:
`host-listeners.js` supplies direct listeners, while `host-events.js` supplies
one document listener per delegated event type. The comparison app imports only
the delegated strategy. `host-elements.js` provides the complete pinned Solid
SVG intrinsic tag table plus explicit typed SVG and MathML constructors; apps
using namespace-aware `Dynamic` or `dynamicElement` import it separately.
Owned portals use `host-portals.js` plus `host-regions.js`; SVG variants also
use `host-elements.js`. Portal apps select `host-portal-events.js` instead of
the basic `host-events.js`, so delegated events cross a weak logical-host map
without charging ordinary event bundles for portal traversal.

## Open world and closed world

The lab treats build mode as an explicit ABI contract. This is the same split
used by mainstream JavaScript tooling: a published package normally keeps its
documented exports because its future consumers are unknown, while a consumer
application bundle knows the complete import graph and can tree-shake or rename
those exports.

| Mode         | Contract                                                                                   | Config                     |
| ------------ | ------------------------------------------------------------------------------------------ | -------------------------- |
| Open world   | Reusable ESM names and public aggregate fields stay stable                                 | `config/open-world.toml`   |
| Closed world | The application graph is linked first; every LilScript-owned name may be renamed or erased | `config/closed-world.toml` |

`npm run verify:modes` compiles the reactive runtime as an open module, exposes
the complete 52-export browser Core surface through the `packages/solidlil`
facade, imports both official Solid and SolidLil bundles, and executes every
export through reactive, utility, scheduler, resource, error, transition,
Suspense, and external-source digests. It also emits the runtime with
closed-world export mangling and requires every runtime export to be renamed.
Brotli-11 transfer size is the primary release gate. Gzip-9 and raw bytes remain
reported diagnostics, so a raw-size tradeoff cannot be hidden behind
compression.

The Vite application and raw compiler lanes always use the closed-world config.
Their generated artifacts are tested after Vite/Oxc and Closure ADVANCED. The
client Web distribution also receives a compile-time client-only flag, allowing
hydration branches to disappear without weakening the separately tested
73-export compatibility entry.

Reusable Core, Store, and Web rows run a bounded final-artifact selection step.
Several behavior-equivalent compiler representations are each tree-shaken and
minified through the real public entry; canonical Brotli-11 scores the resulting
chunk. This matters because function order and identifier allocation can become
better or worse after a downstream bundler removes exports. Every candidate,
hash, size, and selected winner is retained in
`artifacts/distribution-selection.json`.

## Exact parity ledger

```sh
npm run audit:api    # refresh the truthful, non-blocking ledger
npm run parity:api   # strict 100% export + behavior gate
```

The ledger inventories `solid-js`, `solid-js/web`, and `solid-js/store` from the
pinned Solid 1.9.13 browser ESM dependency. An export is “verified” only when it
has differential evidence and its public value type and function arity match; a
same-named function alone does not count. The strict API gate now passes at
135/135. This does not waive the separate LSX, server-target, type-system, size,
or performance gates.

The LSX frontend follows the same rule:

```sh
npm run test:lilx     # parser/lowering contracts
npm run audit:lsx    # non-blocking feature-family ledger
npm run parity:lsx   # strict lowering + integrated runtime gate
```

`tooling/lilx` and its Vite transform now live in this monorepo. The integrated
Solid-versus-SolidLil fixture covers user components, live host/component
spreads, nested Show/Switch control flow, component rows in For/Index, Dynamic
element/component/null and SVG selections, Portal variants, namespace
attributes, keyed Show/Match values and live accessors, immediate branch
cleanup, and idempotent unmount. The strict 21/21 client-rendering LSX gate
passes; hydration and SSR remain explicitly excluded server-coupled systems,
not implied client gaps. `For` additionally proves full typed callbacks,
Solid-compatible duplicate-key identity, fallback ownership, and removal
cleanup. ErrorBoundary and Suspense cover construction and delayed reactive
errors, reset/remount identity, multi-resource reveal, retained pending content,
pending unmount, cleanup, and runtime-slot release. `npm run build` also bundles
this exact differential fixture through official Solid JSX and SolidLil LSX;
the candidate includes its production DOM host ABI, and Brotli-11 is gated
first. Generated reports carry the current size, interaction, and retained-heap
measurements; the old todolist snapshot remains historical context only.

## Reproducible comparison

```sh
npm run compare
```

This command builds both production applications, produces Closure ADVANCED
variants, executes both Vite bundles, both Closure bundles, raw compiler output,
and the integrated LSX fixtures in Playwright Chromium, and measures
raw/gzip/Brotli bytes. Size gates cover open-world Core/Store/Web distributions
plus closed-world Vite, Closure, and integrated LSX applications. Brotli-11 is
listed and gated first; gzip and raw describe that same Brotli-selected artifact
but do not become extra gates.

CPU and RAM observations use 32 randomized complete blocks, ending on a
complete position-and-carryover-balanced cycle. Every artifact observation gets
a fresh incognito context and page; Chromium restarts after each complete `2n`
cycle. A block-specific source URL defeats code-cache reuse for cold
parse/eval/mount. Warm loops follow 500 untimed browser interactions. Chromium
CDP supplies main-thread `TaskDuration`, JavaScript heap, Oilpan/embedder heap,
DOM/listener counts, and process identities; RSS sums every reported Chromium
process. Four forced collections and two animation frames precede each memory
snapshot so GC finalizer tasks are not charged to the next timed operation.

Results use paired geometric ratios, deterministic 95% bootstrap intervals,
and paired sign-flip permutation tests. No observation is deleted, trimmed, or
winsorized. Warm CPU/wall time use a 3% upper confidence bound. Sub-millisecond
cold wall and first-interaction measurements use 0.25 ms absolute upper bounds;
cold CDP CPU remains diagnostic because unrelated renderer tasks can dominate
such a short interval. RAM gates JavaScript heap, combined JS+Oilpan managed
heap, and total Chromium RSS at cold, live, and post-unmount phases. The
component heaps remain separately visible. A deterministic lifecycle gate adds
stale-disposer checks after slot reuse, keyed/positional row cleanup, resources
resolved after disposal, and stable owner/effect high-water checks.
Generated bundles and reports are checked in under `artifacts/` so they
can be inspected without rebuilding.

Application bundle and timing results are valid only for this equivalent app.
The reusable Core/Web/Store reports are separate exact-surface comparisons.
Neither establishes universal performance for arbitrary applications; the
report establishes this fixture in the pinned Playwright/Chromium environment.

## Full compatibility gate

`npm run test:upstream` executes the unchanged pinned SolidJS reference fixture.
`npm run test:upstream:candidate` runs the same 469 tests in 26 files unchanged
with public Core/Web/Store entries resolved to SolidLil. The additional
LilScript compatibility corpus and remaining compiler capabilities are tracked
in [compatibility/README.md](compatibility/README.md). Its 112 cases and the
upstream 469 tests are deliberately reported as separate inventories.

`npm run test:compat` compiles and executes all 112 curated LilScript behaviors
with maximum optimization and with optional optimizations disabled on the
JavaScript target (224 executions). `npm run test:lifecycle` is the fast
deterministic ownership/unmount gate; `npm run benchmark` performs the
randomized Playwright CPU/RAM gate. The app, Web-runtime, public Web-surface,
and LSX differential behavior suites also run inside Chromium rather than a DOM
emulator.

Benchmark controls:

| Variable                                     |            Default | Meaning                                                     |
| -------------------------------------------- | -----------------: | ----------------------------------------------------------- |
| `LILSCRIPT_SOLID_PERF_SAMPLES`               |                 32 | Balanced Playwright CPU/RAM blocks per app artifact         |
| `LILSCRIPT_SOLID_LSX_PERF_SAMPLES`           |                 32 | Balanced Playwright CPU/RAM blocks per LSX artifact         |
| `LILSCRIPT_SOLID_LIFECYCLE_CYCLES`           |               5000 | Deterministic root/signal/memo/effect/cleanup cycles        |
| `LILSCRIPT_SOLID_UPDATES`                    |               4000 | Timed counter updates after warmup                          |
| `LILSCRIPT_SOLID_WARMUPS`                    |                500 | Untimed app updates before the warm loop                    |
| `LILSCRIPT_SOLID_LSX_UPDATES`                |               4000 | Timed LSX updates after warmup                              |
| `LILSCRIPT_SOLID_LSX_WARMUPS`                |                500 | Untimed LSX updates before the warm loop                    |
| `LILSCRIPT_SOLID_BOOTSTRAP_ITERATIONS`       |              10000 | Paired bootstrap and permutation resamples                  |
| `LILSCRIPT_SOLID_BENCHMARK_SEED`             | `solidlil-2026-08` | Reproducible block randomization and statistical resampling |
| `LILSCRIPT_SOLID_MAX_RATIO`                  |               1.03 | Warm CPU/wall and heap upper 95% ratio boundary             |
| `LILSCRIPT_SOLID_COLD_WALL_DELTA_MS`         |               0.25 | Cold wall-time upper 95% absolute delta                     |
| `LILSCRIPT_SOLID_FIRST_INTERACTION_DELTA_MS` |               0.25 | First-interaction upper 95% absolute delta                  |
| `LILSCRIPT_SOLID_JS_HEAP_MARGIN_BYTES`       |             131072 | JS-heap upper 95% absolute fallback bound                   |
| `LILSCRIPT_SOLID_MANAGED_HEAP_MARGIN_BYTES`  |             262144 | JS+Oilpan upper 95% absolute fallback bound                 |
| `LILSCRIPT_SOLID_RSS_NOISE_BYTES`            |            4194304 | Total Chromium RSS upper 95% absolute allowance             |
| `LILSCRIPT_SOLID_ALLOW_SMALL_SAMPLES`        |                  0 | Explicit local-only override for protocol smoke tests       |
