# Q2 source edit storage

Source rules now grow and copy arenas through conversion's existing allocation
owner. Touched-unit maps, compaction/graft remaps, cloned bodies, shared tables,
cell names, function signatures and nested allocation/call payloads are admitted
before construction. Publication independently checks exact surviving storage;
the former after-the-edit net-size reservation is removed.

Compaction retains arena capacity and renumbers surviving records in place.
It stages operands and arguments because rewritten ranges need not be monotone.
Removed nested backing is released after destruction. Shared body/table copies
retain the original reservations until all readers drop; rule completion refuses
a reader that would outlive its owning scope. Temporary helper inputs and newly
created source storage use the original budget without cross-owner resizing.

A pre-existing namespace flattening defect is also repaired: the rewritten
owned function call now applies declaration defaults in the callee, preserving
the supplied argument count. The preceding release pin rejects the affected
fixture with a call-convention verification error.

This is required ownership behavior under existing work/memory limits; there is
no new TOML permission or effort gate. Funded outputs should match, while formerly
unaccounted work can refuse earlier. Retaining compacted arena capacity avoids
copies but can hold more memory than the former shrink-by-copy path. Q2's global
analysis/planning and later artifact/render owners remain outstanding.

[104 distinct focused checks](focused-tests.json) pass across the source rule
kernel and producers. They include exact admitted storage, shared-reader
lifetimes, work/memory refusal, JavaScript behavior, and native captured-bank
behavior under GCC/Clang. Four initial failures are retained with their fixes:
the namespace defect above, a new fixture's type spelling, an old test's owner
lifetime assumption, and the installed Clang path. Only those four checks were
rerun after repair. No full library/fleet/ratchet rerun occurred.

Release pin `q2-source-edits-1`, SHA-256
`d83d88ba32b1bf942dc636708386b6d2683619d4248ef06e2cbd9beed77b6276`,
algorithm 52 / search 32 / walk 10 / local facts 15. [Source identity](identity.json),
[runner](measure.mjs) and [comparison](comparison.json) retain **36 passing release
checks**: six controls × independent raw/gzip/Brotli objectives × old/new pins.
Node observations and independent encodings agree. Every delivered file and
judgment count matches; selected-objective totals remain **350 raw / 351 gzip /
304 Brotli bytes**. Complete-build retained charges return to zero.

Correctly counted baseline work rises by 46–1,133 units per control; peak
reservations rise by 232–2,424 bytes. Coarse CPU samples are .00–.01 seconds and
support no speed claim. Retained arena capacity accounts for part of the peak
increase. The repaired namespace fixture also passes a separate release Node
check (`4\n7\n`); the [preceding-pin refusal](previous-namespace-refusal.txt) and
[current JavaScript](namespace-fixed.js) are retained.

Q2 remains active for global analysis/planning, artifact/render storage,
remaining dirty dependencies, shared/incremental formation and per-module
elaboration. No ratchet baseline or port artifact changes.
