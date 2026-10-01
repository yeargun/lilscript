# Q2: admitted local normalizers and stable-proof replay

Policy algorithm 60 completes admission for return normalization and unreachable
planning, which the owner audit found still used inspection-only temporaries.
Stable local proofs retain their measured work and peak storage. An unchanged
body/created-status pair admits that same complete stage in both reuse modes;
`normalization_reuse=false` physically executes it. Edited/new/ newly created
bodies invalidate the proof, and only empty plans are reusable.

Return planning computes child-region expression safety once and scans terminal
suffixes backwards, replacing copied recursive prefix walks. Reachability uses
the shared dataflow solver with a one-bit-per-operation client projection, not
its general operations-by-state history. Both planners release all temporary
backing before edits/publication. The fixed nine-rule schedule uses stack
storage. Native planning now admits the shared created-body query as scratch.

No tactic/effort changes. Newly counted work and storage can refuse formerly
unaccounted tight limits. Dependency-qualified global work, changed-unit
formation, per-module elaboration and final Q2 qualification remain open; this
is not a Q2 completion or measured speed/compression claim.

## Qualification

The build and all 25 focused checks pass on the first run. These include a new
end-to-end equality check for selected bytes, logical work and peak storage
with normalization reuse on/off, created-body invalidation, declaration/control
flow, lazy terminal branches, cross-module and captured-call behavior, admission
cleanup, and the native captured-bank GCC/Clang O0/O2/UBSan case. The installed
Clang 18.1.3 is selected explicitly. No full library, release, CLI, ratchet or
fleet rerun. Exact test names and results are retained beside this record.
