# Phase 03 — control flow, functions, effects, and failures

Parent: [migration](README.md). Semantics:
[effects and purity](../language/effects-purity.md).

## Objective

Exercise the transformations most likely to drift when inlining, specialization,
closure conversion, SSA destruction, or structured emission changes phase order.

## Required families

- branches, nested early returns, `while`, `for`, `for in`, `for of`, break/continue,
  loop-carried phis, switch/match, and unreachable paths;
- direct/recursive/mutually recursive functions, defaults, extra arguments, methods,
  constructors, first-class function values, and arity/constructibility contracts;
- closures with zero/mutable/constant captures, factories, callback pipelines, and
  escaping identity;
- inferred and declared purity, removable calls, effectful arguments, host effects,
  and evaluation order;
- `try`/`catch`/`finally`, throw values, unused catch bindings, async/task/generator
  flows where implemented, including rejection and cleanup order;
- profile-guided and non-profiled forms with identical outputs.

Retain focused cases for emission-only single-use function expressions: an eligible
entry call (ordinary, generator, and exception-shaped bodies), plus public, ESM,
capturing, recursive, address-taken, reusable-caller, constructor/method, and
loop-site refusals. `for in`/`for of` cases must also prove that deferring the input
producer does not duplicate or reorder its effects.

## Search-specific pairs

For each structural family, retain at least one case where the configured transform
wins and one where its disabled alternative wins gzip or Brotli. These are not
expected compiler failures; they prove the complete-artifact candidate survives.

## Exit criteria

- Oracle coverage includes values **and** effect order, not stdout alone when stdout
  cannot expose the difference.
- Public function cases verify `.length`, constructibility, stable names where part of
  the boundary, and thrown/rejected results.
- No optimizer variant changes behavior relative to the configured baseline.
- Size gates pass for both fast and release configs.
