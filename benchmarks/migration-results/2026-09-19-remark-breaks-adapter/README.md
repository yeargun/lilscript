# Remark-Breaks Adapter Evidence

This directory records preparation of the original maintained remark-breakslil
test adapter. It does not qualify a newly compiled library or compiler release.
The separate source-built attempt belongs in
[`../2026-09-19-remark-breaks-baseline`](../2026-09-19-remark-breaks-baseline/).

## Original Boundary

The unchanged original command after its build is:

```text
npm run check:types && node --test test/*.test.mjs test/official/index.js
```

The four Node entries are `test/api.test.mjs`, `test/closed.test.mjs`,
`test/site.test.mjs`, and `test/official/index.js`. Their complete observed
inventory contains 23 unique identities: 20 test nodes, including parent tests,
and three suites. No failing or skipped declaration is excluded by the adapter.

The common Node evidence owner now runs inventory-pinned prerequisites before
the suite, preserving the original `&&` behavior. A refusal, failure, timeout,
or changed prerequisite input leaves the Node suite unexecuted and required
cases missing. Prerequisites are recorded commands, not invented Node test IDs.
The adapter pins the npm executable, original package scripts/type test, source
and generated declarations, and TypeScript driver files. Each command has its
own cap; the enclosing baseline supervisor bounds their combined execution.

The original API tests resolve package self-import ESM and CJS entries. CJS
assertions check default-only export/function shape; behavior and arity checks
use ESM. The closed test imports the actual closed artifact. Official cases use
candidate ESM through the unchanged unified/remark-parse/remark-rehype/
rehype-stringify consumer pipeline, not an upstream remark-breaks substitute.
The Node site test checks existing static files and markup only.

## Existing-Dist Discovery

[Discovery report](discovery/report.json), SHA-256
`2911b71a0b810a63f4642fdb04a8df21d2814c80d2121e8142a664f37386cfd5`,
records the original type prerequisite passing and all 23 identities passing.
Four passive load records cover the three required ESM/CJS/closed artifacts.
The report remains unverified because a first discovery cannot certify its own
missing frozen inventory. The captured identities are now pinned in
[`remark-breakslil.required-tests.json`](../../libraries/remark-breakslil.required-tests.json).

The [90-second supervisor](discovery-supervisor.json) completed without timeout.
[Before](discovery-before.json) and [after](discovery-after.json) snapshots match:
43 original source/output files and 1,034 installed project dependency entries.
The source-only inventory separately has 37 files because it excludes `dist`.
The discovery pins npm's launcher but does not capture its complete global
runtime dependency tree. This limitation is explicit; source-built qualification
must capture that global tree separately. No compiler or build script ran here.

## Focused Tool Tests

These are separate accepted source snapshots, not one combined run at identical
inputs. Every recorded cohort passed with stable inputs during its execution.

| Receipt | Checks | Scope |
| --- | ---: | --- |
| [tool-tests.json](tool-tests.json) | 30 | 18 Node evidence owner tests, including seven prerequisite regressions, plus 12 read-only inventory tests |
| [artifact-tests.json](artifact-tests.json) | 14 | Existing invocation, artifact, test-evidence and canonical-report validation tests |
| [phase-tests.json](phase-tests.json) | 2 | Default/overridden phase limits reach their owners; six invalid bounds are rejected before arm creation |
| [phase-null-tests.json](phase-null-tests.json) | 3 | Later focused replay adds rejection of explicit null prerequisites; existing absent-prerequisite defaults still pass |

The 30-check receipt pins pre-phase-change `portgate.mjs` as `1b0ce7a4...`; the
two phase checks pin its later `14b26407...` version. Both pin the unchanged
prerequisite owner `de6d8ed3...`. Complete hashes, commands, logs and supervision
are in the linked receipts. The phase tests use explicit compiler/codec stubs
to test timeout ownership, not real compilation or compression correctness.

After the separate source-built baseline passed, a one-line handoff hardening
changed the current portgate identity to `4606716b...`: only an absent prerequisite
field defaults to an empty list; explicit `null` now reaches schema rejection.
The later three-check receipt covers this change. Frozen baseline runner copies
remain `14b26407...`; no original library or compiler attempt was repeated.

Original `check:pack` is a separate package dry-run requirement, not an installed
consumer test. Original `check:site`/`build:site`, UMD/browser execution, and full
CJS behavior remain distinct coverage requirements. None is silently credited
by these Node cases or tool tests.
