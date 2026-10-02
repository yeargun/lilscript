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
cannot satisfy source mappings. Enumerated host adapters and package interfaces are audited separately and may
not import upstream Vue code. They do not satisfy algorithm mappings.

The pre-migration evidence in
[`compatibility/source-parity.json`](compatibility/source-parity.json) records
234 files through 232 deterministic mappings and two strict declaration-only
records. Its source hashes and `artifacts/compatibility-report.json` are historical
until requalified against the compiler-delivery migration. The source auditor
continues to reject stale algorithm claims; delivery alone does not refresh them.

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

`npm run build:compatibility` installs compiler-written ESM, CJS and browser
outputs using `config/compiler-profiles.json`. Public facades, function metadata,
build flags and host providers are source-owned under `src/package` and
`src/providers`; builds no longer rewrite emitted JavaScript or run a second
minifier. The previous builders are retained in `migration/old-history`.

TOML profiles choose the behavior: development/test flags, production flags,
compatibility APIs, the browser or Node host provider, output format and public
entry selection. Pristine-built-in and pure-property-read assumptions remain
disabled because getters, proxies and patched globals are observable. Production
implementation profiles retain effort 15; small forwarding entries use 13.
`LILSCRIPT_COMPILER` pins an executable, and `VUELIL_PROFILES` selects a comma
separated subset. `LILSCRIPT_BUILD_MODE=development` skips optional search while
checking integration; it does not change a profile's source defines and is not
release compression evidence.

The installer validates every artifact against the compiler manifest before
installation. Its local cache checks compiler, source, configuration, dependency
lock and output hashes; changing source during compilation fails the install.
`artifacts/compiler-delivery-report.json` records delivery provenance. Existing
compatibility, source-parity, performance and site reports require separate
requalification after this migration; a delivery receipt does not establish
their acceptance gates.

The final `npm run measure` gate requires one reusable
`packages/vuelil/production` graph built with `config/compiler-production.toml`.
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
