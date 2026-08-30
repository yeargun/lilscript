# Vue 3.5.42 port status

## Current state

The upstream source is pinned and cloned. Full Vue compatibility is not yet
implemented, and no final size or performance win is claimed.

| Gate | Current result | Status |
| --- | ---: | --- |
| Source files satisfied | 234 / 234 | Passed |
| Upstream test files | 196 / 196 | Passed |
| Declaration test files | 20 / 20 | Passed |
| Package entrypoints | 28 / 28 | Passed |
| Published format rows | 38 / 38 | Passed |
| Reactivity project diagnostic | Candidate 5,509 B vs Vue 4,657 B Brotli-11 | Failed |
| Required project-size scenarios | 0 / 4 | Pending |
| Runtime performance | Not measured | Pending |
| GitHub Pages evidence | Generated, incomplete | Pending |

The complete runtime export surfaces of `@vue/shared` (73 values) and
`@vue/reactivity` (50 values) are implemented. The candidate passes all 62
unchanged shared tests and all 440 unchanged reactivity tests. Their package
entrypoints, declarations, and inventoried formats now pass; neither package is
marked fully complete while the repository-wide size and performance gates fail.

Runtime-core exposes all 146 runtime exports, passes all 1,076 unchanged
upstream tests in all 44 files, and has all 65 source files mapped to verified
one-to-one modules. Compiler-core exposes all 145 upstream runtime bindings plus
13 intentional test-only source-module bindings, passes all 671 unchanged tests
in its 20 files, and has all 30 source files mapped to a declaration-free entry
barrel plus 29 verified owner modules.

The source-parity audit derives 234 tracked `packages/*/src` TypeScript files
directly from Git: 231 algorithm/runtime modules and 3 type-only modules. All 15
runtime/mixed `@vue/shared` files and all 13 `@vue/reactivity` files now have
conforming hash-pinned one-to-one mappings. `typeUtils.ts` has explicit
declaration-only handling because it emits no runtime code. All 30 compiler-core
sources now have conforming hash-pinned mappings: `index.ts` maps to a pure
import/re-export barrel and the other 29 files retain their upstream module
responsibilities. Across the full scope all 234 paths have hash-pinned handling,
with no absent deterministic paths and no legacy package-wide monoliths.

Runtime-dom exposes exactly 170 runtime exports and passes its complete unchanged
upstream suite: 253 tests passed, 1 skipped, across all 16 files. All 17
deterministic source paths are present and hash-pinned to verified one-to-one
mappings, including `jsx.ts` as a type-only mapping. Its published formats and
declaration surface pass the aggregate package audit.

The private runtime-test host has all 5 owner modules mapped and exactly 158
aggregate runtime exports. Its 3 local differential tests and all 5 tests in its
unchanged upstream file pass. This records the test host only and does not claim
another public package or published format complete.

Server-renderer exposes exactly 26 runtime exports from all 15 source owners,
shares the runtime-core singleton through runtime-dom, and passes all 308
unchanged upstream tests in all 17 files, including Node and web streams.

Compiler-dom has all 16 source files mapped, including the exact static-stringify
transform and its compile-time hoist wiring. All 13 unchanged upstream files
pass all 135 tests.

Compiler-ssr has all 17 source files mapped to independent owner modules and
preserves the upstream two-pass SSR transform/codegen architecture, runtime
helper identities, option handling, fallthrough attributes, CSS variables,
elements, components, slots, control flow, model/show directives, Teleport,
Suspense, Transition, and TransitionGroup. All 15 unchanged upstream files pass
all 130 tests and snapshots, and its package format and declaration gates pass.

The standalone VueLil `@vue/compat` integration builds standard and runtime-only
ESM entries with 172 exports each over compat-enabled VueLil runtime-dom and
compiler-dom singleton graphs. All 117 tests in all 11 unchanged upstream compat
files pass, and all 6 compat source owners are hash-pinned one-to-one mappings.
All seven inventoried package formats and its declaration entrypoint now pass.

Seven JavaScript `host.js` files are classified separately as allowed primitive
adapters, with hashes and import checks. They do not satisfy any of the 234
algorithm/declaration mappings. All three type-only sources now have strict
handling: two declaration-only records and one verified runtime-dom mapping.
Compiler-SFC has all 30 runtime owners plus strict declaration-only handling for
`shims.d.ts`; all 19
unchanged upstream files pass 470 tests with 1 upstream skip. See
`compatibility/source-parity.json` for per-file paths, classifications, hashes,
and completion state.

The standalone VueLil `vue` package now integrates all three source owners as a
171-export full runtime-compiler entry and a 171-export runtime-only entry. It
preserves runtime-dom and shared/reactivity singleton identities, passes all 144
tests in all 18 unchanged upstream `vue` files, and blocks every scoped upstream
Vue runtime/compiler implementation in the module test graph. The full and
runtime-only ESM-bundler, ESM-browser, and global builds, CJS build, declarations,
compiler-SFC, server-renderer, JSX, and package metadata entrypoints all pass.
Required project size and performance remain unclaimed.

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
8. Vue 2 compatibility runtime, package formats, and declaration parity
   (complete).
9. Complete unchanged tests, all required paired-project size comparisons,
   performance gate, and Pages.

JavaScript host files may provide ECMAScript, DOM, stream, and external-tool
primitives that LilScript cannot express. Vue algorithms may not be delegated
to upstream code, and every shipped adapter byte is included in measurements.
