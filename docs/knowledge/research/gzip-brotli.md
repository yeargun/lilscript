# Gzip and Brotli as compiler objectives

Parent: [research](README.md). Config:
[cost model](../config/cost-model.md). Measurement:
[codec contract](../verification/codec-measurement.md).

## Why raw-local minimization is insufficient

The [DEFLATE specification](https://www.rfc-editor.org/rfc/rfc1951.html) combines
LZ77 back-references with Huffman coding and limits backward distance to 32 KiB.
The [Brotli specification](https://www.rfc-editor.org/rfc/rfc7932.html) also uses
back-references and prefix coding, with a configurable sliding window plus a static
dictionary. Therefore token choice, repetition, frequency, and distance matter—not
only source length.

The format itself, with steppable encode/decode and both JS sides
editable, is [Brotli, the whole machine](brotli-machine.html).
Measured dictionary / mangling quirks live in the
[Brotli mangling lab](brotli-global-mangle/lab.html) (tiny-file generator:
`brotli-mangle-lab.mjs` + `render-brotli-mangle-lab.mjs`).
Hundred-kilobyte global-optimum playbook (jQuery, gl-matrix, Monaco,
in-tree audits, no compiler changes):
[brotli-global-mangle](brotli-global-mangle/README.md).

Consequences for JavaScript emission:

- deleting punctuation can remove a repeated context and lose compressed bytes;
- outlining can win raw but replace repeated bodies the codec would encode cheaply;
- inlining can either expose folding or duplicate code beyond a useful window;
- one-character identifiers are not interchangeable when their surrounding tokens
  have different frequencies;
- declaration/chunk order changes which substrings are in history;
- pooling strings may add syntax that duplicates the codec’s own dictionary work.

## Exact measurement is the terminal authority

Entropy, substring overlap, frequency, and window clustering are candidate-generation
heuristics. They can cheaply propose promising spellings, but only the configured
codec size of the complete artifact ranks a `size-first` finalist. Small exhaustive
tests should verify that the scorer, not the heuristic, chooses the winner.

## Current compiler settings

- raw: emitted UTF-8 byte length;
- gzip: `flate2` over statically bundled upstream stock zlib C 1.3.1 at level 9,
  with deterministic `mtime = 0` framing;
- Brotli: statically bundled official Google Brotli C 1.1.0, generic mode,
  quality 11, `lgwin = 22` (4 MiB nominal window setting);
- size-first: exact selected-codec bytes first;
- function-layout proposals: 32 KiB history for raw/gzip and `2^22` for Brotli.

The repository force-enables the bundled zlib build through `.cargo/config.toml`.
Release CI then runs exact fixtures and checks each desktop binary's dynamic
dependency table. A platform that cannot satisfy that static-library proof may run
the compiler diagnostically, but it cannot publish canonical size evidence.

The layout window is a proposal parameter. It is not proof that every match the codec
uses is captured by the similarity heuristic.

`lilscript-codec --json <artifact>...` is the verification authority. It calls the
same Rust library entry point and statically linked codec objects as candidate
selection, and reports library plus Cargo package identities. Node's compressors may
be recorded as deployment diagnostics but cannot decide a gate. That boundary was
added after the 2026-08 audit found level-9 length differences in 20 of 35 artifacts
between host zlib 1.2.12 and Node 24's separately patched zlib 1.3.1 build.

## Admission is not ranking

Under gzip/Brotli, a candidate enters the final pool if transfer is no larger than the
configured baseline **or** raw size is within
`max_candidate_raw_growth_percent`. This permits performance-oriented priorities to
consider candidates that do not win transfer. Size-first still ranks exact transfer
bytes first, so an admitted larger-transfer candidate cannot win that priority while
a smaller legal candidate remains.

## Tiny programs and streams

Headers and block decisions dominate very small streams, but that is real if a tiny
file is served independently. Keep per-case metrics and also measure ordered family
packs for broader context. Never skip a losing codec row because the result looks
noisy; label the delivery model instead.

## Chunks

HTTP normally compresses each chunk independently. Splitting discards cross-file
codec context and adds request/module syntax, but can reduce initial transfer and
improve caching. Report initial, lazy, total, and weighted deploy cost separately.
The best concatenated Brotli stream is not automatically the best web delivery plan.
