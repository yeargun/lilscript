# N1 native integration evidence

Base: D3 `dbcb72f6`. Acceptance: [N1](../../../docs/migration/n1-acceptance.md).
Policy algorithm 73. Native language/runtime completion remains N2.

## Changes

Native target checking and compilation share representation admission and
original-module diagnostics. The CLI `--check --target native` does no C
rendering, compiler execution or optional JavaScript search. Native input still
passes through the shared semantic rules. Module-global initialization guards
now consume the shared effects/initialization proof, with possible early
callback/reentry accesses retaining their guard.

The library `NativeToolchain` owns compiler resolution, strict flags, providers,
header staging, subprocess diagnostics, publication and receipts. TOML
`[target.native]` controls are documented, validated and reflected in policy.
Ordinary CLI/case/Rust execution shares this owner. Explicit numeric/compiler
oracles retain their independent flags. Receipts identify actual compiler,
arguments, generated inputs, provider sources and executable; they do not
claim hermetic SDK, transitive include or linker-environment capture.

The native matrix identifies implemented, partial, pending and JavaScript-only
capabilities. Exception-status omission and native ABI-dependent specialization
remain N2 because they depend on its exception and ownership protocols. Their
legacy coverage requirements remain open there.

## Focused verification

The retained logs include initial compiler, fixture and runner failures and
their corrections. Across `combined-2.log` and `final-lib.log`, 14 distinct
focused Rust tests pass: configuration/driver failure, native/source checking,
shared folding and tactic permissions, native/all, provider/header delivery,
shared scalar storage/final-use transfers, and edited-program validation.
The two physical-storage tests exercise GCC, Clang 18 and sanitizer paths.
The final diagnostic-only additions are exercised by the final release census;
the monolithic Rust test build was not repeated for those additions.

`qualify.mjs` exercises seven public CLI cases: initialization guards off/on,
checking without an installed compiler, dependency-source diagnostics, provider
headers/config-relative paths, early reentry, and the actual `all` CLI path
with independent raw/gzip/Brotli deliveries plus native execution. It passes
on the final compiler pin recorded in `cli/qualification.json`.

`cases.json` records eight passing cells: four source cases under formation-only
and production C lanes. Six ordinary cells carry toolchain receipts; the two
explicit default-flags oracle cells preserve independent compiler invocation.
Those cells use pin 1, before the later source-diagnostic changes. The runner's
two affected composition tests and the generated configuration-schema check
also pass. No full library, fleet or native-performance suite was run.

The guard fixture emits 4,647 C bytes with the proof disabled and 4,359 with it
enabled, a reduction of 288 bytes and one guard. Both executables print `2` and
`3`. This is emitted-C evidence, not an executable-size or speed claim. The
callback fixture retains its guard. No JavaScript compression win is claimed.

## Capability census

`masked-cases.json` inventories all 403 cases and 260 existing native harness
masks. Masks overlap and often describe missing host setup; they do not alone
establish a compiler limitation. `census.mjs` runs target checking with common
semantic normalization and case controls retained. It does no C compilation,
runtime execution or optional JavaScript search.

The final census admits 134 and refuses 269. Native source refusals have
original source locations. Four refusals instead identify obsolete positional
public-ABI TOML settings; V1 owns those fixture migrations. The pre-fix census
is retained separately, including the export-location hole and the subsequent
host-class inheritance diagnostic found during qualification. No refusal was
silently reclassified as a passing runtime case.

`identity.json` records compiler pins. Executable bytes are represented by their
hashes/receipts, not checked into this evidence directory. The scripts reproduce
them. Logs preserve actual invocations and temporary paths.
