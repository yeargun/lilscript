# Codec measurement

Parent: [verification](README.md). Compiler config:
[cost model](../config/cost-model.md). Research: [gzip/Brotli](../research/gzip-brotli.md).

## Byte identity

Measure the exact emitted file bytes. Do not trim whitespace, remove a final newline,
decode/re-encode Unicode, or concatenate chunks without a specified framing. Record
SHA-256 for source artifacts used in a published row.

## Required metrics

| Metric | Verification setting                                                             |
| ------ | -------------------------------------------------------------------------------- |
| Raw    | byte length of the exact file                                                    |
| Gzip   | bundled upstream stock zlib C 1.3.1, level 9, deterministic header (`mtime = 0`) |
| Brotli | bundled official Google Brotli C 1.1.0, generic mode, quality 11, `lgwin = 22`   |

The compiler scorer and canonical verification lanes use the same encoder code,
not merely the same quality or version label. Gzip is statically linked through
`libz-sys = 1.1.24`, which bundles upstream stock zlib C 1.3.1. Brotli is statically
linked through `compu-brotli-sys = 1.1.0`. Both the compiler-in-loop ranker and the
batch `lilscript-codec --json <artifact>...` verifier call the same library
measurement functions. The verifier records those package/library identities and
the exact parameters once, then returns raw/gzip/Brotli sizes for every requested
artifact in order.

Node's built-in compressors are diagnostics, never gate authorities. This distinction
is necessary even when a runtime reports the same nominal version: the 2026-08 audit
found 20 length differences across 35 artifacts between host zlib 1.2.12 and Node
24.11.1's patched `1.3.1-470d3a2`, and stock zlib 1.3.1 reproduced neither every
Node length nor every Node payload. The former pure-Rust Brotli encoder likewise
ranked tiny artifacts differently from official Brotli C 1.1.0. Encoder identity is
therefore part of the objective contract, not incidental provenance.

`benchmarks/codec-contract.test.mjs` also walks the repository's non-generated
JavaScript, shell, and package manifests, including future runner directories. It
rejects direct Node `zlib` imports, built-in compression APIs, alternate codec
packages, and direct gzip/Brotli subprocesses. This prevents a new report from
silently reintroducing a platform-specific encoder.

`.cargo/config.toml` force-sets `LIBZ_SYS_STATIC=1`, because `libz-sys` otherwise
allows an environment variable to override its static Cargo feature. CI and release
jobs execute the exact scorer fixtures on Linux, macOS, and Windows and reject a
shipped binary whose dependency table names zlib or Brotli. Canonical publication is
currently limited to those attested desktop targets; targets on which `libz-sys`
unconditionally selects a system library are not eligible evidence producers.

The default verification path builds `lilscript` and `lilscript-codec` together from
the same checkout. `LILSCRIPT` and `LILSCRIPT_CODEC` overrides are hashed and useful
for diagnosis, but two unrelated overridden binaries do not by themselves prove that
the compiler's internal search scorer matches the batch verifier. A published
canonical run must use the joint default build or otherwise attest a matching build.

## Per-case and packed-family views

Tiny files include stream headers and may show ties or counterintuitive compressed
sizes. Report them anyway. Also publish an ordered, length-framed family pack so
repetition across realistic code volume is visible. The pack is additional evidence;
it must not replace or waive an individual case gate.

## Multi-artifact delivery

For split output report at least:

- entry/eager raw, gzip, Brotli;
- each lazy/shared chunk;
- initial reachable set;
- full reachable set;
- request count and dependency depth;
- the configured weighted deploy cost.

Compress files independently, as HTTP transfer normally does. A concatenated all-code
stream may be recorded as a diagnostic but is not served-byte evidence.

## Metric-specific baselines

Find the minimum eligible baseline independently for each metric. Never select a
single JS tool by Brotli and then compare LilScript raw/gzip only to that tool. Record
ties exactly; do not round KiB values for gating.

LilScript's objectives are independent too. Compile a raw-selected artifact for the
raw gate, a gzip-selected artifact for the gzip gate, and a Brotli-selected artifact
for the Brotli gate. Only the selected metric is normative for that artifact. Its two
cross-metric measurements remain useful diagnostics, but a Brotli win is not revoked
because that Brotli-selected spelling is larger raw or gzip.

## Determinism check

Rebuild each artifact at least twice in the reproducibility lane. Byte drift is a gate
failure even when compressed sizes happen to tie, because unstable naming/layout
invalidates trends and caches.
