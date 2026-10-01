# S4 generic values and public boundaries

S4 remains active. This batch closes concrete generic struct schemas and
nested type parsing, nullable/default/rest struct adapters, published
constructor and prototype boundaries, and dual type/constructor imports and
explicit re-exports. Declaration identity still chooses the storage recipe;
instantiated types choose nested conversions. Constructors retain their class
and prototype identity, and internal construction/super calls use the same
public field ABI as host calls. Dynamic namespaces select runtime exports.

These conversions are semantic requirements, not optional tactics. Existing
TOML hard vetoes remain unchanged. Algorithm identity advances to 21; search
and walk identities remain 31 and 10. Opaque unions and mutable collection
crossings without an alias-preserving representation remain refused.
Observed generic classes, wider generic callable transport and the rest of
S4's language/fact work remain open.

Prediction: previously refused concrete struct boundaries become usable, at
the required cost of copying public value fields. Existing supported examples
should retain their objective-specific sizes. Qualify the affected boundary
oracles after the batch and compare a small matched pair of existing cases;
there is no full library/fleet rerun or timing claim.

The compiler pin is `/home/azureuser/lilscript-work/bin/s4-boundaries-1/lilscript`,
SHA-256 `b80138abcdf0504f274219880b631387230e97e4b50c63d032cc35b26fb68b26`.
[identity.json](identity.json) records its exact Rust/Cargo inputs.

Twenty-three focused checks pass, including eight new regressions and fifteen
existing boundary/constructor checks. The parser's 43 existing checks and the
nominal core group also passed during this batch. No full suite was run.
Failure-driven corrections preserve separate target expression occurrences and
admit spreads in `super` argument lists. Statement inlining keeps real rest
arrays and spread evaluation; a Node oracle checks both and an ordinary inline.

[comparison.json](comparison.json) records six matched objective cells against
the immutable `s4-contracts-2` compiler. Two development cases total unchanged
**487 raw, 256 gzip-9, 204 Brotli-11 bytes**, each using its own objective's
artifact. A new combined generic-struct/constructor boundary oracle passes on
the release compiler under all three objectives. The judged byte counts match
the delivered artifacts. The new capability has no prior supported artifact,
so its sizes are recorded without a byte-saving claim. No timing, fleet or
Closure claim; no port output or ratchet baseline changed.

Remaining public callable work includes optional/rest and explicit receiver
transport through function values; this batch's new adapters qualify declared
exports, constructors and prototype methods. Wider generic/host work remains
S4-owned.

