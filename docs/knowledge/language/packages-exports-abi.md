# Packages, exports, and ABI boundaries

Parent: [language](README.md). Delivery contract:
[modules and delivery](../../modules-and-delivery.md). Source anchors:
`src/module.rs` (discovery/linking), `src/package.rs` (lock/effects/resolution), and
export/foreign-import records in `src/ir.rs`.

Static relative `.lil` imports form one acyclic typed compilation unit. The linker
resolves and namespaces modules before SSA; source-module syntax is not a runtime
wrapper. Initialization runs once in dependency-first order.

Compilation world and public roots determine what an export means; artifact
format determines how it is delivered. The current compiler still couples some
of these choices, which is tracked in the planned migration.

| World/boundary | Export behavior |
|---|---|
| executable/closed app | accessibility declaration; an unused export is not a retention root |
| reusable-library public root | runtime root exports are retained and mapped to the declared public ABI |
| internal split/preserve artifact | optimized graph is partitioned after linking; compiler-owned linkage is not automatically public ABI |
| foreign ESM | `import extern` supplies runtime identity and a matching `extern` supplies the type contract |

Type-only struct/class exports create no JS binding. That is a language contract
([`docs/language-v0.1.md`](../../language-v0.1.md) § Modules), implemented as
`ExportBinding::TypeOnly`. `export constructor C [as PublicC];` explicitly
publishes the constructor value and marks only that class identity-observed; see
[compressor surface](compressor-surface.md) and
[class identity](../compilation/class-identity.md). `mangle.exports = true` is safe
only for a closed LilScript application whose importers are linked before codegen.
Reusable ESM/script-tag surfaces keep it false. Public aggregate ABI, instance layout,
property mangling, and function spelling are separate dimensions; “exports stable”
does not mean all internal owned property names remain long.

The normalized `JavaScriptAbiManifest` begins to make that distinction executable.
It records export spelling/kind, callable arity/constructibility, constructor
method signatures, aggregate ABI mode, and stable aggregate/extern field sets.
It is included in `--explain=json`. Owner-qualified ordered fields, emitted names,
live bindings/module identity, descriptor details, and expected-versus-observed
post-emission validation remain migration work. Raw/gzip/Brotli and optimization
priority may choose different internals only after the boundary is fixed.

Bare imports require `[dependencies]` plus a verified `lilscript.lock`. Packages are
currently local paths with name/version/compiler-ABI/entry metadata. The lock pins
the transitive graph and source checksum; stale contents, symlinks/path escape,
undeclared transitive visibility, version/ABI mismatch, and conflicting resolution
are hard errors. Package effect summaries let linking remain conservative without
discarding cross-package purity facts.

Foreign `.js`/`.ts`/JSX edges are JS-only and remain ESM for Lilpack/Vite. LilScript
does not parse the foreign language or infer its types. Native rejects the edge.
The imported name and source specifier are ABI, but the local `as` binding is a
compiler-owned lexical name. JavaScript emission allocates a hygienic local spelling
and maps its matching extern function/global to that same spelling. This applies even
with identifier mangling disabled, so an alias such as `host as Array` cannot capture
an emitted `Array.isArray` or another target-generated runtime root.
