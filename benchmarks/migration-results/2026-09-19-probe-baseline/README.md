# Preserved-Release Probe Attempt

One unchanged-source attempt ran on 2026-09-19. The original two-case base
adapter passed. The original 22-profile matrix reached its 240-second deadline
after ten profiles compiled and matched the golden output. The complete canary
requirement remains **unverified**; this is not a passing 22-profile baseline.

This README is post-run narration, not an input or output of the frozen receipt.
No compiler, library, original configuration, assertion, or runner source was
changed. No retry was performed.

## Evidence

- [Final receipt](source-built-release/receipt.json):
  `0e862def92869916cced7f8871e5d0c0e1a6a48d4f920fd71f99e7d35cafc4d7`.
- [Original base report](source-built-release/tests/probelil/report.json): two
  unique frozen Node case IDs, both passing; SHA-256
  `06931c4d0579b0171625bdf3789edfecee5f68f8f6019bd3501a4a1cf5017c8a`.
- [Matrix rows](source-built-release/matrix.json), [original stdout](source-built-release/matrix.stdout),
  [stderr](source-built-release/matrix.stderr), [invocations](source-built-release/matrix-invocations/),
  and [passive runtime loads](source-built-release/matrix-loaded/) preserve the
  completed profiles. Each observed profile has matching exact artifact and
  host-runner bytes in the same child process.
- [Canonical codec stdout](source-built-release/canonical-codec.stdout):
  `a1e3afc9751e3e9dc981926fa8a7169dfeede3bc22a0a9e5f20ebf38b8543eac`.
- [Outer supervision](source-built-release-supervisor.json) completed normally
  with exit 1 in 282.290 seconds, below its 390-second deadline. The inner matrix
  reached 240.008 seconds, received SIGTERM, and its exit was observed without
  forced pipe closure. The base command took 41.412 seconds.
- [Post-run checks](source-built-release-postrun-check.json) rehashed all 109
  recorded output files and found no matching live compiler/wrapper command
  paths. This observation is not proof against escaped process sessions.

`balanced.toml` was retained without an output or completed invocation receipt:
the unchanged sequential script was processing this eleventh row at timeout.
The following eleven rows were not reached: `noPeephole`, `arrows`, `functions`,
`looseNames`, `beam1`, `beam32`, `freqNames`, `freqNamesSearchOff`, `idiomNames`,
`idiomNamesSearchOff`, and `phiRegions`. The four `name_ordering` rows remain
required; their known configuration incompatibility was not executed here.
The 36 evidence errors are three missing/inconsistent-evidence checks for each
of the twelve unreported rows, not 36 observed compilation or behavior failures.

## Completed Outputs

Sizes are independent file measurements, in bytes. Matrix compile times are
single invocation observations on a non-isolated host, not performance claims.

| Output | Raw | gzip9 | Brotli11 | Compile seconds |
| --- | ---: | ---: | ---: | ---: |
| Base optimized | 4034 | 1605 | 1492 | 30.806 |
| Base preset-none | 7309 | 2621 | 2359 | 9.682 |
| Matrix shipped | 4034 | 1605 | 1492 | 30.628 |
| Matrix level0 | 4442 | 1769 | 1624 | 0.170 |
| Matrix level5 | 4090 | 1663 | 1536 | 5.126 |
| Matrix level9 | 4056 | 1602 | 1492 | 24.702 |
| Matrix level15 | 4054 | 1610 | 1495 | 33.356 |
| Matrix searchOff | 4093 | 1678 | 1556 | 0.206 |
| Matrix searchAlways | 3920 | 1569 | 1465 | 55.088 |
| Matrix gzip | 3868 | 1592 | 1515 | 28.754 |
| Matrix raw | 3866 | 1648 | 1564 | 27.706 |
| Matrix perfFirst | 4089 | 1610 | 1494 | 30.711 |
| Required host runner | 70 | 88 | 74 | Not compiled |

The required host runner is counted separately, not hidden in a program-only
deployment claim. Completed matrix rows are original script assertions, not
new frozen Node test identities. All retained bytes/configurations are in
[artifacts](source-built-release/artifacts/).

## Provenance And Scope

The [copied accepted release receipt](source-built-release/parent-receipt.json)
is from `2026-09-19-artifact-service/run-2026-09-19T16-36-23.430Z`, SHA-256
`020c288f5b0e6bafc016e12cbb6d5a7a7ab53b9b28a32444c584c9781abbf533`.
Its source-input identity is
`2e140666dd1cb5fd2f0cf82d0256cc0ed15e630a1defaaf4eca549c887e1e2f3`.
The preserved release compiler is
`/tmp/lilscript-public-integration-release-baseline-20260919/lilscript`, SHA-256
`3d8e450590247c53f928a7bc2eceb1ac50b60dfb21464c6629a55e8a813ea00e`;
its companion codec is
`d55c6f33119cb11153f21145f45058ef3ed188c04554cc198af242f4b12e1ef9`.

The seven original Probe source files have unchanged snapshot identity
`1db0b6087d88e52cb10cda9f1cb8fa34a631e2571ed5afc0dedb6a11565c4530`.
The external preset-none configuration, wrapper, existing evidence owners,
Node 24.11.1 binary, original matrix, and original golden output were pinned;
before/after input and source checks passed. Probe has no dependency tree to
substitute. The executed wrapper SHA-256 is
`95a0836d68bbfd3e6f5e58ac1ecf5dc6f22da441a309e3b171a4772d3ff05a13`.

This is a legacy-backend observation tied to that preserved source-built
release, not current semantic-backend support, a full maintained-library fleet,
a compressed-size win against another compiler, or a speed qualification.
Current compiler sources were not substituted for the accepted parent identity.
