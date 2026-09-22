# Shared Node Library Qualification

001-Q1 extracts the existing Mdast-to-Hast qualification body into
[`qualify-node-library.mjs`](../../../finer/tools/qualify-node-library.mjs).
The current [Mdast client](../2026-09-19-mdast-to-hast-baseline/qualify.mjs) and
[Remark-Rehype client](../2026-09-19-remark-rehype-baseline/qualify.mjs) supply
data recipes and CLI handling. There is one current implementation, not a new
pipeline alongside portgate. Historical receipts and archived wrappers are
unchanged.

## Ownership

Portgate still owns isolated source builds, original prerequisites/tests and
the first codec measurement. The extracted owner retains the existing trusted
release-parent and binary checks, complete source/dependency snapshots, exact
fresh invocation and artifact checks, separate original package check, canonical
codec replay, input-stability checks and output archival. Every gate remains
mandatory; recipes cannot supply callbacks or disable validation. Both the
shared owner and calling recipe are pinned and archived in new receipts.

Recipe and CLI validation precede output creation; importing the module does
not run qualification, create output or change the process exit status. The
Mdast client's four parent/compiler/codec overrides and original defaults stay
available. The before-extraction implementation remains at
`/tmp/lilscript-node-baseline-owner-before-20260919/qualify.mjs`, SHA-256
`7944deecdccbc96525eedc5458eabe6c65871b3b1ded756a4883da70f26905f7`.

| Qualified source | SHA-256 |
| --- | --- |
| Shared owner | `a71dd3c68a8ed4a64a553b70767c127c3b2fb0723c3073e7392d599cc6a409c4` |
| Focused tests | `f907e3c632d06c1474549e088b29167100cb43d9e06b476d724e8806eacaddb9` |
| Mdast client | `d99adcd3ea27897fd63fd0cba2d0329a766b34d9fe26ce79490b8ee112a309ed` |
| Remark-Rehype client | `4ebf5a13ce99a8a4ee0f6c5e0348b5b1d7c9f029bafc99a69f93177771f47bb9` |

## Focused Qualification

The [receipt](run-2026-09-19T20-34-46.683Z/receipt.json), SHA-256
`8447e5e591ea3378295e58b900c48242ac7df52cf6032c6bc2a5ba8a669d4c18`,
records **37 distinct passing tests**: five new qualification-owner tests,
fourteen artifact-evidence tests and eighteen Node-evidence tests. There are no
failures, skips or cancellations. One Node 24.11.1 command ran under a 120-second,
16 MiB output, process-group bound and finished in 16.129 seconds without
timeout, signal or forced pipe closure. This timing is diagnostic, not a speed
claim. No Cargo or maintained-library build ran in this test cohort.

The five new tests cover side-effect-free import, ten invalid data recipes,
eight invalid CLI forms plus missing arguments, both real clients refusing an
existing output directory without changing its sentinel, and three distinct
parent refusals. The latter use existing, fully pinnable inputs: wrong receipt
hash, unqualified public integration and wrong compiler identity. Each checks
the actual rejection, empty command list, unexecuted synthetic producer,
preserved caller/owner bytes and stable evidence. They do not establish complete
failure archival for missing or unreadable inputs before initial pin capture;
that inherited limitation remains explicit.

Independent review re-enumerated all 89 current inputs, matched before/after
manifests and checked the actual 37 names against the test sources. Input digest
is `fd83cbbbd2f7c67ac8bd06682379ef7bc06fe7793e46fe129d2502cbfa28b358`.
The full one-shot qualification harness is retained under `receipt.harness.args`;
stdout, empty stderr and the exact Node binary are pinned. No new permanent test
coordinator was introduced.

## Original Library Boundary

One [Remark-Rehype source-built attempt](../2026-09-19-remark-rehype-baseline/README.md)
then exercised the shared owner end to end. Receipt SHA-256 is
`9398ebb22148544f342edc9e97a5abafb74e9a3e73c5bec72a6a37bdf64e4f1a`.
Both fresh original compiler invocations, the original type prerequisite,
all 21 frozen Node identities, separate package check and codec replay pass.
Independent review verifies all 96 outputs, 25 input pins, full source and
dependency closures, four exact loads and five codec rows. The preserved 19:48
release parent predates the later constructor-ownership source change.

The original bounds remain: build 300 seconds, Node 90, declared type prerequisite
60, codec/package 60 each, shared command budget 600, external supervisor 630.
No Mdast replay, Rust rebuild, installation, assertion change or retry was used.
Whole-workload qualification stays unverified for the documented broader
delivery gaps; closed callable shape is not conversion behavior. This validates
a shared tool and one declared library boundary, not competitive compression,
current semantic-backend coverage or migration milestone closure.

Remark-Breaks' older wrapper is historical-only. Its discovery predates the
current npm-runtime snapshot contract; no compatibility branch or weaker pin
policy was added to absorb it. Earlier Mdast failures and passing replay retain
their exact archived implementations, inputs and outputs.
