# Remark-GFM Original Inventory

001-GF freezes the unchanged original type prerequisite and **22 Node identities**:
19 test nodes, including two parent tests, and three suites. All pass in one
attempt against retained distribution files. This is existing-dist discovery,
not a source-built or full-library qualification.

- [Discovery receipt](existing-dist/receipt.json):
  `b584655ac815c09b7a7c6353d06757cc24cceef4f9a421050664662ef1ad32b0`.
- [Required inventory](../../libraries/remark-gfmlil.required-tests.json):
  `42d537f47dfb744d02d67a2b3957d8ca8a02ec3caa0da3fecdac9f216a016c4d`.
- [Outer supervisor](supervisor.json):
  `bc7389df7701c82c6f111c6d8605dcd1e3a0ab5ec0b42dda356493d65485bd17`.
- [Root audit](audit.json):
  `f6514d8cc371eaa8d93bc2995041497560e3ab8c89d87ad0ee4afd6a7a9c06d7`.

The original `npm run check:types` runs unchanged before the four entry files
selected by `test/*.test.mjs` and `test/official/index.js`. An isolated copy is
byte-identical to all 105 original non-dependency files, including distribution;
its dependency symlink uses the original pinned installed tree. No install,
LilScript invocation, codec, source/configuration/assertion change or test retry
occurs. Outer supervision exits 0 in 4.039 seconds under its original 90-second
bound; type checking retains its 60-second ceiling.

All eight original `tree.json` expectations are parsed and pinned before
execution, with `UPDATE` absent. The suite's self-update path is not used.
The existing Node owner checks all 46 fixture pins before prerequisites, before
tests and after tests. Both isolated and original trees remain unchanged. The
missing `table-no-align/output.md` retains the original serialization expectation
from `input.md`; no expected output is generated from the candidate.

Four passive load records prove execution of these three files:

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| ESM | 33051 | `a8d896ef1f50bcbb93d9410b633d11f2bf445b3035dfe8f33816db333891f724` |
| CJS | 37077 | `0a2ccca3c4a5b34ae1ab42c84bdb4122c4ee09df14d5316ad822c8f31450f5dd` |
| Closed | 34282 | `78334b0c993372d30b7b129e50baae3fbe34e0d55f1a66432f21fd67969aaa5c` |

ESM exercises live processor registration, callback `this`, option forwarding,
escaped-atext handling and eight parser/serializer fixtures through upstream
Remark with the candidate GFM plugin. CJS assertions cover export/callable shape;
closed assertions invoke registration but not full parsing/serialization. Site
tests inspect retained files and markup, not a browser or rebuilt site.
[Caller observations](public-features.json) strengthen 002's explicit host-boundary
evidence without deciding D2 or proving arbitrary-input behavior.

Root's read-only audit verifies all 21 outputs, 11 input pins, 99 source files,
105 original/copied files, 1,071 project dependency entries and 2,643 npm entries.
The [existing inventory consumer](inventory-consumer.json) accepts all 22 frozen
IDs and retains unverified coverage gaps without rerunning tests. Its manifest
is a single-row test projection, not another active workload manifest.
An initial audit caught a manifest patch on Mdast-from-Markdown instead of GFM;
the [incorrect intermediate manifest](manifest-before-audit-correction.json) is
preserved. The corrected comparison proves only GFM's canonical row changed.
No independent agent review is claimed.

## Remaining Gaps

Fresh source qualification is not attempted. The original open configuration
contains `terminal_cleanup_chain` and `wide_single_use_collapse`, absent from the
current deny-unknown-fields schema. Neither field was dropped or translated.
The [shared prerequisite follow-up](../2026-09-20-original-prerequisite/README.md)
now accepts the exact original `check:types` declaration through its pinned
discovery evidence, with eleven focused checks and no script renaming or second
qualification implementation. Read-only Git history traces both configuration
fields to `migration/target-tree`; their introducing commits are not ancestors
of current HEAD. They are branch-specific compatibility gaps, not demonstrated
aliases for current tactics. No source-built attempt or flag translation follows.

Original package/site build checks, installed-package consumers, full CJS/closed
behavior, UMD/browser execution, competitor baselines and cost gates remain open.
The historical package version (4.0.3) versus build-banner version (4.0.2)
discrepancy is not repaired or used to substitute a different boundary.
