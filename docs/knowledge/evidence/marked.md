# MarkedLil evidence

Parent: [evidence](README.md). Required row:
[library proof matrix](library-proof-matrix.md). Live snapshot:
[`docs/current-status.md`](../../current-status.md). Historical investigation:
[marked-01](../migration/board/notes/marked-01.md).

## Boundaries

Keep three rows separate:

1. published reusable package API and compiler-selected ESM;
2. parse-only diagnostic against the pinned official parser path;
3. closed-key diagnostic where all option-key producers/consumers are controlled.

The package row preserves public option keys, defaults, `marked()` behavior,
ESM/CJS/UMD exports, and official parse results. The closed-key row is not the
public ABI.

## Evidence Status

Current semantic/API tests pass. The latest local compiler migration regressed
the package Brotli artifact against the frozen compiler while improving its gzip
diagnostic. That is not a Brotli win. The closed-key test also exposed and now
covers a real compiler bug where dynamic-boundary stability overrode explicit
closed-key policy.

Fresh tracked evidence must replay the prior legal Brotli recipe and compare the
published boundary with eligible independent JS baselines. Historical board and
research numbers are not current authority.

## Engineering Direction

- retain public option/API keys in the reusable package;
- distinguish compiler-owned closed keys from real host extern fields;
- broaden ownership/no-hook proof only for non-escaped compiler allocations;
- preserve the old legal artifact as a replayable incumbent;
- never use a gzip gain to pass a Brotli objective regression.
