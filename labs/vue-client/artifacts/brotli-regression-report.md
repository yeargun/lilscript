# VueLil Brotli regression analysis

Date: 2026-08-30

Target: Vue 3.5.42 at `d63616ca17de965ed32dcb449a4c5cd9982f15d2`

## Conclusion

VueLil's size loss is not caused by readable local names or a failure to run
Brotli. Production locals are mangled to one or two characters, eligible static
bracket accesses are normalized, Oxc runs after Vite/Rolldown, and Brotli already
compresses VueLil's repetition more effectively than Vue's.

The primary problem is that substantially more JavaScript remains after tree
shaking and minification. Runtime-core accounts for most of the runtime-only
gap. Generic `JsValue` operations, host-call wrappers, side-effectful reflection,
top-level initialization, retained compatibility/development paths, and package
flattening all contribute.

The current report-pinned runtime-only and SFC comparisons also use
scenario-selected export entries. They are valid execution diagnostics, but do
not satisfy the newly explicit requirement that the measured package be one
reusable, non-closed-world build. The reusable package must preserve public
exports, mangle private identifiers and properties, and rely on the downstream
bundler for application tree shaking.

No LilScript compiler changes are proposed by this report.

## Source ownership

- 232 runtime or mixed TypeScript sources have explicit one-to-one `.lil`
  owners.
- 2 type-only TypeScript sources use hash-pinned declaration-only handling.
- 7 JavaScript host adapters expose DOM, stream, parser, or ECMAScript
  primitives only.
- No candidate package imports or wraps upstream Vue implementation code.
- Every retained adapter byte is counted in project bundles.

## Final project measurements

All JavaScript sizes below were reproduced from the report-pinned artifacts.
HTML and CSS assets are identical between each pair.

| Scenario | VueLil raw | Vue raw | VueLil Brotli-11 | Vue Brotli-11 | Brotli delta |
| --- | ---: | ---: | ---: | ---: | ---: |
| Runtime-only | 150,118 | 53,297 | 44,811 | 19,061 | +25,750 |
| Runtime compiler | 460,862 | 170,961 | 145,822 | 55,606 | +90,216 |
| SSR | 815,960 | 625,957 | 220,757 | 167,894 | +52,863 |
| Production SFC | 150,624 | 54,271 | 45,116 | 19,358 | +25,758 |

## Runtime-only attribution

| Module family | VueLil rendered | Vue rendered | Raw delta |
| --- | ---: | ---: | ---: |
| Reactivity | 36,082 | 31,080 | +5,002 |
| Runtime core | 139,699 | 79,749 | +59,950 |
| Runtime DOM | 23,484 | 12,620 | +10,864 |
| Shared | 11,853 | 7,595 | +4,258 |

VueLil's source corpus for these modules is less than half the raw size of
Vue's source corpus, but the retained rendered graph is 61% larger. This points
to emitted shape and tree-shaking granularity rather than source volume.

## Token and structure comparison

Runtime-only final bundle:

| Metric | VueLil | Vue | Ratio |
| --- | ---: | ---: | ---: |
| Total tokens | 68,170 | 29,937 | 2.28x |
| Identifiers | 21,018 | 10,337 | 2.03x |
| Operators and punctuation | 36,373 | 16,282 | 2.23x |
| String/template tokens | 4,554 | 1,029 | 4.43x |
| Numeric tokens | 2,077 | 702 | 2.96x |
| Functions | 919 | 443 | 2.07x |
| Object expressions | 216 | 78 | 2.77x |
| Calls | 3,142 | 1,353 | 2.32x |
| Top-level initialized variables | 286 | 171 | 1.67x |
| Top-level expressions | 42 | 3 | 14.0x |

Runtime-only contains 2,311 assignments versus 623 in Vue. The top-level
initializers and reflection calls make otherwise-unused definitions observable
to Rolldown, reducing safe dead-code elimination.

## Mangling audit

Mangling is active:

- all final local bindings are one or two characters;
- static string-bracket accesses have been converted to dot access;
- `config/open-world.toml` enables identifier and property mangling;
- public export names remain stable, as required for a reusable Vue package.

What remains unmangled is mostly observable or dynamically addressed:

