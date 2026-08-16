# Startup and performance policy

Parent: [config](README.md). Ranking:
[global optima](../compilation/global-optima.md). Source anchors:
`StartupCostConfig`/`JavaScriptPerformanceConfig` in `src/config.rs`, generated syntax
analysis in `src/js_peephole.rs`, performance shape analysis in `src/profile.rs`, and
final ranking in `src/compiler.rs`.

`[javascript.startup]` analyzes generated syntax deterministically:

| Key | Default | Role |
|---|---:|---|
| `parse_weight` / `compile_weight` / `memory_weight` | 1 / 1 / 1 | build the startup tie score |
| overhead limits | 30% / 30% / 35% | hard candidate ceilings when `startup-cost-guard` is configured |
| `max_nesting` | unset | absolute ceiling whenever set, independent of the guard feature |

These are syntax-derived estimates, not measured browser milliseconds or heap bytes.

`[javascript.performance]` weights typed-IR deoptimization risk, allocation pressure,
indirect calls, and hot-code volume (defaults 32/12/24/1). Profile counters weight
functions/blocks. `max_regression_percent` (25) defines the
realistic-performance-first ranking bucket; it is not a hard candidate rejection.

Priority uses the metrics as follows:

- size-first: exact configured transfer bytes, then performance only for an exact
  transfer tie;
- balanced: `3 * normalized transfer + 2 * normalized performance`, then transfer;
- realistic-performance-first: over-limit bucket plus normalized transfer, then
  performance;
- performance-first: performance ratio, then transfer.

After that tuple, all policies tie-break by startup score, raw bytes, and lexical
output. Runtime claims still require a representative measured browser/workload gate;
the model only guides compilation.
