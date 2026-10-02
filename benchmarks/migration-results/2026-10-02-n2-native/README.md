# N2 native continuation evidence

Base: N1 `62856e0c`. [Acceptance](../../../docs/migration/n2-acceptance.md).
N2 remains active. This record does not declare native completion.

## Runtime ownership batch

Algorithm74, final runtime compiler pin 5 in `runtime-identity.json`.
The C runtime now owns retained UTF-16 strings, views, managed nongeneric
products and traced cycles. Real reference counts remain intact during cycle
discovery; white objects lose their edges before any white object is freed.
Intrusive traversal/destruction lists avoid recursion through deep graphs.
The TOML collection threshold defaults to 4096 allocations and scales with
live size; zero means explicit/shutdown collection only. Zero-count objects
are still reclaimed immediately.

Callback/string ABI 2 records owner-bearing strings. The header exposes a
copy from temporary host UTF-16 storage. Captured boxes exist on lexical
region entry and gain initialized payloads at their original initialization
sites. Recursive closures can consequently retain their own boxes and the
collector reclaims the resulting cycles. This changes the physical ABI;
providers must regenerate their headers.

The standalone production-runtime test passes GCC, Clang 18 and combined
ASan/UBSan, all under `-Wall -Wextra -Werror`. It checks duplicate edges,
rooted and unrooted cycles, rooted descendants, 100,001-node graphs, string
view lifetimes, temporary joins, self-owning maps and typed buffer views.
`runtime-qualified/qualification.json` retains those executions. Subsequent
changes added unused annotations and a host string-copy API; the collector
algorithm did not change.

`runtime-closures-final/qualification.json` records the final generated-source
gate under the same three compiler profiles. It covers strings across loops,
views after replacement, arrays, managed product copies, callable fields,
captured callbacks, self-recursion, host-returned copied strings, and class
cycles. Explicit collections must leave zero live objects after each group.
All runs match the independent fixed output and pass leak/undefined checks.
The final gate rechecks generated code only; the already passing standalone
runtime matrix is not repeatedly rerun.

`runtime-cases.json` records eight passing formation-only/production C cells,
including the formerly ledgered self-recursive closure. Its ledger entry is
removed. The generated configuration schema also matches and documents every
accepted key. Initial compilation failures, including an emitter Rust borrow
and strict-C unused-declaration warnings, are retained beside their corrections.
No broad Rust/library suite was rerun; changed Rust test expectations will be
compiled in the consolidated N2 test batch. No executable-speed or compression
win is claimed from this ownership gate.

Generic/tagged product layouts, remaining portable data, exceptions/suspension,
Unicode/regex, native library/host interfaces, cross profiles and full native
performance/corpus qualification remain within N2.

## Portable records and shapes

`data/qualification.json` records the next coherent batch, compiled with pin
`/home/azureuser/lilscript-work/bin/n2-data-2/lilscript`. All seven generated-source,
GCC, Clang 18 and ASan/UBSan steps pass under strict C11 warning-as-error flags.
The maintained `tests/native/data.oracle.mjs` independently computes ECMAScript
output; `data/oracle.json` records its identity alongside the LilScript fixture.
Every native run must also finish with zero live objects after explicit collection.

This batch implements string-keyed records using the traced hash table, numeric
own-key ordering, Object keys/values/hasOwn/assign, checked scalar/array/record JSON
stringification and scalar array join through one UTF-16 builder. Declared shapes
share record ownership while preserving schema-selected fields, spread snapshots,
optional-key writes, intersections and discriminant tests. `__proto__` is an ordinary
own record key; replacement preserves insertion order. Tests include UTF-16 lone
surrogates, control escaping, boundary integer keys, aliases and recursive shapes.

The focused maintained runner passes six formation-only/production C cells.
Only those three verified case IDs were removed from the multi-case native mask.
No library suite or fleet benchmark ran. Updated Rust capability expectations
remain for the consolidated N2 Rust batch. Generic/product payloads and typed JSON
parsing remain open; this is implementation evidence for a batch, not N2 closure.

## Tagged and generic products

`products/qualification.json` and `product-places/qualification.json` record two
focused fixtures on `/home/azureuser/lilscript-work/bin/n2-products-1/lilscript`.
All 14 source/build/run steps pass GCC, Clang 18 and ASan/UBSan, with strict warnings.
`products.oracle.mjs` supplies an independent value-copy model. The 2,000-iteration
loop must stay below 40 live allocations, every group returns to zero owners, and
200 shape/product cycles must be collected. Managed return conversion through a
generic callback adapter, record/map/array payloads, nullable/union returns and
nested generic field updates are included.

