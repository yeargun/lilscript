# S4 closed erased product transport

Replace the simple-forwarding requirement on private generic bodies with a
shared, admitted transport proof. Keep the existing cheaper forwarding proof as
a sufficient fast path. A bounded worklist examines branches, loops, collections,
generic products, captured closures and recursive helper groups. Complete
original call sets also qualify concrete owned callback arguments. A callback's
input proof is reused across its invocation sites within the query.

The fact grants representation-preserving private transport, not purity, a
constant value, termination or permission to inline. `JsValue`, `unknown`, host
storage and unqualified callable producers cannot expose private product
backing. Refusals retain the generic body's source operation span. The original
generic declaration and JavaScript calling convention remain erased; no cloned
source bodies or runtime type arguments are introduced.

Prediction: already supported forwarding keeps its emitted output; newly
supported generic bodies preserve value snapshots at effort 0 and 13 under
each independent objective. This is a capability expansion, with additional
bounded analysis for bodies outside the forwarding fast path, not a claimed
fleet compression or compilation-speed improvement. Public opaque crossings
and mutable host collection adapters remain S4 work.

## Result

The two runtime checks pass at effort 0/13 under all three objectives, covering
branch selection, recursive calls, array construction/slice/push/indexed writes,
maps, nullable results, nested structs, captured snapshots and concrete named/
closure callbacks. The source-span refusal check passes for direct and indirect
opaque egress, `unknown`, closure escapes and host ingress. Four existing generic
transport/reuse checks also pass, including zero retained backing after failure
and successful retry, unrelated primitive generic interfaces and bounded work
growth at 16/64/256 calls (30,200 / 118,301 / 477,167 preparation units).

The CLI release build passes. Pin
`/home/azureuser/lilscript-work/bin/s4-erased-transport-1/lilscript`, SHA-256
`b0d10fdb01b8bd1517ff6681fcaae880c380f607d52ca04b02101733deea397d`;
algorithm 34 / search 32 / walk 10 / local facts 14.
[Identities](identity.json), [comparison](comparison.json) and [runner](measure.mjs)
retain every input, output and independent oracle. The existing forwarding
control remains **50 raw / 69 gzip / 53 Brotli**, measured on each objective's
own output. All new capability oracles pass, including hard TOML vetoes for
inlining, constant folding and scalar replacement. Scores equal delivered sizes.
These new capabilities have no supported old-compiler size baseline; no fleet,
timing or native-completion claim accompanies them.

During verification, a test used unsupported explicit generic literal syntax;
it now uses the contextual `Pair{...}` syntax. Another negative test had a dead
opaque local, then an invalid `unknown` conversion; the retained test observes
the explicit `as JsValue` crossing. These fixture corrections do not weaken the
host-egress refusal. No ratchet baseline or port output changed.

Logs: `/tmp/lilscript-s4-erased-transport-tests5.log` (two runtime passes),
`/tmp/lilscript-s4-erased-transport-negative.log` (final refusal pass),
`/tmp/lilscript-s4-erased-transport-reuse.log`,
`/tmp/lilscript-s4-erased-transport-release.log`,
`/tmp/lilscript-s4-erased-transport-measure.log`.
