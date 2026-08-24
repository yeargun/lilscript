# Optimization-level frontier

This lane measures six search-effort levels (`0, 3, 6, 9, 12, 15`) on two
independent simple modules for each raw, gzip-9, and Brotli-11 objective. Each
row compiles and executes the module, checks the compiler's exact selected
objective against `lilscript-codec`, records compiler-reported and end-to-end
wall time, and enforces the level-specific cap on deduplicated surviving scored
artifacts.

For every fixture in this sampled lane, each measured level must match or beat
the best selected metric at all lower measured levels. This is a regression gate
over the six fixtures and six levels listed above, not a claim that every
internal search feature is nested for arbitrary programs. Whether level 15
retained the best lower-effort result is also reported for Pareto diagnosis.
Time is deliberately diagnostic: single-run wall time is too noisy for a
release gate. `javascript_selection.candidates_evaluated` is a deterministic
count of surviving scored outputs, not total proposals, emissions, codec calls,
CPU work, or a resource budget; the report labels it accordingly. Raw, gzip,
and Brotli use separately configured compiler artifacts and selection
algorithms; cross-metric sizes are diagnostic only.

The runner rejects missing or malformed explanation metrics and verifies that
every derived config retains the requested objective, `candidate_search =
"always"`, the 1536-candidate ceiling, and no explicit `optimizations` allowlist
that would bypass level-derived candidate caps. The schema-3 report records compiler,
source, reference, config, artifact, runner, scorer, and expected-stdout digests.
It also records a deterministic-results digest that excludes only the explicitly
diagnostic wall-timing fields.

```sh
node comparison/effort/run.mjs
```
