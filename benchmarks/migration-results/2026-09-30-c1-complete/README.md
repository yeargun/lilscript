# C1: configuration and permission completion

Qualified source `47c8d4f2886c1c266a1767876defc8266029da75`.
[identity.json](identity.json) pins the immutable compiler and codec.

The compiler resolves all optimization permissions through one declarative
family registry, which also generates the tactic reference. It lists the
implemented producers, shared and stage-specific prerequisites, analysis and
invalidation owners, supported runtime risks, purpose and tradeoffs. The audit
adds the previously omitted structural/output producers for folding, inlining
and pooling. Shared rules and structural discovery consume stage eligibility;
output formation and retained-artifact admission continue enforcing the same
family vetoes. An explicit `on` cannot invent a producer or an undeclared risk.
Unavailable helper-sharing/recurring-reconstruction names remain diagnosed
compatibility entries. Their future implementations belong to their own steps.

Explicit `policy.version = 3` requires reconstruction permissions independently
of effort. Omitted versions and version 2 preserve the existing diagnosed
level-16 startup grants. Default effort stays 13; naming strategies and their
14+ deferred tail are unchanged. All renamed-key conflicts now fail, including
objective/effort and debug aliases; equal scalar/singleton objective aliases
remain equivalent. File discovery, explicit files, CLI overrides, deprecated
environment adapters and intersected hard resource ceilings have executable
public-entry coverage.

Policy explanations distinguish default/preset/alias/canonical origins,
disabled/unavailable/automatic states, effective producers and admitted risks.
Diagnostic origins stay outside canonical identity. Fixed-size structural
summaries distinguish unvisited discovery from no discovered opportunities,
and unreached/legal/unknown/truncated/conflicting/redundant/equivalent proofs.
Existing terminal receipts distinguish veto, exact rejection, proxy pruning,
resource exhaustion and kept candidates. Artifact provenance describes the
selected assignments; it does not assert that every pass changed the source.

## Enforcement audit

| Route | Owner and acceptance evidence |
|---|---|
| Shared folding/removal/inlining | `Frontend::rules`, registry stage prerequisites and the independent interpreter/Node/native rule suites |
| Structural object/call/inline/string alternatives | Registry-filtered inventory, family publication proofs, recipe provenance and search veto/replay suites |
| Target rules and output families | `OutputTactics::check_policy`, `TargetRules`, the fixed rule permission map and executable output-veto cases |
| Names/properties | Hygienic plan eligibility, target property assignment and direct/reused-output veto cases |
| Runtime reconstruction | Direct table/array execution, independent objective searches and retained provenance tested under legacy and explicit-risk policies |
| Reuse and final admission | Policy checks retain original transformation/risk evidence; an equal-byte score never confers permission |
| Configuration and reference | Strict loader, resolver, CLI precedence suite and schema/registry agreement |

## Qualification

All 1,764 active library tests are qualified, including the large data-table
oracle (263.37 seconds); four pre-existing ignores remain. The ordinary run
passed 1,762 tests and exposed an obsolete assertion expecting a conflicting
debug alias to be silently overridden. The corrected conflict/consistent-alias
case passes, as does the additional unreached-discovery assertion. All 13 Rust
CLI tests pass. The production compiler implementation is unchanged by that
test correction.

All 19 public CLI precedence/registry tests and schema consistency pass.
The [corpus gate](ratchet.md) passes all 642 programs under each objective.
The [port gate](ports.json) passes Marked/Zod/PostHog's 29/1,353/21 reference
tests. [Full hash verification](comparison.json) preserves all 1,926 generic
and 20 port artifacts from the qualified objective-set pin `1484b18d`. No
size baseline changes. The retained [verifier](summarize.py) reproduces the
comparison and checks the immutable compiler identities. This milestone also qualifies the preceding scoped reach-accounting
change; its separately retained [CPU comparison](../2026-09-30-q2-target-reach-scoped/README.md)
is an accounting correction with overhead, not a speed claim. C1 makes no new
compilation-speed claim and does not mark any other migration milestone complete.

Logs: `/tmp/lilscript-c1-closure-{ordinary2,remaining,cli,release,precedence,schema,ratchet,ports,summary}.log`.
