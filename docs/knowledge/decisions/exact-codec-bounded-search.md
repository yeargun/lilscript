# Exact codec scores, bounded search

Status: accepted. Parent: [design decisions](README.md).

## Intent

Optimize the bytes users serve. Raw length, gzip-9, and Brotli-11 are distinct,
non-additive objectives; a locally shorter spelling can compress worse.

## Decision

Each product compile has one authoritative transfer metric. The configured legal
incumbent is retained. Heuristics may order candidates, allocate work, and avoid
byte-identical repeats, but only the pinned complete-artifact scorer may select a
gzip/Brotli size winner. Other codecs are optional diagnostics, not mandatory
work for every intermediate candidate.

Search is deterministic and bounded. Reports name attempted, rejected,
unvisited, and starved families. `best-observed` is the normal guarantee;
`bounded-optimal` is reserved for a declared finite domain that was exhausted.

## Tradeoff

Exact scoring costs compile time, so coverage-first scheduling and cached
artifact hashes matter. A larger search is useful only when it reaches a legal
alternative or interaction worth measuring.

## Refusal

- No global-optimum language for a production beam.
- No raw-size proxy as the final gzip/Brotli authority.
- No full Cartesian product, unbounded codec probing, or solver without a
  measured case that requires it.

Details: [objectives](../compilation/objectives.md),
[candidate search](../compilation/candidate-search.md).
