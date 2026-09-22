# Micromark Original Boundary Discovery

One unchanged existing-dist attempt passes all **1,963 original Node identities**
across `test/official/index.js`, `test/public-api.test.mjs` and
`test/stream-api.test.mjs`. These include parent test nodes and utility tests;
they are not 1,963 independent public-library assertions. None fail or skip.
No compiler, package installation, assertion/configuration edit or codec ran.

- [Discovery receipt](existing-dist/receipt.json):
  `7644929e24cbfae5c25ca9ef4fb79bcdceb177130b3e1934e53595f3d74fccb5`.
- [Original case report](existing-dist/node/report.json):
  `3ca7f78d21c3ee046f64c016cd3927dadd37c059a75f53022727072e2494b658`.
- [Frozen inventory](../../libraries/micromarklil.required-tests.json) retains
  all observed identities and the original selection, without excluding cases.
- [Read-only audit](audit.json) checks 24 retained outputs, eight tool/runtime
  pins, 137 source files, 148 files including distribution, and all 872 installed
  dependency entries (754 file paths, 61,887,035 bytes). Before, after and current
  trees agree, including directory listings and symlink destinations.
- The existing [inventory consumer](inventory-check.json) accepts the frozen
  cases through the maintained-workload row, using a declared one-row subset.
  It preserves additional coverage gaps and does not certify the whole fleet.

The discovery intentionally starts without a frozen required-case inventory, so
its report remains `unverified`. It establishes an inventory and observations,
not a source-built compiler or complete package qualification. Root performed the
read-only audit; there was no independent agent review. The external supervisor
exits normally with no timeout or forced pipe closure under the original
90-second process-group bound. The attempt took 3.6 seconds, a shared-host
diagnostic rather than a performance claim. Original stream tests create and
remove four temporary workspace files; none remain and the full tree is unchanged.

Eleven passive records observe seven exact file identities:

| Artifact | Bytes | Coverage |
| --- | ---: | --- |
| `micromark.esm.js` | 84,608 | Original Markdown, extension and tokenizer behavior |
| `micromark.cjs` | 85,358 | Public root export names, not the full ESM behavior suite |
| `micromark.closed.js` | 94,565 | One heading-output assertion |
| `micromark.stream.js` | 86,317 | Original streaming and EventEmitter behavior |
| `micromark.stream.cjs` | 92,421 | Public export and EventEmitter prototype/method identity |
| `micromark.umd.js` | 85,447 | Import and one heading assertion in Node, not a browser |
| `micromark.test.js` | 32,328 | Test-only utility bundle, not public production delivery |

The stream's `node:events` dependency is part of the observed contract. Tests
require the actual EventEmitter prototype and methods, retained callbacks,
once-wrapper identity, listener removal and callback `this`. These cannot be
replaced by independent value copies. Original GFM hooks and slice-serialization
callers also execute. Their output assertions do not independently prove every
token/context identity or arbitrary proxy and aliasing behavior.

The [public-feature mapping](public-features.json) binds exact case identities to
these constraints, inherited tokenizer hooks, ordinary resolver errors and
numeric initial-point access order. The point test requires exactly one read of
each numeric field; it does not test arbitrary truthy/coercing values or successive
snapshot identities. The existing source-level `jsNum` compatibility question
therefore remains open. D2 is not settled by this receipt.

Static build inspection finds four original compiler invocations: open root,
closed root, stream and test support. The existing two-invocation typed-library
qualifier cannot be reused unchanged or satisfied by fabricated prerequisites.
The unchanged open config also contains `name_ordering = "idiom-converged"`,
absent from the current `deny_unknown_fields` schema. This source-inspected
compatibility gap is shared with jQuery and Probe, not an executed compiler
refusal or permission to translate the setting.
Original type checking, pack, fresh-site, installed-package and real-browser
coverage remain separate. Existing distribution has no qualified current-source
build provenance. There is no semantic-backend, competitive-size, release-speed,
or whole-library milestone claim. This README is post-run narration, not a
frozen input or another migration plan.
