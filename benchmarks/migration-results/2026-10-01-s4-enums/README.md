# S4 ABI enums and flag sets

1. Retain explicit integer/string ABI values and declaration ordinals as checked
   enum metadata. Support `enum Kind: string { Text = "text" }`, integer ABI
   enums, `Kind.from(value)`, `.abi`, `.ordinal` and integer flag sets. Validate
   duplicates, ranges and closed domains at the declaration/source operation.
2. Use the declared ABI value as the canonical representation for these new
   enums. This keeps mutable collections, reflected fields and generic/host
   transport consistent without changing object identity. Private alternative
   representations must retain explicit conversions at observations and belong
   to the existing Q4 schema/data choice owner, not a silent semantic shortcut.
3. Share constant/type/operation metadata through folding and both targets.
   Closed-enum matches remain exhaustive. Development crossings validate the
   declared domain. Explicit-ABI migration refuses observations of an enum that
   declares no ABI; legacy source keeps its existing compatibility contract.
4. Document the source selector and its effort-independent behavior. Finish the
   implemented enum paths before focused compiler/runtime/config/ABI checks and
   matched raw/gzip/Brotli measurements. Payload variants and sealed virtuals
   remain within S4 and follow this enum work; this batch alone does not close R8.

Prediction: typed string/integer protocols expose closed-domain facts and avoid
handwritten conversion chains. Canonical ABI storage may leave private enum
compression opportunities for Q4; exact objectives, not spelling length, must
judge those later choices. No full-library/fleet rerun or timing claim.

## Implemented and checked

ABI values, ordinals, checked `.from`, `.abi`, flags, exhaustive matches and
folding share checked enum identities. Both targets use the same canonical ABI
storage. Development crossings reject invalid domains without host coercion
hooks. The parser now admits the signed int32 minimum directly; folded nullable
enum literals retain domain verification. Value bindings shadow enum namespaces.

Three focused tests pass: all JavaScript objectives in production/development,
shared/native behavior at effort 0/13, and source/config/domain refusals. They
cover mutable collection aliases, public shape fields, optional results, exactly
once effectful conversion, high flag bits, shadowing and legacy compatibility.
The final shadowing extension was checked separately after the group. CLI/lint/
LSP release builds pass. The generated configuration schema was refreshed.

Compiler pin: `/home/azureuser/lilscript-work/bin/s4-enums-1/lilscript`, SHA-256
`e4e7546ef440edd0ee3dc84d044d56a4b557b41a060892d8d7b91f328f12f3de`.
Algorithm 30 / search 31 / walk 10 / local facts 13. [Identity](identity.json),
[measurement recipe](measure.mjs) and [comparison](comparison.json) retain the
source/config/binary hashes and independent Node oracles. The unchanged legacy
enum control stays **91 raw / 88 gzip / 78 Brotli**. The new ABI protocol has no
old-compiler comparator; all three objective outputs satisfy the same oracle,
and their delivered byte scores match the compiler's scores.

This is a source-capability batch, not a compression or runtime-speed claim.
Canonical storage leaves private representation choices to Q4. S4 remains
active for payload variants/sealed dispatch, the catalog/generic boundary work
and shared fact consumers. No fleet or full-library rerun was performed.