- runtime-only has 647 unique long property names versus Vue's 378;
- generic host helpers receive keys as strings, preventing safe property
  renaming;
- public and plugin-facing `JsValue` bags require named properties;
- compatibility and warning text remains reachable through initialized module
  state.

Changing export names would make the package smaller but would violate the
non-closed-world API contract. It is not an acceptable fix.

## Compression behavior

| Variant | Shannon bits/byte | Brotli/raw |
| --- | ---: | ---: |
| Runtime-only VueLil | 5.415 | 29.85% |
| Runtime-only Vue | 5.247 | 35.76% |
| Runtime compiler VueLil | 5.617 | 31.64% |
| Runtime compiler Vue | 5.304 | 32.53% |

Brotli is already compensating for VueLil's repeated syntax. Runtime-only raw
size is 2.82x Vue while Brotli size is 2.35x. More dictionary tuning or string
pooling attacks bytes Brotli already handles and cannot close the structural
gap.

## Forced constructor-wrapper ablation

The seven simple constructor wrappers at the beginning of the runtime-only
candidate were force-inlined at scope-resolved call sites, removed, and then
processed with the same Terser configuration. Every variant produced execution
checksum `4980359c3a2017ff38abd4e44e6d2a9a2c75508fb3bc92b25f7d96e8c9e35707`.

The comparison baseline for this ablation is the unchanged candidate after the
same Terser pass: 137,121 raw, 49,441 gzip-9, and 40,654 Brotli-11 bytes.

| Forced inline | Calls | Raw delta | gzip-9 delta | Brotli-11 delta |
| --- | ---: | ---: | ---: | ---: |
| `{}` | 11 | -35 | +35 | +39 |
| `Object.create(null)` | 1 | 0 | 0 | +24 |
| `Array(length)` | 3 | -21 | -3 | -18 |
| `new WeakMap` | 6 | +13 | 0 | +12 |
| `new WeakSet` | 1 | 0 | 0 | 0 |
| `new Map` | 5 | -11 | -8 | -3 |
| `new Proxy(target, handlers)` | 2 | -25 | -13 | -16 |
| All seven | 29 | -65 | +132 | +104 |

Therefore Brotli already commonalizes these repeated constructor expressions.
Pooling all seven is 104 Brotli bytes better than force-inlining all seven. This
family is not a material cause of the 25,750-byte runtime-only gap. The more
important host costs are generic property reads/writes, dynamic method dispatch,
accessor/reflection construction, and the optimization barriers those operations
create.

## Structural ablations

Additional scope-resolved transformations were applied before the same Terser
pass. Every variant retained the runtime-only execution checksum.

| Variant | Brotli-11 | Delta from Terser baseline |
| --- | ---: | ---: |
| Unchanged VueLil | 40,654 | 0 |
| Inline 250 generic `hostRead` calls | 40,587 | -67 |
| Remove 52 public name/length reflection mutations | 39,624 | -1,030 |
| Fold production development globals | 40,588 | -66 |
| Reflection removal plus generic read inlining | 39,534 | -1,120 |
| Reflection removal plus production globals | 39,510 | -1,144 |
| Vue after identical Terser | 18,766 | -21,888 versus VueLil baseline |

The strongest combination leaves a 20,744-byte gap. This rules out constructor
pooling, simple generic reads, public reflection, or obvious development checks
as the dominant explanation. They should still be improved, but the main issue
is the amount and shape of code considered reachable after package flattening:
dynamic operation expansion, renderer trampolines, initialized compatibility
state, and owner boundaries that downstream tree shaking cannot remove.

## Ranked root causes

### 1. Runtime-core retention and side-effectful generated shape

Runtime-core contributes +59,950 rendered bytes in runtime-only and much more
in compiler and SSR scenarios. It contains more top-level initialized state,
function wrappers, dynamic operations, and reflection calls than Vue's module.

### 2. Single-file package flattening

The production configuration currently uses `bundle.mode = "single"`.
Flattening preserves exact cycles but removes module-level tree-shaking
boundaries. A reusable package should be tested with preserved modules so Vite
can remove complete unused owners.

### 3. Reflection roots

