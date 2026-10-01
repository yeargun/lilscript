# G2 private property naming completion contract

G2 is complete after G1 and its class-owner correction.
[Evidence](../../benchmarks/migration-results/2026-10-01-g2-complete/README.md) records its focused qualification. Field identity and the checker's reflected-set closure own
legality; mangling is an exact-judged representation choice under the existing
`property-mangling` control.

1. [x] Audit all construction and access consumers. Remove the blanket generic
   exclusion where the existing reflected-set closure proves private storage.
   A class identity test alone is not field reflection; however, kept-class
   assignment initialization can observe prototype setters. Broaden those
   classes only under the explicit pristine-builtins contract. Preserve extern,
   published, reflected and host-derived inheritance families and reserved keys.
2. [x] Reuse property slots across sibling branches of private inheritance
   trees while keeping inherited fields coherent and fields on one instance
   distinct. Use the complete legal continuation alphabet. Simplify allocation
   to the actual interference relation and avoid family-wide wasted slots.
3. [x] Keep the `property-mangling` veto effective in direct formation, retained
   heads and search; preserve the unmangled candidate and document eligibility,
   defaults, risks and compile/size tradeoffs. Do not reorder field initialization:
   legal declaration movement is G3's task.
4. [x] Replace the two stale adapter-name fixture expectations with explicit
   existing name-observation contracts and independent anonymous-wrapper oracles.
   Do not change public constructibility or R7 call/adapter semantics.
5. [x] Run one focused property/reflection/permission batch and independent
   raw/gzip/Brotli comparisons on affected open and closed workloads. Include
   computed/enumerated/serialized/extern/generic crossings, inheritance and
   prototype pollution. Held-out calibration and competitor qualification remain V1.
6. [x] Update the checklist, coverage, history and handoff; commit/push before G3.

Expected result: more legal private property choices and shorter/reused keys;
exact codec results decide whether to use them. No runtime work is added by
renaming. Prototype observability remains an explicit eligibility constraint,
not an implicit strengthening of the source/host contract.
