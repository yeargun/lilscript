# SolidLil LSX parity

Complete client parity requires lowering and integrated differential runtime evidence for every in-scope client-rendering feature family. Hydration and SSR are separately inventoried and explicitly excluded because they require a coordinated server runtime.

Differential gate: `npm run test:lilx` compares `tests/lil/lsx-runtime.lilx` with `tests/solid/lsx-runtime.jsx` through `tests/lsx-runtime.test.mjs`.

| Feature family | Lowering | Runtime | Boundary |
| --- | --- | --- | --- |
| Host elements and nesting | verified | verified | Static host elements and nested elements compile from .lilx and mount through the LilScript DOM runtime. |
| Static attributes | verified | verified | Quoted and boolean host attributes compile to DOM attribute calls and are asserted after mount. |
| Dynamic attributes | verified | verified | Each dynamic host property receives an independent render effect so unrelated dependencies do not rerun. |
| Value and checked properties | verified | verified | String and boolean property updates have dedicated host calls. |
| Reactive text expressions | verified | verified | Expression children lower to owned text insertion. |
| Delegated events | verified | verified | Common bubbling events lower through the delegated host path. |
| Native event listeners | verified | verified | Non-delegated scroll event lowering compiles and updates reactive state in the integrated fixture. |
| classList updates | verified | verified | Each object-shaped class flag receives an independent render effect and cleanup scope. |
| Show control flow | verified | verified | Arbitrary truthy values, keyed raw-value callbacks, non-keyed live accessors, DOM identity, component/nested branches, fallback replacement, stale-accessor guards, ownership, and immediate cleanup are integrated. |
| For control flow | verified | verified | Full LilScript callback types, host/component rows, live index accessors, component fallbacks, prefix/suffix duplicate identity, insertion, reordering, removal, fallback ownership, and row cleanup are integrated. |
| Index control flow | verified | verified | Typed signal rows, positional identity, value updates, removal, and host fallback lowering are integrated. |
| Switch and Match | verified | verified | Ordered short-circuit selection, arbitrary truthy values, keyed raw callbacks, non-keyed accessors, fallback, component/nested branches, stale-accessor guards, ownership, and cleanup are differential-tested. |
| User components | verified | verified | User components have ordered live props, children, component spreads, nested control-flow composition, fragment normalization, keyed/indexed row use, and owned cleanup evidence. |
| Spread props | verified | verified | Ordered component and host spreads support later overrides, live getters, properties, attributes, classList, style, refs, listener replacement, and unmount listener cleanup. |
| Refs | verified | verified | Callback refs receive the created host node once during untracked construction. |
| use: directives | verified | verified | use: directives lower through the owned untracked runtime helper with an explicit value. |
| Dynamic elements/components | verified | verified | Dynamic lowers live string, component, and null selections with live props/children, SVG namespace selection, immediate branch cleanup, and identity pruning. |
| Portal | verified | verified | Ordinary, document-head, SVG, and shadow-root portals lower with refs, reactive children, delegated event hosting, idempotent teardown, and handle release. |
| SVG and MathML namespaces | verified | verified | Nested SVG and MathML nodes lower to namespace-aware constructors and are asserted by namespace URI. |
| Namespaced attributes | verified | verified | Static and reactive xlink/xml names resolve to setAttributeNS and are asserted through namespace-aware DOM reads. |
| Hydration | excluded | excluded | Out of scope for the client-rendering parity contract; true hydration requires a coordinated server marker, event replay, resource handoff, and mismatch-recovery subsystem. |
| SSR | excluded | excluded | Out of scope for the client-rendering parity contract; no server template or streaming claim is made. |
| Suspense and error boundaries | verified | verified | Typed ErrorBoundary fallbacks and reset callbacks, construction and delayed reactive errors, two-resource Suspense reveal, preserved pending content ownership, fallback/content cleanup, pending unmount, slot release, and minified-bundle behavior are differential-tested. |

Strict client gate: **pass**. Excluded server-coupled families: **2**.
