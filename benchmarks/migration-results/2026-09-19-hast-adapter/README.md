# HAST Original-Suite Evidence

The [adapter receipt](receipt.json) records the frozen 460 Node test identities
(456 tests and four suite nodes), source/fixture hashes and 35 focused evidence
checks. Its existing-dist replay is diagnostic, not a source-built baseline.

## Source-Built Baseline

The subsequent [port record](source-built/hast-util-to-htmllil.json) and
[manifest](source-built/manifest.json) preserve a source-built run with the
qualified debug compiler from the
[artifact/service receipt](../2026-09-19-artifact-service/README.md).
This is the compiler's **legacy default backend**, not migrated whole-library
support. Both compiler invocations, original-suite records, load observations,
logs and all five measured delivery files are retained in `source-built/`.

```sh
RAYON_NUM_THREADS=2 node finer/tools/portgate.mjs record \
  --arm service-baseline-hast \
  --compiler /tmp/lilscript-artifact-service-baseline-20260919/lilscript \
  --codec /tmp/lilscript-artifact-service-baseline-20260919/lilscript-codec \
  --ports hast-util-to-htmllil \
  --out /tmp/lilscript-source-built-hast-20260919 --timeout 300
```

Node was the repository-pinned 24.11.1. The source-built run used an isolated
workspace and left the original library checkout unchanged. Build trust is
`ok`; all 460 required identities pass, and exact ESM/closed production hashes
were observed at load time. The gate exits 1 with `unverified` certification:
package/CJS, UMD/browser/site runtime and complete delivery/dependency coverage
are still open. The record preserves the prior adapter's conservative
additional-coverage messages verbatim; the source-built compiler provenance
portion now has evidence, but that does not discharge the remaining obligations.

| Delivered File | Raw | gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| ESM | 30,101 | 9,971 | 8,807 |
| Closed | 29,952 | 9,816 | 8,668 |
| CJS | 31,100 | 10,259 | 9,100 |
| UMD | 31,171 | 10,287 | 9,129 |
| Raw compiler output | 30,005 | 9,904 | 8,750 |

These are distinct delivery boundaries, not competing interchangeable outputs.
The original suite executes ESM/closed, not every measured file. The roughly
7.5-second debug build is a diagnostic observation, not a release-speed gate.

Port record SHA-256:
`0a3c34ff4c96f77ba1f8fb34dbaa3099431567cbc9837c2dc47224c3f5b7d675`.
Manifest SHA-256:
`e9e256873f6138e17b25641d44323495d5b3f66daf2fa0aa41b67c3a81c3ba99`.

## Original Packaging And Site Checks

The [additional receipt](source-built-additional/receipt.json), SHA-256
`753f8619613608a88532a7e230dbeb8fbaf8e8219f813d512162f3b480ecc8e0`,
records passing unchanged `npm run check:pack` and `npm run check:site` commands.
The original site suite's one test and one suite node pass without skips;
rebuilt `_site/to-html.js` matches the scored ESM byte-for-byte. All six dist
outputs and the original checkout remain unchanged. The dry-run package contains
26 files, 30,862 packed bytes and 182,865 unpacked bytes.

These original checks discharge their own command obligations, not executable
CJS, UMD/global, installed-package, declaration or browser coverage. The site
test inspects source/HTML markers; it does not run a browser. Certification stays
unverified, and the original port record is not retrospectively rewritten.

## Installed CJS Supplement

The [installed-package receipt](source-built-package-cjs/receipt.json), SHA-256
`d00cb8da1522021f1cce55a4749c903694f049bb962db746ef776071548a0c7f`,
records an offline local `npm pack` and installation into a fresh consumer with
lifecycle scripts disabled. The [wrapper](qualify-package-cjs.mjs) uses the
unchanged preserved source-built workspace; it neither recompiles the library
nor installs upstream or other runtime dependencies. All original, preserved
workspace, installed package and pinned evidence inputs remain unchanged.

The 26-file tarball is 30,862 bytes, SHA-256
`c9ab094bf62e21030fb84056aeca5fbc6bf5a4ff40e3d44e921bde56f51792bb`.
Package `require` resolves to the installed 31,100-byte `dist/to-html.cjs`.
The shared passive Node loader observer records its actual CommonJS source as
SHA-256 `6251b2367acc1a9875fe44fc3ff78cb622056ae1551dbdcfd4d560d4c4e659fb`,
matching the preserved scored artifact.

All eight [supplementary cases](source-built-package-cjs/smoke-results.json)
execute: **seven pass and one fails**. Package resolution, named exports, arity
two, text, escaping, invalid-argument behavior and array-input serialization
pass. The public-name check keeps its expected value `toHtml`, but observes
`kb`; the smoke command exits 1 and qualification remains `unverified`.
The array fixture expands the original test's `h('b')` and `h('i')` into plain
HAST element objects, without loading a fixture-building dependency.

This is bounded installed-CJS delivery evidence, not the complete CJS suite.
It neither replaces nor adds to the original 460 ESM/closed test identities.
Public-observation/profile questions, UMD/browser execution, declaration
consumers and complete delivery coverage remain open. No failed expectation,
production artifact or original test was changed to obtain this result.

## Public-Binding Gap

A [supplementary binding probe](public-boundary.json) ties the inspected files to
the preserved source-build record. Upstream `toHtml` has name `toHtml` and arity
2; legacy ESM reports name `kb`, and closed output reports `mb`, both with arity
2. The original function is constructible with an own `prototype`; these outputs
are not. Both source-built profiles explicitly request arrow spelling, so this
is a public-contract question as well as an implementation observation.

The passing original suite does not cover these differences. Treat them as
002/007/011 compatibility and profile gaps, not as an eligible size advantage.
The supplementary probe does not replace any original test or establish complete
upstream dependency provenance.
