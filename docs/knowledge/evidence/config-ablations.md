# Configuration and optimization ablations

Parent: [evidence](README.md). Config behavior:
[config](../config/README.md). Measurement contract:
[codec measurement](../verification/codec-measurement.md).

An ablation changes one named input while holding source, target, boundary, toolchain,
codec settings, and other config constant. Both outputs must pass the same semantic
oracle before their sizes are eligible.

Useful maintained families in this repository include finite-value propagation,
function folding/subsumption/layout, profile-guided specialization, inlining/phase
order, SSA/phi destruction, pooling/packing, regex/catch/generator spelling,
compress passes, and chunk planning. Their current commands and measured rows live in
the benchmark folders. [`docs/benchmark-results.md`](../../benchmark-results.md)
contains explicitly historical rows; only a regenerated report with current scorer
provenance can support a current byte claim.

## Required report

- exact source and boundary;
- enabled vs disabled resolved config, including priority/search/cost model;
- raw/gzip-9/Brotli-11 of exact bytes and artifact hashes;
- semantic result for both variants;
- candidate count/compiler time when search changed;
- runtime/startup/memory result when the transform changes representation/work;
- interpretation scoped to the fixture.

Do not label a default-vs-custom build a single-pass ablation if priority silently
changes ten other tactics. Use exact allowlists/hard-offs or an explain dump to prove
the intended difference. A focused win supports “this transform wins here,” not
“enable it unconditionally.”

An ablation gates only the metric selected by its resolved config. Raw, gzip,
and Brotli may all be recorded for the exact artifact, but the two unselected
metrics are diagnostics and may regress. If several metrics need normative
claims, compile and verify a separate artifact for each objective.

For a non-local candidate, also retain the locally attractive loser and explain which
complete codec metric selected the winner. This is executable documentation of global
search behavior.
