# S4 concrete public callbacks

Admit concrete struct-bearing callback parameters when the exported function
only invokes them. An admitted use proof follows local aliases and declaration
defaults, and refuses identity observation, escaping storage and captures.
The existing callable codec supplies the incoming adapter: arguments become
public objects and returned products become private snapshots. Required rest,
default, evaluation and exception behavior stays with the common callable ABI.

Development checks inspect the original incoming callable, including an unused
parameter, and each returned field before conversion. Shape and product checks
share the non-null object contract. Each valid getter is still read exactly once.
Production creates no development checks. No new effort-dependent semantics or
behavior flag is introduced; existing optimization vetoes remain hard vetoes.

Prediction: existing public product output remains unchanged in production.
New callback adapters have an explicit byte/runtime cost and are emitted only
where this ABI is needed. Unrestricted callable identity transport and mutable
host collections remain S4 work; a wrapper is not their identity solution.

## Result

Three focused callback checks pass (2.55 seconds), followed by 21 affected
existing product-boundary and shape checks (27.47 seconds). Coverage includes
effort 0/13, all three objectives, hard inlining/folding/scalar-replacement
vetoes, callable aliases, typed rest, declaration defaults, internal calls,
getter order/counts, exceptions, public arity and development failures for the
original callable and returned values. The first runtime check exposed the
synthetic default assignment in the use proof; the proof now admits defaults
and checked local reassignments without granting identity observation.

The CLI release build passes. Pin
`/home/azureuser/lilscript-work/bin/s4-public-callbacks-1/lilscript`, SHA-256
`4e4f5cdeb5448da427f8a15b9d4bf5d136cf0d090eb6702e25bc324a0758ddb3`;
algorithm 35 / search 32 / walk 10 / local facts 14.
[Identities](identity.json), [comparison](comparison.json) and [runner](measure.mjs)
retain independent behavior oracles, exact delivered sizes and scored/delivered
agreement for each objective. The existing production product control stays
byte-identical. The new callback capability has no supported old-compiler size
baseline. No full-library/fleet/timing or native-completion claim; no ratchet or
port artifact changed.

Logs: `/tmp/lilscript-s4-public-callbacks-tests2.log`,
`/tmp/lilscript-s4-public-callbacks-boundaries.log`,
`/tmp/lilscript-s4-public-callbacks-release.log`,
`/tmp/lilscript-s4-public-callbacks-measure.log`.
