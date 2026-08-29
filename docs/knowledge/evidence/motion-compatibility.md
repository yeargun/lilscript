# Motion Compatibility Gate

Parent: [Evidence](README.md). Required evidence:
[library proof matrix](library-proof-matrix.md). Live snapshot:
[`docs/current-status.md`](../../current-status.md).

This page defines scope and historical audit work, not current size authority.
Any numerical claim requires a fresh tracked row for the exact named boundary;
partial helper surfaces must not be described as the full Motion package.

## Scope

**DOM / pure TypeScript only.** LilScript targets the npm `motion` package
surfaces that re-export `framer-motion/dom` (plus `motion/mini` and
`motion/debug`). React entry points (`motion/react*`) and other framework
bindings are **out of scope** and are not ported.

## Current status

The in-tree Motion 13 audit under `benchmarks/popular/ports/motion/` mirrors
`motion-utils`, `motion-dom`, and the `framer-motion/dom` surface. The separately
versioned MotionLil package owns current package artifacts and compatibility
commands; this document does not infer its current bytes from the in-tree copy.

**DOM-level progress (compile-green, not yet behavior-certified):**

- `motion-utils`: complete
- `motion-dom`: runtime `.lil` files present for all non-types modules; full
  `motion-dom/index.lil` and public `dom.lil` / `index.lil` compile
- Projection node includes the upstream layout/projection pipeline
  (`applyProjectionStyles`, MotionPath `pathFn` interpolation)
- Entrypoints: `index.lil` / `dom.lil` (≡ `motion` / `framer-motion/dom`),
  `debug.lil` (`recordStats`), `mini.lil` (`animate` / `animateSequence`)
- Selected-surface popular-lab contract `mix` / `wrap` / `stagger` / `spring`
  matches npm `motion@13` on digest
  `motion:14400000:28719240:880000:5494928` and is measured vs Vite 8

Do not claim full Motion DOM compatibility until the gates below are green.
Export-count parity for the DOM surface is approaching npm `motion`
(~366 re-exports in `dom.lil`); treat that as inventory until tests pass.

## Audited upstream scope

The baseline is Motion `v13.0.0` at
[upstream tag `v13.0.0`](https://github.com/motiondivision/motion/tree/v13.0.0).

| In-scope entrypoint | Upstream runtime exports | LilScript |
| --- | ---: | --- |
| `motion` (≡ `framer-motion/dom`) | 312 | `index.lil` / `dom.lil` |
| `motion/debug` | 1 | `debug.lil` |
| `motion/mini` | 2 | `mini.lil` |

Out of scope (not ported, not tracked as blockers):

| Entrypoint | Notes |
| --- | --- |
| `motion/react` | React components / hooks |
| `motion/react-client` | React client bundle |
| `motion/react-m` | Minimal React `m` |
| `motion/react-mini` | React mini hook |

## Definition of complete (DOM)

LilScript may claim Motion **DOM** compatibility only when all of the following
hold:

1. Supported DOM entry points (`motion`, `motion/mini`, `motion/debug`) have a
   versioned LilScript package and an explicit API/export manifest.
2. Public signatures, defaults, overload behavior, callbacks, controls,
   cancellation, timing, errors, and side effects match Motion `v13.0.0` DOM.
3. Applicable upstream DOM unit tests run against compiled LilScript and pass
   without replacing assertions with app-specific snapshots.
4. Browser suites cover Web Animations, requestAnimationFrame scheduling,
   HTML/SVG styles, selectors, observers, scroll, gestures, layout projection,
   interruption, and reduced-motion behavior.
5. Tree-shaken Vite applications import the LilScript DOM package through its
   public API and are compared with the same applications importing Motion DOM.
6. No implementation delegates behavior to the npm Motion runtime. Typed web
   platform declarations are allowed; runtime wrappers are not a port.

An stdout match for one value pipeline satisfies none of these library gates.

## Required language and platform work

The current LilScript language is missing facilities used throughout Motion DOM:

- structural object types, broad unions, optional properties, overloads, and
  generic collection/object utilities;
- exceptions, promises/thenables, async behavior, rest/spread, dynamic key
  access, and selected reflection/type-guard behavior;
- a versioned DOM declaration package covering Element/HTMLElement/SVG,
  events, Web Animations, requestAnimationFrame, observers, scrolling, media
  queries, CSS style access, and document/window scheduling boundaries;
- deterministic browser test support with fake clocks and frame scheduling;
- package-library emission, declaration generation, and compatibility-aware
  tree shaking across public entry points.

These are compiler/language features, not syntax aliases for TypeScript.

## Port sequence

1. Port `motion-utils` pure utilities and run translated upstream unit tests.
2. Implement frame scheduling, subscriptions, easing, interpolation, and value
   animation primitives with deterministic clocks.
3. Port MotionValue, keyframes, springs, inertia, sequences, and controls.
4. Implement the `motion/mini` WAAPI surface against typed browser APIs.
5. Port the hybrid DOM renderer, HTML/SVG value handling, effects, and scroll.
6. Port gestures and layout projection with browser integration tests.
7. Run upstream DOM unit/Playwright suites and only then publish size/performance
   comparisons for identical Vite applications.

Each stage must keep the JavaScript implementation as the behavioral oracle,
record unsupported tests explicitly, and reject compatibility claims while any
required test is skipped.

## Benchmark rules

- Closure receives the exact readable JavaScript reference measured by the
  ordinary JS rows.
- LilScript uses the same algorithm and abstraction scope; intentionally
  specialized LilScript is labeled as a diagnostic.
- Hand-specialized JavaScript is an optimization oracle, not a compiler input.
- Real npm/Vite package builds remain context only until a complete equivalent
  LilScript package passes the compatibility gate.
- Corpus totals contain only artifact kinds present with the same meaning for
  every workload.
