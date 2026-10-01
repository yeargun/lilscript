# S4 authored string pooling

1. Consume `@pool` on functions and methods, including nested closures, defaults,
   templates and module imports. Keep checked string identities in the module's
   immutable metadata so folding, inlining and dead-body removal cannot discard
   a live literal's request. Reject misplaced attributes instead of ignoring them.
2. Reuse the target pool with explicit authored admission, independently of codec
   thresholds and search seeds. Honor `string-pooling` vetoes through source
   conversion, formation and artifact replay. Native already shares immutable
   constant payloads and needs no additional transformation.
3. Admit new metadata/payload storage, preserve UTF-16 strings, document behavior,
   and qualify source/permission/resource/runtime and matched objective cases in
   one coherent batch. No full-library or fleet rerun.

Prediction: no directives means unchanged artifacts. Pinned sharing can increase
raw or compressed size and must not be advertised as a universally winning
heuristic. Dead or proved-away values need no storage; remaining live strings
must honor the pin regardless of the chosen objective.

## Result

Pin `s4-pool-2`, SHA-256
`78d13f51ca5a82b1df771107160ae9afc2272bb99f09ffb7671eca9fdb152ea6`;
algorithm 39 / search 32 / walk 10 / local facts 14. The baseline is the
preceding immutable `s4-inline-for-1` pin. [Identity](identity.json),
[comparison](comparison.json), [runner](measure.mjs) and compiler-written
artifacts retain the source/config/codec/byte identities and Node oracles.

Fifteen focused checks pass: source/runtime on all objectives, native constant
sharing, hard source and replay vetoes, misplaced attributes, mutable target
encodings, and memory/work refusal cleanup. A final three-check rerun covers the
template correction below. The release CLI and configuration-schema check pass.
No full-library or fleet rerun was performed.

| Program | Raw before → after | Gzip before → after | Brotli before → after |
|---|---:|---:|---:|
| Unpinned control | 81 → 81 | 90 → 90 | 73 → 73 |
| Pinned branches | 81 → 87 | 90 → 96 | 73 → 74 |
| Template chunks | 62 → 80 | 79 → 94 | 56 → 66 |
| Imported pinned helper | 81 → 87 | 90 → 96 | 73 → 74 |
| **Total** | **305 → 335 (+30)** | **349 → 376 (+27)** | **275 → 287 (+12)** |

These directives deliberately override a savings heuristic. The growth is
expected for small strings, not a compression win or a default-policy change.
All 24 before/after objective artifacts pass independent behavior and exact
score/delivery checks. The ordinary control retains identical bytes and search
counts. No runtime-speed or fleet claim is made.

The first checks exposed a repeated-pass bug that re-pooled its initializer;
a one-shot formation marker fixes it while retaining the value constraints.
Array packing and data encodings now preserve pinned values. The first matched
comparison also exposed direct template embedding before pooling; template
formation now keeps pinned chunks as expressions for the pool. Resource tests
cover new metadata and restored parent charges, including borrowed class-field
defaults that are not authored in the pinned function. The flat-string encoding
fixture was corrected: that table belongs to array packing, while a nested table
exercises the separate data encoder.

`@pool` is complete for the documented contract. `@choose` and wider public ABI
crossings remain S4 work; later milestone scope is unchanged.