`product-places.lil` separately agrees with emitted JavaScript on a right-hand side
that reallocates/replaces its selected array element or replaces a shape field.
The native writer uses logical-location writeback and preserves copy independence.
The complete products fixture has a JavaScript formation refusal at Object.values
of a product record, now explicitly carried by V1; its native output matches the
independent oracle. This is not claimed as full cross-target fixture parity.

Products keep direct C value layouts. Tagged transport uses traced immutable
snapshots and statement-scoped temporary owners, with explicit call/adaptor
ownership conversion. Generic fields use the checked tagged ABI. Polymorphic
array fields/calls and hot specialization remain N2 work, along with the existing
control/runtime/interface/performance commitments. No broad library suite ran.

## Polymorphic arrays

Pin `n2-arrays-3` completes shared-identity typed/generic array storage, generic
array fields, owned callback arguments/results/accumulators, and sparse copies.
The independent `tests/native/arrays.oracle.mjs` covers aliases across generic
functions/classes/products, nested arrays, mutation, managed reductions, sparse
map results and bounded per-iteration product boxing. The shared checked-read
lowering now keeps the optional representation at the load: coercing a sparse
integer read first lost absence on native. The prior JavaScript output preserves
absence; this corrects native parity rather than changing the source contract.

The [five-fixture batch](arrays) passes all 35 source/build/run cells (GCC,
Clang 18, and Clang ASan/UBSan) across arrays, products, product-places, data and
ownership. Live-object assertions return to zero; 1,000 product elements stay
below the temporary-allocation bound. Initial mismatch receipts and all build
logs are retained. No broad suite or performance comparison was run. The full
arrays source has a separate generic-class-field JavaScript formation refusal,
owned by V1; the independent oracle is not represented as full source parity.

## Exception completions

Algorithm75, pin `n2-exceptions-6`, implements status propagation and structured
try/catch/finally completion routing with deterministic ownership cleanup. Nested
finally regions preserve displaced returns and throws, and route break/continue
through crossed finalizers. Array callbacks and callback adapters propagate
failure without converting or retaining an invalid result. Checked representation
and bounds failures become catchable errors. Providers can query, take or raise
an owned tagged exception through the additive ABI 2 status interface. Shared
throw-free effects omit call checks only with dead-code-elimination permission.

The [six-fixture batch](exceptions) passes 48 source/build/run cells under GCC,
Clang 18 and ASan/UBSan, including the standalone ownership runtime. The new
independent ECMAScript oracle covers nested completion replacement, managed
payloads and captured catch bindings, callback failure, provider handling and
bounded allocation loops; emitted JavaScript agrees too. Bounds oracles use the
explicit development-checks profile. Two maintained throwing-call C cells pass
in formation and production. No full library suite or performance claim.

Initial strict GCC warnings are retained: string indexing now proves bounds
before pointer arithmetic, and failed converted-call results receive a typed
scratch sentinel before propagation. Source fallthrough semantics are unchanged.
The lexical exceptions mask is removed. Other corpus masks remain conservative;
portable tagged transport does not implement JavaScript host object operations.
Remaining ordinary runtime errors, suspension, text, data and interfaces stay N2.

## Unicode and typed regular expressions

Algorithm76, final pin `n2-text-6`, implements Unicode 17 case conversion and
pinned standalone QuickJS libregexp with owned UTF-16 state. Source/flags,
lastIndex, test/search/replacement, named/numeric captures, Unicode properties
and sets, empty matches and catchable syntax/resource errors are covered.
The original MIT sources, manifest and deterministic amalgamation script are
maintained. TOML stack/poll controls document their defaults and precise limits;
short matches can finish without an engine poll. Ordinary strings do not include
the Unicode tables unless a case operation or regex requires them.

[Retained evidence](text) records 35 focused source/build/run cells across text,
limits, exceptions, arrays and products. GCC, Clang 18 and ASan/UBSan pass, including
allocation bounds and zero live owners. The complete text fixture also matches
emitted JavaScript on Node 24.20.0. Eight maintained formation/production C/JS
cells pass; four JavaScript-host cells remain explicitly masked on native.
These do not constitute the full native corpus or broad library qualification.

