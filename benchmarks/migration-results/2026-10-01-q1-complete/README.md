# Q1 common representations and per-site calls

Final compiler `q1-complete-2`, SHA-256
`bd40f900d5219899f9e129146aedce011162e5d869aacff430b48ea8f19be7a4`.
[identity.json](identity.json) pins its complete Rust/Cargo source manifest and
codec. [measure.mjs](measure.mjs) reproduces [comparison.json](comparison.json)
against immutable S3. Sources, configs, independent Node oracles, complete
outputs and compact work receipts are under `artifacts/`.

| Scope | Programs | Raw delta | Gzip delta | Brotli delta |
|---|---:|---:|---:|---:|
| Ordinary permissions, effort 13 | 15 | −9 | 0 | −1 |
| Recurring helper-sharing opt-in, effort 13 | 1 | 0 | 0 | 0 |
| Startup encoding opt-in, effort 13 | 1 | 0 | 0 | 0 |
| Mixed helpers, effort 14 | 1 | −9 | 0 | −1 |

All 108 old/new behavior observations pass. No individual cell grows. The mixed
helper program retains its large literal-producing body while copying a small
arithmetic body. The same gain at effort 14 does not establish an extra benefit
from joint exploration; that bounded mechanism is qualified for legality,
permissions and incumbent protection, with broader calibration owned by Q3.

## Implementation and checks

`src/representation.rs` now owns family metadata, site/alternative identities,
immutable maps and finite site/joint scheduling. Structural layouts and inline/
string recipes use the same registry for discovery and publication, retaining
their typed legality evidence. Target data and per-helper expression duplication,
constant signatures and sharing use its assignments. The JS-only choice types,
data-only scheduler and three separate call-rule entries are replaced.

Raw estimates rank but do not suppress offered alternatives. Extra family and
bounded pair moves run after the ordinary completed walk, automatic at effort
14 with `representation-joints` overrides. Selected maps retain tactic/runtime
risk and joint provenance, including under replay. Canonical rollback and old
whole-family choices remain. One artifact arena/verifier/exact admission path
owns all delivered candidates. Policy algorithm 15 / search 30 / walk 9 identify
the changed behavior.

**48 distinct focused checks pass** across `focused.log`, `adjacent.log` and
`final-checks.log`: mixed choices, immutable identities, joint prefix protection,
TOML/explicit-map/replay vetoes, structural discovery and table decoders. The two
initial Q1 failures were fixture issues (a reserved function name and an assertion
that ignored family-key ordering); the corrected checks pass. An older deferred
naming witness already failed on immutable S3 because S3 found its result in the
ordinary search. Disabling inlining in that focused fixture restores the original
uphill naming behavior and both the witness and prefix assertions pass. No known
failure remains. The generated config reference and `git diff --check` pass.

Work across 54 objective cells:

| Counter | S3 | Q1 |
|---|---:|---:|
| Structural proposals | 180 | 180 |
| Structures | 78 | 78 |
| Renders | 234 | 234 |
| Structural codec probes | 86 | 86 |
| Terminal exact judgments | 368 | 416 |
| Terminal positions examined | 6,165 | 6,494 |

These are logical work counts, not timing claims. The final release only differs
from candidate 1 by explicit algorithm/schedule version markers; its complete
size/behavior comparison was repeated under the final pinned binary. The earlier
identity is retained separately. No full-library/fleet run, accepted ratchet
update or port rebuild was performed. V1 owns broad qualification.
