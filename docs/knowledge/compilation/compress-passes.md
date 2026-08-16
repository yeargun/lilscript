# Compress passes

Parent: [Compilation](README.md). Source: `src/compress_passes.rs`. Options: `ProjectConfig::compress_pass_options()`.

These are IR transforms aimed at size. They are **not** always-on even under `preset = "maximum"`: JS ANDs them with `javascript.compression` decisions and, for outlining, a default-off.

## Run order

| Pass | Compression decision | Default (size-first, maximum preset) | What |
|---|---|---|---|
| Path-sensitive constants | `path-sensitive-propagation` | on (size-first and balanced) | Sparse/relational constants; fold dead arms |
| Expression superopt | `expression-superoptimization` | on (size-first and balanced) | Bounded pure Int/Bool/string rewrites |
| Partial escape sinking | `partial-escape-sinking` | on (size-first) | Sink/scalarize allocs that escape on some paths only |
| Array pipeline fusion | `array-pipeline-fusion` | on (size-first) | Fuse eligible `map`/`filter`/`reduce` chains |
| Region outlining | `region-outlining` | **off** (`unwrap_or(false)`) | Extract repeated 4–28 instruction windows into helpers |

`[optimization]` can force each on/off. `preset = none` plus omitted keys leaves them off.

## Why outlining defaults off

Comment in `src/config.rs`: helpers often **win raw and lose gzip/Brotli**. Candidate
search can still try outlining via `ir-compress-pass-variants` (level ≥ 14): if the
canonical pass is off, a with-outlining clone is added only when the
`region-outlining` compression decision is allowed and
`[optimization].region_outlining` is not explicitly `false`; if configured on, a
without-outlining clone is added. The complete-artifact codec score picks.

When phase-order search has already built an aggressive-inline proposal, candidate
search may also cross outlining with that one strongest bounded proposal. It does not
multiply outlining across every optimizer tuple. Small optimizer budgets reserve the
ordinary outline contrast before this interaction, so the configured pipeline always
remains first and the interaction requires a third slot.

## Outlining legality and exact matching

The region hash is only a cheap bucket. Before two occurrences share a helper, the
pass canonicalizes their SSA live-ins and definitions and requires exact equality of
the operation/constant graph, operand-use structure, instruction result types,
live-in types, and live-out identities. An equal-looking opcode window with different
types or dataflow is not a match.

Candidate regions also reject dynamic-observable evaluation. `JsValue` coercions
that can call user code, proxy-sensitive reads/checks, and dynamic operations that
may throw cannot be outlined, merged, or moved behind a new helper call. The
remaining operation allowlist and the exact structural comparison are legality
proofs; the codec score only decides whether an already legal outlined artifact is
worth shipping.

Every generated helper is marked `FunctionOrigin::RepeatedRegionOutline`. Late
private-function subsumption, parameterized merging, and identical folding exclude
that origin so they cannot erase the deliberately introduced reuse boundary. A
closed-script IR probe can then score the outlined helper with `AllEligible`
pure-helper substitution: eligible leaf helpers may disappear, while a multi-use
outlined composite remains shared. No-inlining IR receives the complementary
`SingleStaticUse` pre-probe. Both are optional emission interactions under the
existing `pure-helper-inlining` decision, not new semantic passes.

## Always-on cousins (not this module)

Linear array/object builder fusion lives in the main optimizer. Identical-function folding and parameterized merging run after compress passes.

## Config

```toml
[optimization]
pipeline_fusion = true
partial_escape_sinking = true
region_outlining = false
expression_superopt = true
path_sensitive_propagation = true
parameterized_function_merging = true  # gated again by compression decision for JS

[javascript]
compression = [..., "array-pipeline-fusion", "partial-escape-sinking", ...]
```

An exact `compression = []` disables all of these for JS even if `[optimization]` says true, because `compress_pass_options` ANDs with `compression_enabled`.