The independent Unicode 17 oracle agrees byte for byte on **2,228,224** isolated
lower/upper conversions, including all code points and surrogate units. Its
10,879,214-byte stream has SHA-256
`b305dfa00386e88ac44b8444b141b6146393e0661fa4630911e455a5ed4b2473`.
Contextual sigma, expansions and mixed surrogate sequences are additionally
covered by the ordinary text fixture. The exhaustive run uses pin5; pin6 only
changes regex source serialization and JavaScript regex-literal serialization.

Initial failures are retained: two upstream symbols required private linkage/
namespace handling in amalgamation; ASan's fake-stack locals required actual
frame addresses for the parser limit; GCC's one-minute compile limit exposed
repeated exception cleanup. Sharing one failure exit per lexical region shrinks
identical fixture C from 2,673,298 to 664,938 bytes. The revised fixture builds
under all profiles. No executable-size or runtime-performance win is claimed.
The runner now terminates timed-out compiler process groups, including children.

The JavaScript parity check also found an escaped-line-terminator regex literal
that failed artifact admission. Literal serialization now preserves legacy
constructor meaning and retains invalid Unicode constructors so they still throw
at runtime. The new fixture independently checks both outcomes. Updated Rust
expectations remain for the consolidated N2 Rust batch. N2 still owns its
remaining data, suspension, host ABI and performance/cross-target commitments.

## JSON and binary64 decimal conversion

Algorithm77, pin `n2-json-4`, adds strict owned JSON parsing into the existing
tagged arrays/records/scalars, float JSON output and locale-independent shortest
binary64 printing. JSON parsing uses a heap-backed frame stack, preserves UTF-16
escapes, duplicate-key ordering, signed zero and overflow/underflow, and releases
partial values on catchable syntax failures. Typed views retain the existing
`JS.assume` precondition; this does not add arbitrary schema validation or host
prototype operations. The Unicode/regex/numeric recipes now share one private
C utility amalgamation; Unicode tables remain conditional.

The [five-fixture batch](json/final-qualification.json) passes **41** focused
source/build/run steps under GCC, Clang 18 and ASan/UBSan, including the standalone
ownership runtime. Fixtures cover JSON, existing data, arrays, text and ownership.
The JSON fixture matches independently computed ECMAScript output and emitted
JavaScript, handles nesting depth 10,000, checks malformed input cleanup, and
finishes with zero owned objects. No full library/case matrix was rerun.

The numeric oracle compares **248,109** observations: every binary exponent's
transition values and 100,000 random bit patterns for formatting, 100,000 random
decimal tokens for parsing, and **39,918** exact rounding midpoints and adjacent
decimal tokens constructed independently with Python Decimal. Native GCC and
Clang ASan/UBSan match Node 24.20.0 byte for byte; the sanitizer executable also
matches under the independently generated `de_DE.UTF-8` comma-decimal locale.
The stream is **4,908,206 bytes**, SHA-256
`9ee3ef5c6e51f09165a30612129c607e240fc2f8a399faebe8dd956bfbc122fb`.

The new oracles exposed the pinned parser's intentional 38-digit truncation and
its formatter's failure to consider an adjacent decimal in a power of two's
asymmetric rounding interval. The parser adapter compares a sticky discarded
suffix against the exact next binary64 midpoint using bounded integer scratch.
The amalgamation corrects the printer's adjacent-candidate search and defers
trailing-zero removal until after validation. Vendor originals stay unchanged.
An initial adapter carry-normalization bug was caught by the same boundary
oracle and corrected; [mismatch counts](json/numeric-corrections.json) retain the
progression instead of presenting the initial results as passes.

Reproduction uses `tests/native/qualify-ownership.mjs` for each fixture. Generate
numeric input with `python3 tests/native/json-midpoints.py`; pipe it to
`json-numbers.oracle.mjs` under Node and to `json-numbers.c` compiled beside the
generated JSON fixture's `ownership.c`/header and host provider. The retained
[driver](json/qualification-driver.py) records the pinned local paths, GCC/locale
commands, affected fixtures and JavaScript parity check. The numeric sanitizer
build uses Clang 18, `-std=c11 -O2 -Wall -Wextra -Werror`,
`-DLS_NATIVE_QUALIFICATION -g -fsanitize=address,undefined
-fno-omit-frame-pointer`, the generated fixture include directory, and `-lm`.
ASan leak detection and UBSan halt-on-error are enabled. Compiler/fixture hashes
and build/failure logs are retained. Updated Rust expectations await N2's
consolidated Rust check. N2 remains active; no native speed or executable-size
win is claimed for this semantic batch.
