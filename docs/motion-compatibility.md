# Motion Compatibility Gate

## Current status

LilScript does **not** currently implement Motion. Installing `motion` in the
benchmark workspace provides a JavaScript ecosystem reference only. The
LilScript `motion-values` program is an animation-value compiler workload; it
does not provide Motion's public package, DOM engine, or React integrations.

This distinction is enforced in the benchmark harness: real Motion output is a
context-only Vite production build and is excluded from Closure/LilScript
deltas and corpus totals.

## Audited upstream scope

The baseline is Motion `v13.0.0` at
[upstream tag `v13.0.0`](https://github.com/motiondivision/motion/tree/v13.0.0).
A source audit on 2026-08-06 found:

| Package | TypeScript source files | Source lines | Test files |
| --- | ---: | ---: | ---: |
| `motion` | 7 | 22 | 0 |
| `motion-utils` | 51 | 843 | 19 |
| `motion-dom` | 319 | 28,652 | 67 |
| `framer-motion` | 306 | 34,444 | 99 |
| **Total** | **683** | **63,961** | **185** |

The installed `motion` root exposes 312 runtime exports. Its package manifest
also publishes root, debug, mini, React, React client, React `m`, and React mini
entry points. The upstream repository contains 196 test files overall and a
separate Playwright command.

These counts are inventory, not a progress percentage: exports differ greatly
in complexity and some behavior is browser- or scheduler-dependent.

## Definition of complete

LilScript may claim Motion compatibility only when all of the following hold:

1. Every supported upstream entry point has a versioned LilScript package and
   an explicit API/export manifest.
2. Public signatures, defaults, overload behavior, callbacks, controls,
   cancellation, timing, errors, and side effects match Motion `v13.0.0`.
3. The applicable upstream unit tests run against compiled LilScript and pass
   without replacing assertions with app-specific snapshots.
4. Browser suites cover Web Animations, requestAnimationFrame scheduling,
   HTML/SVG styles, selectors, observers, scroll, gestures, layout projection,
   interruption, and reduced-motion behavior.
5. React entry points pass their upstream tests, or are explicitly excluded
   from a separately named DOM-only compatibility level.
6. Tree-shaken Vite applications import the LilScript package through its
   public API and are compared with the same applications importing Motion.
7. No implementation delegates behavior to the npm Motion runtime. Typed web
   platform declarations are allowed; runtime wrappers are not a port.

An stdout match for one value pipeline satisfies none of these library gates.

## Required language and platform work

The current LilScript language is missing facilities used throughout Motion:

- structural object types, broad unions, optional properties, overloads, and
  generic collection/object utilities;
- exceptions, promises/thenables, async behavior, rest/spread, dynamic key
  access, and selected reflection/type-guard behavior;
- a versioned DOM declaration package covering Element/HTMLElement/SVG,
  events, Web Animations, requestAnimationFrame, observers, scrolling, media
  queries, CSS style access, and document/window scheduling boundaries;
- deterministic browser test support with fake clocks and frame scheduling;
- package-library emission, declaration generation, and compatibility-aware
  tree shaking across public entry points;
- React interop sufficient for hooks, components, contexts, refs, and client
  boundaries if the React exports are in scope.

These are compiler/language features, not syntax aliases for TypeScript.

## Port sequence

1. Port `motion-utils` pure utilities and run translated upstream unit tests.
2. Implement frame scheduling, subscriptions, easing, interpolation, and value
   animation primitives with deterministic clocks.
3. Port MotionValue, keyframes, springs, inertia, sequences, and controls.
4. Implement the `motion/mini` WAAPI surface against typed browser APIs.
5. Port the hybrid DOM renderer, HTML/SVG value handling, effects, and scroll.
6. Port gestures and layout projection with browser integration tests.
7. Add React entry points only after the DOM core and React interop are stable.
8. Run upstream unit/Playwright suites and only then publish size/performance
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
