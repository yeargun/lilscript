# Remark-Rehype Source-Built Boundary

One unchanged source-built attempt passed the original type prerequisite, all 21
Node identities (18 test nodes, including the official parent, and three suites),
the separate original package dry-run, and canonical codec replay. There were no
failures or skipped identities. The source/config/assertions and frozen inventory
were unchanged; no compiler rebuild, dependency installation, or retry ran.

The [receipt](source-built-release/receipt.json), SHA-256
`9398ebb22148544f342edc9e97a5abafb74e9a3e73c5bec72a6a37bdf64e4f1a`,
records `passed`, `originalTestBoundaryPassed`, `packageDryRunPassed`, and
`inputsStable` as true. Broader `qualification` remains `unverified`.

The [external supervisor](supervisor.json) ended with status 0 and no timeout or
signal in 57.005 seconds. Its SHA-256 is
`5b2749a3a2c094f21538f9089519d47dbc81893c22463a00d1169bac6941b812`.
Its actual process-group deadline was 630 seconds with
a one-second kill grace. The shared command budget was 600 seconds; individual
ceilings were build 300, type prerequisite 60, Node suite 90, codec 60, and package
check 60 seconds. These are bounds, not additive performance measurements.

## Provenance

- The accepted release parent is
  [run-2026-09-19T19-48-31.223Z](../2026-09-19-artifact-service/run-2026-09-19T19-48-31.223Z/receipt.json),
  receipt SHA `8918faac11122e916d8c7decbdfe03d71f3b873c6dc990691a2709fe498e1a39`.
  Both parent input manifests are copied into this run.
- Preserved compiler SHA:
  `e0c4cb31f2bf306512767d296e8411f58652e52f12ad158a4d7733acc60712bd`;
  codec SHA: `2c9683d9b80746472f687dad7758075cdb9519032ecd3e5c48ec93e81bb50094`.
  This parent predates C2. These identify the qualified release, not subsequent
  worktree changes or semantic-backend library support.
- The thin [recipe](source-built-release/wrapper.mjs), SHA
  `4ebf5a13ce99a8a4ee0f6c5e0348b5b1d7c9f029bafc99a69f93177771f47bb9`,
  and shared [owner](source-built-release/qualification-owner.mjs), SHA
  `a71dd3c68a8ed4a64a553b70767c127c3b2fb0723c3073e7392d599cc6a409c4`,
  are both pinned and archived. Existing portgate, compiler-invocation, Node
  evidence, codec, and bounded-command owners perform the actual work.
- Independent postrun review rehashed all 96 retained outputs and 25 input pins.
  Source-only 35-file, full-original 41-file, and isolated workspace snapshots
  agree before/after/current. Installed project contents contain 1,338 entries /
  1,182 file paths / 58,633,333 bytes; global npm contains 2,643 entries / 2,106
  file paths / 11,734,603 bytes, including symlink destinations. These are pinned
  contents, not a hermetic operating-system claim.

The live [shared owner](../../../finer/tools/qualify-node-library.mjs) and
[common qualification evidence](../2026-09-19-node-library-qualification/README.md)
describe the extraction; this run pins the archived owner bytes above.

## Artifacts

Two recorded fresh compiler invocations produced raw and closed JavaScript from
the original `src/entry.lil` and respective configs. The original build script
produced the ESM, CJS, UMD and declaration deliveries. It preserves the original
11.1.3 banner even though package metadata is version 11.1.4.

| Artifact | Raw | gzip-9 | Brotli-11 | Original Runtime Evidence |
| --- | ---: | ---: | ---: | --- |
| ESM | 14,192 | 4,845 | 4,351 | Three exact module loads and behavior tests |
| Closed | 14,820 | 5,441 | 4,923 | One exact load; callable-export shape only |
| CJS | 15,627 | 5,512 | 4,970 | Not executed |
| Raw | 14,103 | 4,781 | 4,286 | Build intermediate, not directly executed |
| UMD | 15,734 | 5,548 | 4,999 | Not executed |

The five independently scored files match the [canonical codec replay](source-built-release/canonical-codec.stdout),
SHA `3e172346adf4f79396af661c2152ff013e3d397bba5797e83f364c14f3721ddd`.
The receipt retains every artifact hash; four passive load records bind the
actually imported ESM/closed bytes to those same archived and scored files.
No combined-stream score, competitive gain, or size improvement is claimed.

## Remaining Scope

The original `npm run test:types && node --test test/*.test.mjs
test/official/test.js` boundary ran unchanged. Candidate behavior uses generated
ESM directly and through unified mutate/bridge consumers. Upstream Mdast supplies
only two numeric footnote-helper oracle functions, not a replacement plugin.

Portgate returned 1 for its declared additional coverage, even though the
original type and Node commands passed. The later separate `npm run check:pack`
returned 0: 13 files, 27,311 bytes packed and 114,548 unpacked. That fulfills this
run's dry-run requirement, not installed-package runtime delivery; the earlier
portgate coverage list is not a later pack execution result.

CJS behavior, freshly packed/installed require/import delivery, UMD/browser
execution, fresh `check:site`, and closed-world conversion behavior remain open.
The original site test reads existing files and imports its configured ESM
callable; it does not rebuild the site or run a browser. Compilation observations
(50.417 and 0.318 seconds; total build 51.054 seconds) are shared-host diagnostics,
not a speed or regression claim. This README is postrun narration, not a frozen
run input.
