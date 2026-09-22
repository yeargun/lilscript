# Remark-Breaks Source-Built Baseline

One unchanged-source attempt passed on 2026-09-19: both original compiler
invocations, the original TypeScript prerequisite, all 23 original Node
identities, and the separate original package dry-run check. Source, tool and
dependency identities stayed stable. No retry, compiler rebuild, library edit,
network dependency installation, or altered assertion was used.

The [final receipt](source-built-release/receipt.json) is SHA-256
`b66224b5221eeed1c3e15c74f8b712f12ccb0c343b542a35269a2c40631fe846`.
Its `passed`, `originalTestBoundaryPassed`, `packageDryRunPassed`, and
`inputsStable` fields are true. Whole-workload `qualification` remains
`unverified` because broader delivery requirements remain open. This README is
post-run narration, outside the frozen receipt's output manifest.

The `qualify.mjs` in this evidence directory is **historical-only**, identifying
the implementation used for this receipt. Current matching Node-library recipes
use the [shared qualification owner](../2026-09-19-node-library-qualification/README.md).
This older discovery lacks its npm-runtime snapshot contract; it was not silently
migrated through compatibility branches or weaker input checks. A future fresh
qualification must first establish matching current discovery evidence.

## Executed Boundary

The unchanged `node scripts/build.mjs --compile` made exactly two fresh
`js-module` compiler invocations: original `lilscript.toml` to the open raw
artifact, and original `lilscript.closed.toml` to the closed artifact. The same
original script packaged ESM/CJS/UMD with esbuild and copied the declaration.
Compiler receipts have no previous output to inherit; all bytes are retained.

The [test report](source-built-release/tests/remark-breakslil/report.json)
records `npm run check:types` passing before the original four-file Node
selection. All 23 frozen identities passed: 20 test nodes, including parent
tests, and three suites. Four passive load records bind the three required
ESM/CJS/closed artifacts to actual executed bytes. Original official cases use
the compiled ESM through the unchanged unified ecosystem consumer pipeline;
no upstream remark-breaks implementation is substituted.

The separate original `npm run check:pack` also passed. Its
[stdout](source-built-release/original-check-pack.stdout) reports 14 files,
8,360 packed bytes and 27,178 unpacked bytes. This was a dry run, not a retained
tarball installation or an installed-consumer runtime test. Generated JavaScript
and declaration hashes were rechecked after it.

The original portgate record exits 1 and keeps its declared additional coverage
unverified. The wrapper validates the complete original test boundary separately;
it does not erase those outstanding requirements or reclassify the portgate row.

## Exact Sizes

Both the initial canonical measurement and independent replay agreed. Values
are file bytes; this is not a competitive win or a combined-stream optimum.

| Artifact | Raw | gzip9 | Brotli11 |
| --- | ---: | ---: | ---: |
| Packaged ESM | 2783 | 1243 | 1118 |
| Packaged CJS | 3793 | 1678 | 1525 |
| Closed | 2767 | 1364 | 1225 |
| Intermediate raw ESM | 2695 | 1178 | 1058 |
| UMD | 3900 | 1708 | 1550 |

All hashes and scores are in the receipt, [retained artifacts](source-built-release/artifacts/),
and [canonical codec stdout](source-built-release/canonical-codec.stdout), SHA-256
`7873c41003c289be92844c701a1cab5ed91b6140cecc8cc85dd5afed6159f319`.
Raw and UMD outputs were measured, not credited with runtime test execution.

## Provenance

The compiler is the preserved release at
`/tmp/lilscript-public-integration-release-baseline-20260919/lilscript`, SHA-256
`3d8e450590247c53f928a7bc2eceb1ac50b60dfb21464c6629a55e8a813ea00e`.
Its accepted parent is
`2026-09-19-artifact-service/run-2026-09-19T16-36-23.430Z`, copied with its input
manifests into this receipt. Parent receipt SHA-256 is
`020c288f5b0e6bafc016e12cbb6d5a7a7ab53b9b28a32444c584c9781abbf533`;
accepted compiler-source identity is
`2e140666dd1cb5fd2f0cf82d0256cc0ed15e630a1defaaf4eca549c887e1e2f3`.
The companion codec is
`d55c6f33119cb11153f21145f45058ef3ed188c04554cc198af242f4b12e1ef9`.
These are historical preserved release inputs, not current Rust-source or
semantic-backend qualification.

The original source-only snapshot contains 37 files, identity
`9e19ad2c5209619734d38422e9e788ebd6550da946d2b12acb29076c6affa091`.
Full before/after captures include 1,034 project dependency files in 1,183 tree
entries and 2,106 global npm runtime files in 2,643 entries, including nested
dependencies and symlink destinations. Empty user/global npm configurations,
an isolated cache, pinned Node 24.11.1, and offline configuration were recorded.
The original workspace configuration was preserved. No hermetic OS claim follows.

[Supervision](supervisor.json) completed in 18.000 seconds without timeout.
The observed build phase took 13.039 seconds; these single-run diagnostics are
not speed or regression claims. Limits were 180 seconds for build, 90 for the
Node suite, 60 for the explicitly pinned type prerequisite, codec and package
checks, plus shared 430-second and outer 450-second bounds. The wrapper identity
is `3eb848d7a0abfb2ec1bcb350bc48a1f1d5a3c787a1fd7a7b20ba486de01de742`.

## Remaining Scope

CJS assertions cover default-only export/function shape; behavior and arity
assertions use ESM, so this is not full CJS behavior coverage. The closed lane
has its original mutation case, not an expanded whole-suite equivalence claim.
Original `check:site`/`build:site`, UMD/browser execution and a freshly installed
package consumer remain unverified. The existing Node site test only reads
static files and markup. No full library-fleet, new compiler heuristic, current
semantic support, or relaxed observation/optimization policy is claimed.

After independent validation of this receipt, portgate received a one-line
explicit-null prerequisite schema hardening, covered by the separate
[three-check tool receipt](../2026-09-19-remark-breaks-adapter/phase-null-tests.json).
That later tool version is not the runner used here. The original non-null
inventory, frozen runner copy (`14b26407...`), receipt and output hashes are
unchanged; no library/compiler retry was performed.
