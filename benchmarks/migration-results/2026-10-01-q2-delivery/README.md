# Q2 delivery allocation owner

Delivery planning now admits reference walks and facts, atom/label propagation,
placement, layout trials, links, simulation, setter payloads and final plan
metadata before allocation. The existing compilation ledger owns all backing.
Temporary inputs release on return, refusal and unwind while setter creation
can still grow the original module's buffers. Rejected trials release their
own copies; accepted trials release the displaced layout. A resource refusal
propagates as admission failure rather than masquerading as an illegal merge.

Contexts borrow positions and namespaces. Label propagation, export linking and
lazy simulation avoid the former temporary clones. Placement compaction reuses
its file vector, and settlement reuses statement/module capacity. Template
inspection streams borrowed parts; admitted name expansion avoids hidden index
strings. Complete target cloning shares the same typed delivery-copy helpers.
No placement, naming, search or effort policy is intentionally changed.

There is no new optimization flag: this is mandatory resource accounting under
existing work and retained-memory limits. Insufficient limits can refuse earlier
now that formerly uncounted work/storage is admitted. Disabling caches grants no
additional budget or permission. This completes the planner owner; later
artifact/render metadata and source analysis/edit owners remain Q2 work.

[Sixteen focused checks](focused-tests.json) pass: exact surviving plan storage
for single/split/preserved delivery, module growth through setters, rejected
trial rollback, work/memory refusal, temporary-input success/refusal/unwind,
name expansion, shared state, static cycles, lazy evaluation, carried hosts,
imported entry facades, entry-specific order and hashed names. The test build
compiled the library tests but only these selected checks ran; no full library,
fleet, ratchet or port qualification ran.

Pin `/home/azureuser/lilscript-work/bin/q2-delivery-1/lilscript`, SHA-256
`7c8417b5a59c74c747943b4989e86dcd0c728beb9be2afeedc828926765640ac`.
Algorithm 51 / search 32 / walk 10 / local facts 15. [Source identity](identity.json),
[runner](measure.mjs), [comparison](comparison.json) and emitted artifacts retain
all results. **36 release checks pass**: six delivery controls × three objectives
× previous/current. Independent encoding agrees with manifests and the scored
entry rows; Node observations agree. Every file and exact judgment count matches
algorithm 50. Objective totals are **1,624 raw / 1,526 gzip / 1,131 Brotli** on
both pins. These totals count each entry's delivered closure, as search does.

Logical work rises because allocations and traversal that were previously
unaccounted are now charged. For example, shared split raw work rises
798,396 → 1,031,146; its peak retained reservation rises
16,056,648 → 16,063,163 bytes. Complete-build retained storage returns to zero
for every checked artifact. Several codec peaks fall slightly because shared
copy helpers avoid redundant capacity; codec storage dominates those peaks.
The single CPU samples are .00–.05 seconds. Most match at that resolution;
shared split gzip, shared single Brotli and in-place lazy multi-entry Brotli each
increase by .01 seconds. No speed or compression gain is claimed.

Q2 remains active for remaining global dependencies, shared/incremental
formation, per-module elaboration reuse and the other allocation owners.
No accepted ratchet baseline or port artifact changes.
