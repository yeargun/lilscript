# Vue 3.5.42 port status

## Current state

The upstream source is pinned and cloned. Full Vue compatibility is not yet
implemented, and no final size or performance win is claimed.

| Gate | Current result | Status |
| --- | ---: | --- |
| Source files satisfied | 92 / 234 | Failed closed |
| Upstream test files | 70 / 196 | Incomplete |
| Declaration test files | 0 / 20 | Incomplete |
| Package entrypoints | 0 / 12 | Incomplete |
| Published formats | 0 / 7 | Incomplete |
| Reactivity project diagnostic | Candidate 5,509 B vs Vue 4,657 B Brotli-11 | Failed |
| Required project-size scenarios | 0 / 4 | Pending |
| Runtime performance | Not measured | Pending |
| GitHub Pages evidence | Generated, incomplete | Pending |

The complete runtime export surfaces of `@vue/shared` (73 values) and
`@vue/reactivity` (50 values) are implemented. The candidate passes all 62
unchanged shared tests and all 440 unchanged reactivity tests. Package formats,
declarations, and downstream Vue packages remain open, so neither package is
yet marked fully complete in the repository-wide gate.

Runtime-core currently implements 84 of 146 runtime exports and passes 244
unchanged upstream tests in 12 files. Compiler-core exposes all 145 upstream
runtime bindings plus 13 intentional test-only source-module bindings, passes
all 671 unchanged tests in its 20 files, and has all 30 source files mapped to a
declaration-free entry barrel plus 29 verified owner modules.

The source-parity audit derives 234 tracked `packages/*/src` TypeScript files
directly from Git: 231 algorithm/runtime modules and 3 type-only modules. All 15
runtime/mixed `@vue/shared` files and all 13 `@vue/reactivity` files now have
conforming hash-pinned one-to-one mappings. `typeUtils.ts` has explicit
declaration-only handling because it emits no runtime code. All 30 compiler-core
sources now have conforming hash-pinned mappings: `index.ts` maps to a pure
import/re-export barrel and the other 29 files retain their upstream module
responsibilities. Across the full scope there are 140 absent deterministic
paths, no present but unverified paths, and 2 paths occupied by legacy
package-wide monoliths.

Compiler-dom has all 16 source files mapped, including the exact static-stringify
transform and its compile-time hoist wiring. All 13 unchanged upstream files
pass all 135 tests.

Compiler-ssr has all 17 source files mapped to independent owner modules and
preserves the upstream two-pass SSR transform/codegen architecture, runtime
helper identities, option handling, fallthrough attributes, CSS variables,
elements, components, slots, control flow, model/show directives, Teleport,
Suspense, Transition, and TransitionGroup. All 15 unchanged upstream files pass
all 130 tests and snapshots. Package formats and declaration parity remain
incomplete; 140 paths across the full scope remain absent.

Five JavaScript `host.js` files are classified separately as allowed primitive
adapters, with hashes and import checks. They do not satisfy any of the 234
algorithm/declaration mappings. Two type-only sources remain without mapped
`.lil` files or strict declaration-only handling. See
`compatibility/source-parity.json` for per-file paths, classifications, hashes,
and failure state.

The first paired actual-project comparison uses one reactivity application and
identical Vite 8/Oxc settings. It currently fails the size target: VueLil is
5,509 Brotli-11 bytes and Vue is 4,657 bytes. This diagnostic does not count as
one of the four final application scenarios.

The reactivity diagnostic builds the exact same application source, importing
from `vue`, once with installed `vue@3.5.42` and once with only that specifier
resolved through the source-derived VueLil production integration. Both Vite
8.2.1/Rolldown 1.2.6/Oxc
0.147.0 production bundles execute to checksum
`6ca74c5976e4cbda32d2c0b8c6df969bc37c436d31f03e55696066649a7831a1`.
The candidate module graph contains no upstream Vue runtime module.

| Reactivity app bundle | Raw | gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| VueLil candidate | 17,502 B | 5,978 B | 5,509 B |
| Vue 3.5.42 upstream | 13,347 B | 5,024 B | 4,657 B |
| Candidate delta | +4,155 B | +954 B | +852 B |

This is a failed diagnostic, not final Vue size evidence. Completion requires
passing paired runtime-only client, runtime-compiler client, SSR, and production
SFC application scenarios. Published package `dist` formats and the older
reactivity library artifact report do not count toward that gate.

## Required implementation order

1. `@vue/shared` audited one-to-one source modules and ECMAScript host ABI
   (complete).
2. `@vue/reactivity`, including native Proxy and weak-identity semantics.
3. Scheduler, VNodes, components, renderer, built-ins, and hydration.
4. DOM operations, directives, transitions, and custom elements.
5. Compiler core and DOM compiler, then the full runtime compiler.
6. SSR compiler, server renderer, streams, and asynchronous rendering.
7. SFC compiler, source maps, script setup, CSS transforms, and preprocessors.
8. Vue 2 compatibility runtime, package formats, and declaration parity.
9. Complete unchanged tests, all required paired-project size comparisons,
   performance gate, and Pages.

JavaScript host files may provide ECMAScript, DOM, stream, and external-tool
primitives that LilScript cannot express. Vue algorithms may not be delegated
to upstream code, and every shipped adapter byte is included in measurements.
