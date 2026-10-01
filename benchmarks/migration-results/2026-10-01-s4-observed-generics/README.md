# S4 observed generic classes and callable contracts

1. Erase class type parameters without replacing an observed constructor's
   identity. Instantiate constructor/rest/super contracts through the shared
   checked binder substitution, including verification of edited IR.
2. Make JavaScript constructor/prototype/default/receiver handling consume the
   same callable signature for ordinary and generic functions. Preserve public
   arity and host identity; never infer a concrete runtime type from a binder.
3. Limit the existing opaque-product proof to products actually substituted
   for generic binders. A fixed struct parameter does not become opaque merely
   because its function has another type parameter. Keep unqualified product
   erasure refused, with its wider representation owner still explicit.

Prediction: supported observed generic classes become one ordinary JavaScript
class, independent of instantiations; no per-type constructor wrappers. Existing
non-generic output should stay unchanged. Run focused identity, constructor
verification, generic value-copy and public-route checks after implementation.
No full library or fleet rerun; broader opaque transport remains S4 work.

## Result

Implemented. Ordinary and generic constructors share the checked signature
owner; defaults, rest, inheritance and implicit constructors preserve one runtime
identity. Fixed struct parameters keep their schema even when other parameters
are generic. Constructor values crossing to JavaScript retain constructibility.
One-module builds now retain the canonical checked export interface instead of
reconstructing dual type/constructor exports from syntax. Native class identity
uses the existing immutable destructor identity, with no object-header increase;
checked casts reuse ownership-preserving native conversions.

Pin: `/home/azureuser/lilscript-work/bin/s4-observed-generics-1/lilscript`, SHA-256
`45512001bcf799d2726a576cd77c9184675b3a5a4f2be90e5443d1d40af0d3b2`.
Algorithm 26 / search 31 / walk 10 / local facts 10. Source/codec identities are
in [identity.json](identity.json) and [comparison.json](comparison.json).

Nine distinct focused checks pass ([checks.json](checks.json)). The native
identity/cast oracle runs Node plus seven GCC/Clang/UBSan/ASan profiles, with
artifacts and commands retained in [native-artifact.json](native-artifact.json).
CLI, lint and LSP release builds pass. The matched class boundary stays at
243 raw / 173 gzip / 139 Brotli. The two new generic capability oracles pass each
objective, and every scored artifact matches the delivered bytes. There is no
before-size claim for formerly refused programs, fleet claim or timing study.

This is a semantic/capability correction, with no new optimization flag or effort
gate. Opaque generic products and wider union/host transport remain S4 work;
complete native capability qualification remains N2. Detached primitive method
values are already refused by the checker with an explicit-closure alternative;
the carried `charCodeAt` audit passes and needs no implicit receiver binding.
