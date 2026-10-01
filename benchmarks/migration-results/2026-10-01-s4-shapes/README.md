# S4 declared shapes

1. Add checked reference shapes with declared data/accessor fields, named
   construction, optional keys and field defaults. Reuse nominal field identity
   and private property naming; no fake public constructor or value-copy rule.
2. Lower spread through the declared source field set, preserving evaluation
   order, getter counts and CreateDataProperty semantics. Host extra keys never
   enter the result. Intersections require identical type/kind for shared keys.
3. Add immutable declared tags for `is`/`as?`, checked shared-key agreement and
   source-owned diagnostics for unsupported identities.
4. Carry accessor effects and boundary contracts through shared facts and
   development checks. Native unsupported storage receives a source diagnostic
   until N2 supplies its runtime; no hidden late refusal.

Prediction: private data shapes enable existing field/escape optimization and
mangling. Reflected storage keeps ABI keys; accessor operations remain ordered.
Optional-key omission has a real code cost and must not be optimized away.
Run focused semantic/boundary/configuration checks and independent raw/gzip/
Brotli comparisons after a coherent implementation batch, without a fleet rerun.

The implementation also adapts concrete product fields at reflected shape/class
storage and preserves current-root updates after a reentrant RHS. This closes a
previously uncovered public field ABI gap; formerly incorrect programs are new
capability oracles, not matched size wins. Optional spread defaults and explicit
absence have separate construction behavior.

## Result

Implemented declared data/accessor shapes, contextual keyed literals, field
initializers, intersections, declared-key spread, immutable tags and checked
casts, optional-key writes and recursive development crossing checks. Reflected
shape/class product fields use public value codecs, including reentrant projected
updates and assignment results evaluated once. Native shape storage and erased
public generic presence have source diagnostics and retain their N2/S4 owners.

Pin: `/home/azureuser/lilscript-work/bin/s4-shapes-1/lilscript`, SHA-256
`a621b1ba460dce82fbc96924a6e48b80458f46803057614943326f31077c388b`.
Algorithm 27 / search 31 / walk 10 / local facts 10. Exact compiler, source and
codec identities are in [identity.json](identity.json) and
[comparison.json](comparison.json).

Eight focused checks pass across this batch ([checks.json](checks.json)), with
only affected tests repeated after fixes. CLI, lint and LSP release builds pass.
Two frozen matched controls are unchanged: **388 raw / 299 gzip / 246 Brotli**
bytes in their respective objective lanes. Two new capability cases pass all
three objectives; every score equals its delivered artifact's exact codec size.
No full library/fleet rerun, native-complete result or CPU/runtime parity claim.

No new optimization toggle: these are checked source/ABI contracts, independent
of effort. Existing property-mangling permission controls private keys;
`checks="development"` adds descriptor, recursive-data and accessor-result
validation. Public codecs and optional-key helpers have real code/allocation
costs. R2's remaining private-storage work may remove unnecessary key handling
where observation is impossible; R18 owns erased public presence and wider
opaque/collection transport. R8 and the shared-consumer/catalog audit remain S4.
