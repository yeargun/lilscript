# VueLil laboratory

VueLil is a source-level LilScript rewrite of Vue, pinned to `vue@3.5.42`
(`v3.5.42`, commit `d63616ca17de965ed32dcb449a4c5cd9982f15d2`).
This laboratory does not treat an export stub, wrapper around Vue, or reduced
feature subset as a compatible implementation.

## Contract

The completion gate covers `vue`, the runtime and compiler packages,
`@vue/server-renderer`, `@vue/compiler-sfc`, and `@vue/compat`. It requires:

- one source file at `src/<package>/<relative>.lil` for every tracked upstream
  `packages/<package>/src/<relative>.ts` or `.tsx` file, with exact algorithm
  and module-responsibility verification pinned to both source hashes;
- all applicable unchanged upstream tests and declaration tests to pass;
- exact public exports, descriptors, function arities, constructor behavior,
  singleton identity, and package entrypoints;
- no runtime import of upstream Vue implementation code;
- lower Brotli-11 bytes in every required paired production application build;
- matching deterministic execution checksums and no upstream Vue runtime in
  each candidate bundle; and
- no material regression in browser, reactivity, compiler, or SSR workloads.

Size completion compares what applications bundle, not VueLil package files
against Vue's published bundles. Each scenario builds one unchanged source tree
that imports from `vue` twice with the same pinned Vite 8, Rolldown/Oxc,
production defines, and `es2022` target. Only `vue` module resolution changes to
the candidate under `packages/vuelil`. Raw, gzip-9, and Brotli-11 bytes come only
from the repository's canonical `lilscript-codec`.

The required final scenarios are runtime-only client, runtime-compiler client,
SSR, and a production SFC application. The reactivity-only app is a
diagnostic milestone only and cannot satisfy the final project-size gate.

Compatibility, transfer size, and performance are independent gates. See
[`compatibility/scope.json`](compatibility/scope.json) and
[`PORT_STATUS.md`](PORT_STATUS.md) for the current truthful state.

## Source parity

`npm run audit:source-parity` derives its complete input set from the pinned
Vue Git tree rather than a hand-maintained file list. It maps each file by the
single rule `packages/<package>/src/<relative>.ts(x)` to
`src/<package>/<relative>.lil`, records SHA-256 hashes for both sides, rejects
duplicate or many-to-one mappings, and rejects empty, TODO, stub, or upstream
JavaScript delegation candidates. A mapped algorithm counts only when
`verifiedMappings` hash-pins both files and records `moduleResponsibility` as
`preserved` and `algorithmParity` as `exact`.

Type-only files are identified from their parsed TypeScript module body and do
not disappear from the gate. They require a normal verified `.lil` mapping or
an explicit `declarationOnlyHandling` record containing the exact upstream
hash, `handling: "declaration-only"`, and a specific reason. JavaScript files
cannot satisfy source mappings. The seven enumerated `host.js` files are audited
separately as primitive host adapters and may not import upstream Vue code.

The current evidence in
[`compatibility/source-parity.json`](compatibility/source-parity.json) covers all
234 files through 232 deterministic mappings and two strict declaration-only
records. The separate package compatibility build preserves the pinned
declarations, verifies every inventoried entrypoint and format, and writes
hash-pinned evidence to `artifacts/compatibility-report.json`.

## Setup

```sh
npm install
npm run setup
```

`setup:upstream` checks out the immutable upstream revision under the ignored
`upstream/vue` directory and installs its pinned pnpm workspace. The upstream
checkout must remain clean. `setup:compiler` builds the LilScript compiler and
canonical codec scorer from the repository root.

## Gates

```sh
npm run audit:source-parity
npm run test:source-parity
npm run test:declarations
npm run test:package-parity
npm run check:foundation
npm run check
npm run build:project-comparison
npm run measure
npm run benchmark
npm run build:pages
```

The final `npm run measure` gate requires one reusable
`packages/vuelil/production` graph built with `config/open-world.toml`.
Runtime-only and SFC must resolve the same `vue.runtime.js`; Vite must perform
all scenario-level tree shaking. The gate rejects scenario-specific candidate
paths and records source, bundle, module-graph, toolchain, package-lock, codec,
and upstream provenance. The currently checked-in size report predates this
stricter reusable-package requirement and remains diagnostic.

`npm run check` is intentionally fail-closed until every package and format and
all 234 source mappings and four required project scenarios in the scope
manifest are complete. `audit:source-parity` also exits nonzero while mappings
are incomplete, after refreshing the truthful evidence file. A library
distribution comparison does not count, and no generated report may describe
a partial milestone as Vue compatibility.
