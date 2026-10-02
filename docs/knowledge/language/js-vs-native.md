# JavaScript vs native

Parent: [Language](README.md). Related: [mission](../mission.md),
[current architecture](../compilation/current-architecture.md),
[native support](../../native-support.md), [configuration](../../configuration.md).

`--target all` shares parsing, checking and target-neutral Program rules.
Folding, liveness, inlining, scalar replacement and ownership facts feed both
targets. JavaScript then chooses representations and names against its selected
raw/gzip/Brotli objective. Native admits concrete storage and emits strict C11.
Neither target may change arithmetic, identity, effects or source lifetimes to
make a representation cheaper.

| Facility | JavaScript | Native |
|---|---|---|
| Scalars, UTF-16 strings, generic products, arrays and collections | JavaScript values and checked source operations | Owned layouts, typed views and tagged boundaries |
| Records, shapes, Object operations and typed JSON | Source-owned objects and selected encodings | Owned records/arrays, checked representation views and exact decimal conversion |
| Exceptions | Structured throw/catch/finally | Owned status with lexical cleanup and completion preservation |
| Generators and async/tasks | Generators and Promise jobs | Typed frames, iterator completion and an owned FIFO microtask queue |
| Typed Regex operations and Unicode casing | Engine operations | Pinned QuickJS libregexp and Unicode 17 tables; no interpreter fallback |
| Extern functions and public libraries | Explicit JavaScript host/module ABI | Explicit function providers and namespaced generated C interfaces |
| Host-specific JsValue operations, extern JS classes, dynamic import and JS reflection | JavaScript host contract | Source-qualified diagnostic |

Portable tagged values and `JS.assume` views are supported by native; their
presence alone is not a JavaScript-only classification. Arbitrary JS host
objects and regex match objects still require a JavaScript host. The detailed
[capability inventory](../../native-support.md) states the representation and
interface boundaries; admission of a family does not admit every combination.

`[target.native]` controls compiler, strict speed/size/balanced flags, artifacts,
SDK/triple, WebAssembly stack reservation, diagnostics and runtime resource
limits. It also controls immutable static data, proven closure stack storage
and closed generic specialization. Each control documents its compile-time,
artifact-size and runtime tradeoff. Disabling an optimization keeps the general
representation; enabling it never bypasses its proof. The retired top-level
`[native]` table remains ineffective.

JavaScript compression and native execution are measured separately. Native
qualification includes GCC/Clang, sanitizers, independent JavaScript oracles
and an executed WASI profile. A C flag or a smaller C source file is not itself
evidence of a smaller executable or faster execution. The migration evidence
records the measured workloads and the limits of each comparison.
