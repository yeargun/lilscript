# Original Mdast-to-Hast Boundary

This is existing-distribution inventory discovery, not a new source-built compiler
qualification. No compiler or codec ran, and no library assertions were changed.

- [Discovery receipt](existing-dist/receipt.json): SHA-256
  `b49158603b840d7c97dbebc2d8d338310dcf5189494491e67f740ffbc11925c0`.
- [Original Node report](existing-dist/node/report.json): SHA-256
  `7910ac5781dbc4f63574563562b7e84f8afb4f3b2b692cc6a3a5d7f5e75969d9`.
- [Outer supervisor](discovery-supervisor.json): one attempt, exit 0, no timeout;
  90-second total bound. The inner discovery reported 4.156 seconds; this is not
  a performance comparison.
- [Frozen inventory](../../libraries/mdast-util-to-hastlil.required-tests.json):
  all **152 distinct identities**, comprising 149 test nodes (including parent
  tests) and three suite nodes. All passed; none failed or skipped. All three
  original entry files and 27 imported official modules are included.

The original command is `npm run test:types && node --test test/*.test.mjs
test/official/index.js`. Its unchanged type prerequisite passed first, with a
60-second cap. TypeScript 5.5.4's actual `lib/tsc.js`, both type-test files,
`tsconfig.json`, source/generated declarations, package scripts and npm executable
are pinned. The empty required inventory during discovery intentionally leaves
the Node report unverified; the resulting IDs are now frozen, not retroactively
presented as an independently qualified source build.

Four passive load records cover these exact existing production files:

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `dist/to-hast.esm.js` | 14023 | `8d29b1f9390d4676511e7df19ac2152ecd175e896f0fd26c86eabed51c18f98f` |
| `dist/to-hast.closed.js` | 14767 | `f345881ccb4facd1e6422184801420bb5024755e31a1b563964f55706198a8a5` |

[Before](existing-dist/before.json) and [after](existing-dist/after.json) match:
61 source-only files, 67 files including the existing distribution, the complete
installed project tree (1507 entries, 1321 file paths, 58,950,842 bytes), and the
global npm implementation with nested dependencies (2643 entries, 2106 files,
11,734,603 bytes). Tree totals include symlink destinations; they are identities,
not unique physical disk usage. Npm ran offline with isolated empty user/global
configuration and cache; inherited npm and Node injection settings were cleared.

The upstream `mdast-util-to-hast@13.2.1` dependency supplies only the original two
numeric footnote-helper oracle functions. All candidate conversion calls use the
generated ESM/closed artifacts. The original runtime selection does not exercise
CJS, UMD/browser or newly installed package resolution. Its site test reads
existing files and loads the configured ESM; `check:site`/`build:site` and the
separate package dry-run were not run here.

The first authorized [source-build attempt](../2026-09-19-mdast-to-hast-baseline/README.md)
failed the original runtime assertions with different generated artifacts. A
later separately qualified compiler, after retiring assignment sinking, passed
the same 152-identity boundary in a new authorized replay. Both attempts have
their own immutable evidence; this passing existing-dist discovery is not the
source-build credit for either. The wrapper reused existing owners and pinned
accepted release compilers, not current semantic-backend library support. This
README is post-discovery narration outside the frozen discovery receipt.
