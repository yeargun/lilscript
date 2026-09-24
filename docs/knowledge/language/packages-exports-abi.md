# Packages, exports, and ABI boundaries

Parent: [language](README.md). Delivery contract:
[modules and delivery](../../modules-and-delivery.md). Source anchors:
`src/module.rs` (discovery), `src/package.rs` (lock and resolution),
`src/check/modules.rs` (module-graph checking) and the export tables of the
Program IR in `src/program/`.

Static relative `.lil` imports form one acyclic typed compilation unit. One
module-graph checker resolves every import and export, and the unit is compiled
as one program; source-module syntax is not a runtime wrapper. Initialization
runs once in dependency-first order.

Compilation world and public roots determine what an export means; artifact
format determines how it is delivered. The current compiler still couples some
of these choices; plan task M3.1 separates `execution`, `world` and `format` as
independent contract axes ([future architecture §14](../../future-architecture.md#14-configuration-and-interfaces)).

| World/boundary | Export behavior |
|---|---|
| executable/closed app | accessibility declaration; an unused export is not a retention root |
| reusable-library public root | runtime root exports are retained and mapped to the declared public ABI |
| internal split/preserve artifact | the optimized program is partitioned by the delivery plan; compiler-owned linkage is not automatically public ABI |
| foreign ESM | `import extern` supplies runtime identity and a matching `extern` supplies the type contract |

Type-only struct/class exports create no JS binding. That is a language contract
([`docs/language-v0.1.md`](../../language-v0.1.md) § Modules).
`export constructor C [as PublicC];` explicitly publishes the constructor value
and marks only that class identity-observed; see
[compressor surface](compressor-surface.md). **Until plan M4.1** the compiler
refuses it; the deleted route's implementation is in
[history](../history/compilation/class-identity.md). A library build keeps its
export names, and an application build has none to keep (`mangle.exports` is
retired). Instance layout, property renaming and function spelling are separate
dimensions; “exports stable” does not mean all internal owned property names
remain long.

The deleted route derived an ABI manifest from its IR; it went with that route
(plan M1.4). Contract A5 makes the delivered bytes the authority instead: names,
helpers and packaging are final before scoring, and an independent parse checks
every delivered file (plan M2.5). Raw, gzip and Brotli may choose different
internals only after the boundary is fixed.

Bare imports require `[dependencies]` plus a verified `lilscript.lock`. Packages are
currently local paths with name/version/compiler-ABI/entry metadata. The lock pins
the transitive graph and source checksum; stale contents, symlinks/path escape,
undeclared transitive visibility, version/ABI mismatch, and conflicting resolution
are hard errors. `--write-lock` no longer writes package effect summaries: only the
deleted route read them. Cross-package purity comes from the effect fact (plan M6.2).

Foreign `.js`/`.ts`/JSX edges are JS-only and remain ESM for Lilpack/Vite, unless
`bundle.host_modules` carries them with the output (then the compiler parses them
to deliver them). LilScript does not infer their types. Native rejects the edge.
The imported name and source specifier are ABI, but the local `as` binding is a
compiler-owned lexical name. JavaScript emission allocates a hygienic local spelling
and maps its matching extern function/global to that same spelling. This applies even
with identifier mangling disabled, so an alias such as `host as Array` cannot capture
an emitted `Array.isArray` or another target-generated runtime root.
