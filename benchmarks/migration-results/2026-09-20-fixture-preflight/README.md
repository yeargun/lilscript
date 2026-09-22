# Fixture Preflight Evidence

001-Q2 strengthens the existing Node evidence owner before running a suite that
can regenerate its expected fixtures. `runNodeTestEvidence` now validates pinned
fixture declarations and records identities before prerequisites, after
prerequisites and after tests. An initially missing/changed fixture prevents both
prerequisites and tests; a prerequisite changing a fixture prevents the suite.
Final checks still reject mutations made during execution. Original assertion
content is unchanged. This is not an OS read-only sandbox or protection against
a hostile racing writer.

The existing complete dependency-tree capture moves from
`qualify-node-library.mjs` into `artifact-evidence.mjs`; the qualifier imports it
instead of keeping another implementation. It preserves nested dependencies,
symlink targets, executable bits, deterministic order and the 512 MiB/50,000-entry
defaults. Entry refusal now happens before visiting an over-limit entry. Three
new tests cover nested/symlink content, exact bounds and invalid bounds, cycles
and missing destinations. [Equivalence evidence](dependency-equivalence.json)
matches both complete dependency snapshots from the prior accepted Mdast run.

Four new Node checks cover refusal before self-updating tests/prerequisites,
prerequisite mutation, post-suite mutation and malformed/duplicate declarations.
The prior imported-fixture regression now requires no execution, not merely
rejection after execution. No new test runner, compiler-specific policy or
library assertion replacement is introduced.

## Results

The [initial receipt](receipt.json), SHA-256
`a57bcc33632832b0e1a8d05de96ecb090a287c6d4d7ba874932f6b16db2d6ef9`,
passes 43/44 checks. The new cycle test fails in its cleanup because `rmSync`
rejects that directory symlink. Its [initial source](initial-artifact-evidence.test.mjs)
and failure logs remain preserved. Changing only cleanup to `unlinkSync` makes
all 17 artifact checks pass in the [affected rerun](corrected-fixture/receipt.json),
SHA-256 `f8c80ce4a2c3b33a17c59aa0e0856a954051d97bcc8bdb6938242d5a7dc02a45`.
The 22 Node and five qualifier tests reuse their source-identical passing results;
44 distinct checks are accepted. No Rust build or full-fleet test occurred.

The [root read-only audit](audit.json), SHA-256
`dccc904ea7838a7cbedfd7aa2178d954aae9dad1758222aa9562fab3077246ea`,
checks affected input identities, the changed-test-only rerun, before-edit files,
dependency snapshot equivalence and the successful original GFM consumer.
No independent agent review is claimed. Before-edit files are preserved at
`/tmp/lilscript-fixture-preflight-before-20260920/`.

All 979 compiler inputs still match the qualified release digest
`269288a54694ef126db4e16bba350254c0a7cc2629a6c6641cd5cf672511295a`.
Historical qualification receipts and their archived owners remain immutable;
they do not qualify the new evidence-owner source by implication. No new
source-built library or compression/speed claim follows from these tool tests.