Runtime-only retains 67 `Object.defineProperty` calls versus one in Vue. Their
spans occupy about 5.4 KB raw and, more importantly, make function bindings
observable. Reflection should be attached only to retained exports or moved to
tree-shakable per-export wrappers.

### 4. Generic `JsValue` and host operation ABI

The runtime-only output retains hundreds of generic read/write/call operations.
These add wrappers, argument handling, string property names, and calls that
downstream tools cannot reason through.

### 5. Runtime-DOM adapter and dynamic access

Runtime-DOM contributes +10,864 rendered bytes. Its host adapter is 11,101 raw
bytes and ordinary static DOM operations still route through generic helpers.
Typed host interfaces can emit direct property and method operations while
keeping dynamic helpers only for computed keys.

### 6. Retained compatibility and development paths

Runtime-only retains at least 3.1 KB of compatibility/development literal
payload before counting branch code. False production flags need to eliminate
the entire import path and its initializers, not only the final branch.

### 7. Compiler-only dependencies

The runtime-compiler candidate uniquely retains roughly 101 KB rendered from
`entities` and `source-map-js`. Browser compiler production needs a narrower
dependency boundary and must not include SFC/test source-map paths.

### 8. Duplicated shared tables

Large HTML, SVG, MathML, and attribute tables occur in both shared and runtime
graphs. This is around 6 KB obvious raw duplication. Brotli handles much of it,
so it is lower priority than retention and dynamic calls.

## Non-closed-world build requirement

The final package lane must:

1. compile one reusable production package per entrypoint;
2. preserve all public export names and observable descriptors;
3. mangle private identifiers and safely private properties;
4. avoid `VUELIL_PROJECT_EXPORTS` or scenario-specific source deletion;
5. publish the same artifacts consumed by the project comparisons;
6. let Vite/Rolldown tree-shake the reusable ESM package;
7. preserve modules rather than flattening the package graph when that improves
   downstream elimination;
8. keep test-only exports and development constants out of production;
9. prove no upstream Vue implementation appears in the candidate graph.

The current report-pinned scenarios satisfy execution and independence checks,
but runtime-only and SFC use export-selected entries. They remain diagnostics
until rerun against the reusable package lane.

## No-compiler remediation plan

### P0: Build and packaging

- Use a module-preserving production configuration for reusable packages.
- Point package exports and comparisons at the same production artifacts.
- Separate test/development graphs from production graphs, especially
  compiler-SFC and server-renderer.
- Remove namespace-wide reflection loops and descriptor writes from package
  wrappers.
- Assert production artifacts contain no test markers, diagnostics, or compat
  owners when their flags are false.

### P0: Runtime source idioms

- Represent refs, computed refs, dependency links, and deps with typed classes
  or typed internal records rather than per-instance dynamic bags.
- Replace finite string-dispatched collection operations with fixed typed
  helpers.
- Replace renderer rest-argument trampolines with fixed-signature callable
  fields.
- Use typed DOM host interfaces for static Node, Element, CSSStyleDeclaration,
  and Event operations.

### P1: Internal constants and contexts

- Keep exact public enum objects, but use typed constants internally for shape,
  patch, scheduler, and node-type checks.
- Give tokenizer callbacks and compiler transform/codegen contexts typed
  internal views while preserving public plugin-facing object shape.
- Construct app and helper objects in one expression where descriptor order and
  behavior remain exact.

### P2: Cleanup and measurement

- Deduplicate shared lookup tables across package graphs.
- Remove redundant host aliases where canonical names are safe.
- Test `stable_local_names = false`; do not mangle public exports.
- Add small-import graph tests that fail if rendered runtime bytes regress.
- Rerun all four project scenarios only after the reusable package build passes
  the full compatibility suite.

## Reproduction

Canonical report and module accounting:

```sh
cd labs/vue-client
source "$HOME/.nvm/nvm.sh"
nvm use 24.11.1
npm run build:project-comparison
npm run measure
```

Canonical Brotli scoring:

```sh
target/release/lilscript-codec --json path/to/assets/app.js
```

Current machine-readable evidence:

- `artifacts/project-size-report.json`
- `artifacts/performance-report.json`
- `artifacts/compatibility-report.json`
- `compatibility/source-parity.json`
