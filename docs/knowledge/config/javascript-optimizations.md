# `javascript.optimizations` and `optimization_level`

Parent: [Config](README.md). Search mechanics: [candidate search](../compilation/candidate-search.md).

`optimization_level` is **search effort** 0–15. It does not weaken type checking or `[optimization]` IR passes (those have their own keys). Duplicate feature names and levels > 15 are errors.

A listed feature still needs its compression decision when it is dual-gated, and
`candidate_search = off` still zeros optional terminal work. Effort is not the
same as “this representation is searched”; see
[decision registry](../compilation/decision-registry.md) and
[objectives](../compilation/objectives.md).

## Level → features (`minimum_level`)

| Min level | Features |
|---|---|
| 1 | `startup-cost-guard` |
| 3 | `performance-shape-model` |
| 4 | `conditional-expression-variants`, `expression-phi-region-variants`, `local-phi-expression-region-variants`, `phi-edge-value-forwarding-variants` |
| 5 | `update-loop-variants`, `compound-mutation-variants`, `constructor-initializer-fusion-variants`, `fresh-literal-factory-inlining-variants` |
| 7 | `default-argument-variants`, `comma-expression-variants`, `ssa-destruction-variants` |
| 8 | `entropy-cross-scope-reuse`, `entropy-property-assignment` |
| 9 | `structural-loop-variants`, `parsed-peephole` |
| 10 | `ir-inlining-variants`, `ir-specialization-variants`, `structural-control-flow-variants`, `profile-guided-optimization` |
| 11 | `ir-closure-factory-variants`, `do-loop-variants`, `call-site-specialization` |
| 12 | `switch-lowering-variants`, `capture-signature-cloning` |
| 13 | `identical-function-folding`, `function-layout-variants`, `joint-representation-search` |
| 14 | `ir-function-subsumption-variants`, `ir-phase-ordering-variants`, `ir-compress-pass-variants`, `joint-chunk-symbol-search` |

Level also caps candidate count, retained bytes, beam width, and optional
terminal codec calls, including when an exact `optimizations` list is set. See
the effort table in [candidate search](../compilation/candidate-search.md).

## Exact allowlist

```toml
[javascript]
optimization_level = 0
optimizations = ["parsed-peephole", "startup-cost-guard"]
```

Only listed searches run. The allowlist chooses behavior, while
`optimization_level` continues to bound effort. `production` also caps
candidates at 384; `always` removes that search-mode cap but keeps the level and
explicit ceilings.

Empty `optimizations = []` disables all of these features. With candidate search
off, that means one configured emission and no optional terminal exact-codec
probes. Listing `parsed-peephole` does not override this hard-off.

`fresh-literal-factory-inlining-variants` is a late local representation search.
It can substitute only a private, direct-call-only, zero-argument function whose
complete body returns a fresh empty array literal. Each call remains a distinct
allocation. The exact configured codec scores the proposal across at most the two
best complete structural/name layouts and publishes one compiler output. Explicit
chunk plans keep the shared factory because their ownership/import planner does not
yet carry the declaration-suppression proof.

## Features that are also compression decisions

`ir-inlining-variants`, `ir-closure-factory-variants`, `ir-phase-ordering-variants`, `default-argument-variants` (via `callee-default-arguments`), `structural-loop-variants` (via `loop-spelling-selection`), `compound-mutation-variants` (via `mutation-spelling-selection`), `joint-chunk-symbol-search`, `joint-representation-search`.

Both gates must pass when using level-derived mode.

## Subsumption extra rule

`ir-function-subsumption-variants` is auto-searched only for `size-first`. Other priorities need the exact feature name **or** `optimization.function_subsumption = true`. `function_subsumption = false` always wins.

## Typical effort settings

Fast edit loop:

```toml
[javascript]
optimization_level = 0
candidate_search = "off"
candidate_limit = 1
candidate_byte_budget = 1
candidate_beam_width = 1
candidate_proposal_limit = 0
terminal_codec_probe_limit = 0
```

Maximum release (compile-time expensive):

```toml
[javascript]
optimization_level = 15
candidate_search = "always"
candidate_limit = 1536
candidate_byte_budget = 67108864
candidate_beam_width = 48
candidate_proposal_limit = 1536
terminal_codec_probe_limit = 384
cost_model = "brotli"
```

Checked-in root config: level 15, `production` search, 1536 limit (effective
384), 1 MiB byte budget, beam 12, and default caps of 384 optional structural
work units plus 384 optional terminal work units for artifacts through 16 KiB
(both size-scaled above that).
