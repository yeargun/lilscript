# Marked Release Baseline: Bounded Failure

The single authorized attempt ran on 2026-09-19 with unchanged Marked sources,
configuration, build script, and the frozen 75-node suite/test inventory. The
unchanged build compiles four profiles serially: open Brotli, closed Brotli,
gzip, and raw. Its shared 300-second build deadline expired before completion.
No retry, profile reduction, library edit, or substituted artifact was used.

## Result

- The first invocation, `lilscript.toml`, completed successfully in
  **244.740 seconds**. Although its output is named `marked.raw.js`, this
  configuration optimizes for Brotli.
- **One of four compile outputs completed:**
  [artifacts/marked.raw.js](artifacts/marked.raw.js), 34,092 bytes, SHA-256
  `c08107d23ed741a987e8d9297f8f93594749c57323b1f03e7467b399120621de`.
  This partial artifact is **unscored and untested**.
- The full build was terminated at its 300-second deadline (`SIGTERM`,
  recorded elapsed time 300.010 seconds). The runner exited 1 and classified
  the complete build as untrustworthy.
- Canonical codec measurement and the **75-node production test stage did not
  run**. This is not a passing library baseline or package qualification.
- Compiler inputs, evidence tools, original library files, and existing
  dependency contents remained stable. Partial logs, the completed invocation
  receipt, and its output were preserved. No archive or validation failure was
  reported.

The frozen [receipt.json](receipt.json) has SHA-256
`b3d659bf1a9eedd6f1520ab25b222b69698c0d4620b80d6108b665fdcda4d52f`.
See also [markedlil.json](markedlil.json) and the
[build log](logs/markedlil.build.log).

## Provenance

The accepted release parent is
`benchmarks/migration-results/2026-09-19-artifact-service/run-2026-09-19T16-36-23.430Z/receipt.json`,
copied here as [parent-receipt.json](parent-receipt.json). Its SHA-256 is
`020c288f5b0e6bafc016e12cbb6d5a7a7ab53b9b28a32444c584c9781abbf533`;
the accepted compiler-input manifest digest is
`2e140666dd1cb5fd2f0cf82d0256cc0ed15e630a1defaaf4eca549c887e1e2f3`.
Both the parent qualification and its input-stability check passed.

The preserved executables are under
`/tmp/lilscript-public-integration-release-baseline-20260919/`:

- `lilscript`: SHA-256
  `3d8e450590247c53f928a7bc2eceb1ac50b60dfb21464c6629a55e8a813ea00e`.
- `lilscript-codec`: SHA-256
  `d55c6f33119cb11153f21145f45058ef3ed188c04554cc198af242f4b12e1ef9`.

The copied [wrapper.mjs](wrapper.mjs) and runner sources identify the actual
executed tooling. Node was pinned to 24.11.1; `RAYON_NUM_THREADS=2`. The source
snapshot contains 104 files and has digest
`41a47a645f2708a9c9093264182cd466fa44793b71b2a8a5110075bdf293cb50`,
identical to the earlier debug-timeout workload. Compiler and runner revisions
differ, so these attempts are not a controlled debug-versus-release comparison.

## Interpretation

The completed first invocation reports wall time 244,730.6 ms, accumulated emit
time 403,233 ms over 268 calls, codec time 13,299.9 ms over 541 calls, and
peephole time 27,845.6 ms over 74 calls. These counters may overlap or accumulate
concurrent/nested work; they are **not additive wall-clock phases**. They do not
establish codec dominance. Background host activity was not isolated, so this
failure is not release-performance qualification.

The maintained boundary remains unverified, as do its separately declared
closed-profile, exact UMD/browser, and installed-package coverage obligations.
No semantic-backend whole-library support or resolution of D2/D5 is claimed.

This README is post-run narration, added after the receipt was frozen. It is
not a parent input or part of the receipt's captured output inventory. The
receipt and recorded evidence were not rewritten to include it.
