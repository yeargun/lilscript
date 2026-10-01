# S4 disjoint public product unions

Add public codecs for a value struct combined with distinct string, number or
boolean alternatives. The public object and private product are distinguished
from primitives by category tests, without a field read or runtime type argument.
Nullable outer values retain the existing absence codec. Checked private union
injection and narrowing preserve the same product recipe; `JsValue` conversion,
assumptions and invocation-only callback adapters use the shared public codec.

Private generic transport admits representation-independent primitive category
tests. Array tests, class identity hooks and equality cannot observe a product's
private backing; ambiguous public object/array codecs remain refused. This is
not permission to erase a product through an opaque union member.

Prediction: existing scalar-union code is unchanged; these new product-union
capabilities add only the codecs their ABI requires. Development checks validate
the selected member and snapshot valid fields once. Runtime correctness is
checked at effort 0/13 under all objectives and hard optimization vetoes, followed
by focused opaque-transfer regressions and a matched artifact control.

## Evidence

Three focused tests pass, covering effort 0/13, all three objectives, hard
optimization vetoes, callbacks, generic transport, nullable values and development
validation. Three existing opaque-boundary/refinement regressions also pass.
The release CLI build and all independent artifact oracles pass. No full-library
or fleet rerun was performed.

The matched scalar-union control is byte-identical: 76 raw / 94 gzip / 66 Brotli.
The new product-union capability measures 1,087 raw / 420 gzip / 377 Brotli under
its independently selected objectives; it has no old-compiler size-win claim.
Scored and delivered bytes agree. See [comparison.json](comparison.json) and
[identity.json](identity.json) for source/config/artifact hashes and search work.

Pin `s4-product-unions-1`, SHA-256
`c1034897a2200225478fdbce609ee7f94c8b50d7aab621241b9c686b4d274ace`.
Algorithm 36 / search 32 / walk 10 / local facts 14. S4 remains active for wider
opaque ABI contracts and the shared fact-consumer/removal audit. No timing,
runtime-parity, native-completion, ratchet-baseline or port-artifact claim.
