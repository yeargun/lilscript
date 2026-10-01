# Q2: primitive-class dependency propagation

Policy algorithm 61 replaces sixteen whole-program class sweeps with explicit
value, cell and returned-value dependencies. A compact reverse-edge index and
fixed queue revisit only affected producers. Each producer has a sixteen-visit
limit; exhaustion retains wider sound facts. All writers and complete caller
sets contribute, including omitted/default arguments. Open script roots,
unknown host values, reference writes and observable activations remain barriers.

The build and all 22 focused checks pass, including a 64-function dependency
chain, mixed writers, open roots, connected transformations, hard-refusal cleanup
and native captured closures. No full library or fleet rerun.

The release comparison covers the accumulated algorithms 56–61 against the
preceding algorithm-55 pin. All 54 checks pass across six delivery cases and
raw/gzip/Brotli: Node behavior, independent encoder scores, complete file hashes
and reuse-on/off logical work, search decisions and peak storage. Selected
totals remain **1,299 raw / 1,252 gzip / 947 Brotli**. Increased logical work
includes the newly admitted analysis and planner storage/work; these aggregate
costs must not be attributed solely to the class worklist. The tiny controls
are not a CPU-performance study.

`comparison.json` records source, compiler and encoder hashes, all files,
resources, search counts and samples. The current binary SHA-256 is
`0626cfe45fc8cd2baaf5057f6e2251ab55066d203af4be9dba23fe663286cff2`.
`run.py` and `measure.mjs` reproduce the affected checks. Q2 remains active for
remaining dependency scheduling, incremental formation, module elaboration and
final acceptance; no milestone-completion claim.
