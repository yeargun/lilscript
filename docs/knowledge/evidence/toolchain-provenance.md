# Toolchain and result provenance

Parent: [evidence](README.md). Baselines:
[toolchains](../verification/baseline-toolchains.md). Codec bytes:
[measurement](../verification/codec-measurement.md).

A reproducible size row records more than a package lock:

| Input | Record |
|---|---|
| LilScript | version/commit or binary digest, target, mode, config content/hash, profile hash |
| JS baseline | tool/version, API/CLI options, target/module mode, extern/export/unsafe assumptions |
| source | LilScript/JS/host/fixture hashes and public boundary |
| codecs | implementation, gzip level/header, Brotli quality/window/mode |
| runtime | Node/browser/OS/architecture and timing protocol where relevant |
| output | exact artifact SHA-256, raw/gzip/Brotli, semantic/API result |
| harness | runner/report schema and digest, selected filter, catalog count |

Current pinned baseline locations:

- micro Terser, Rolldown/Oxc, esbuild: `benchmarks/popular/package-lock.json`;
- Vite/popular tooling: the same package lock;
- Closure apps: per-app `versions.env`, verified JAR SHA-256, and build reports;
- popular packages: `benchmarks/popular/package-lock.json` plus selected entrypoint in
  generated result JSON.

Canonical transfer measurement is supplied by the release `lilscript-codec` binary:
upstream stock zlib C 1.3.1 through `libz-sys = 1.1.24`, and official Google Brotli
C 1.1.0 through `compu-brotli-sys = 1.1.0`. Reports record the scorer binary digest,
package/library identities, and parameters. A Node or system-codec measurement is
diagnostic and cannot populate an eligible gate cell.

Default runners build compiler and scorer together. Runners that support overrides
require `LILSCRIPT` and `LILSCRIPT_CODEC` as a pair and record both digests; two
unrelated overridden binaries can measure final artifacts consistently but cannot
prove that compiler-in-loop candidate ranking used the same codec objects, so they
are diagnostic unless the matching build is independently attested.

Generated micro reports distinguish `catalogCases` from selected `cases`; a focused
run overwrites ignored summaries and must never be quoted as a full run. The latest
checked-in jQuery numbers come from `benchmarks/popular/build/jquery-results.json`,
but that row predates the canonical scorer and must be regenerated before it becomes
a current byte claim; prose tables are summaries, not a second database.

Network “latest” checks are research inputs, not reproducibility. Update a pin
explicitly, rerun semantic validity first, regenerate all affected artifacts, then
review size drift.
