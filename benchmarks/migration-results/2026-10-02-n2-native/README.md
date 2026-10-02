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

## Call presence and managed references

Algorithm78 implements callback ABI 3 with a supplied count and a borrowed
per-position absence bitmap. Defaults execute in the selected callee, including
absence before later supplied arguments, indirect/generic calls, constructors
and super. Ordinary scalar arguments remain unboxed. Legal synchronous mutable
references now support managed and callable payloads without acquiring ownership
of the caller's slot. Captured bare declarations allocate their cell before
assignment; assigned captures preserve ownership and initialization checks.
The shared source prohibition on captured/suspending reference parameters remains
in force; it does not require a native escaping-reference ABI.

Four final affected fixtures pass **28** source/build/run steps under strict GCC,
Clang 18 and ASan/UBSan: call transport and arrays on pin `n2-call-transport-4`,
products and exceptions on pin3. Independent ECMAScript oracles cover defaults
past parameter 64, effect order, generic adapter defaults, alias writes, callable
references, nullable findIndex elements and deleted positions in both absence
policies. Repeated calls check bounded allocation and zero owners at completion.
Pin4 only extends the findIndex fix to legacy defaults and explicit nullable null.

The extracted default-argument subset also matches emitted JavaScript at effort
0 and 13; the complete source passes native GCC at effort13. The full source's
JavaScript formation is refused with `reference callable requires a complete
private interface`; V1 owns that gap. Do not count the extracted subset as full
source parity. The exact extraction driver, sources, oracle, logs and receipts
are [retained](call-transport). Initial build and fixture failures are retained:
Rust2021 does not admit let-chains; callback parameter types must match the
array's nullable element type. Neither is presented as a passing initial run.
No broad library rerun or performance claim. Updated Rust expectations await
N2's consolidated check; callable containers, static data, suspension, native
interfaces and performance qualification remain open.

## Callable containers

Algorithm79, pin `n2-callable-1`, adds a paid native representation-conversion
demand owner. Actual transfers, emitted array descriptors and tagged callable
producers/readers request physical signature bridges; unrelated program
signatures do not form an unconditional all-pairs adapter matrix. Generic arrays
keep their shared identity and convert both reads and writes. Products, shapes,
records, nested callable arguments and returned closures use the same bridge
recipes. Statement temporaries own new adapters until their consumer retains
them; reverse views recover the original callable and preserve identity.
Checked call-argument conversion completes before user code is invoked.

The new independent ECMAScript fixture and existing call-transport fixture pass
**14** focused source/build/run steps under strict GCC, Clang18 and ASan/UBSan.
The full new source also passes all **3** native effort13 build/run steps.
Checks include shared arrays, function identity, generic callback fields, value
copies, managed captures, nested callback bridges and repeated reverse views
with bounded owner counts and zero owners at completion. No full library suite
or performance claim. [Evidence and reproduction driver](callable-containers).

JavaScript formation refuses the same source at effort0 and13 with `erased
product escapes a closed typed interface` at the generic `getMapper` callback
field read. Both failed receipts are retained and V1 owns the gap. Native
qualification uses the independent ECMAScript oracle; it is not claimed as
full emitted-JavaScript parity. N2's remaining static data, safe representation
failures, suspension, native interfaces and performance/cross-target work remain
open.

## Checked representation failures

Algorithm80, pin `n2-views-4`, checks allocation witnesses before tagged values
are interpreted as classes, records, maps, regexes, buffers or typed arrays.
Typed views preserve the stored element width; indexed unions check the tag
before interpreting their payload as a pointer. Direct class downcasts and
callable parameter/result downcasts use the same witness checks. Proven upcasts
and compatible physical callable views retain their direct representation.

Callback and constructor arguments finish converting before invocation. Native
conversion failure facts propagate through the existing common call graph,
keeping necessary checks through direct calls, returned values, selected region
results and structured completions. Ordinary calls with no fallible conversion
do not acquire an extra check before invocation. Runtime failures preserve the
source precondition's memory-safety guarantee; this is not full recursive schema
validation and does not promise ECMAScript parity for ill-typed assumptions.

The final focused batch passes **38** source/build/run steps: 27 under GCC,
Clang18 and ASan/UBSan across the new representation fixture, existing call
transport, exceptions and standalone ownership runtime; 11 additional production
and plain-binary steps. The misuse fixture covers 25 rejected representations,
valid class/callback downcasts, bounded failure cleanup and zero remaining owners.
Malformed array elements never enter callbacks. Production and sanitizer output
agree with the explicit native expectations. Existing call/exception fixtures
retain their independent valid-program oracles. Plain Uint8Array code compiles
without the tagged-value runtime. [Receipts, logs and driver](representation-views).

The initial Rust module-visibility error and fixture's un-narrowed optional
string refusal are retained; the corrected source explicitly supplies an absent
name fallback. No full library rerun or native performance win is claimed.
Immutable graphs, suspension, remaining native interfaces, full capability and
performance/cross-target qualification remain N2 work.

## Immutable static graphs

Algorithm81, pin `n2-static-2`, gives checked once-created immutable graphs
static physical storage. Array/object aliases, imported constants, record key
order, generic product snapshots, inheritance, UTF-16 literals, absent values,
signed zero, non-finite values and subnormals use the same physical runtime
views. Source initialization points remain in order. Repeated function and loop
activations keep fresh identities. Static owners bypass retaining, destruction
and cycle scans. A garbage cycle may refer to static data without retaining
the cycle or modifying the static header. The fingerprinted
`target.native.static_data=false` selects ordinary runtime initialization.

The shared const checker admits typed read-only readers and mutable outer
copies from slice/concat/Object.values; shared const children remain protected.
This is a provenance correction, not permission to remove host calls.

**29 native/oracle source/build/run steps** pass across static and owned modes,
efforts0/13, GCC/Clang18 and ASan/UBSan, including standalone ownership.
**Five** finite-variant JavaScript/oracle steps and **three** const-child mutation
refusals also pass. Four CLI policy receipts confirm the flag changes native
identity and leaves JavaScript identity unchanged. Full-source JavaScript
formation refuses mandatory non-finite literals at both efforts; V1 owns that
gap. The finite variant changes only Infinity/NaN to2.5/3.5 and is not claimed
as full-source parity. [Drivers, logs, receipts and exact sizes](static-data).

| Effort | Static storage | Startup owners | GCC executable bytes |
|---|---|---|---|
| 0 | off | 18 | 110624 |
| 0 | on | 0 | 128440 |
| 13 | off | 16 | 105944 |
| 13 | on | 0 | 127144 |

The executable growth is a real tradeoff in this fixture. Startup allocation
is measured; runtime speed and broad performance are not. Initial Rust borrow,
fixture syntax/type and driver failures are retained separately from final
passes. No broad suite rerun. Consolidated Rust/native corpus checks, suspension,
native interfaces and performance/cross-target work remain N2.

## Generator frames and iterable completions

Algorithm82, compiler pin `n2-generators-1`; source and compiler identity are
retained under `generators/native/qualification.json`. Fifteen main-fixture
steps pass across efforts0/13, strict GCC, Clang18 and combined ASan/UBSan.
Sixteen supplemental steps qualify saved throws across a yield in finally,
return/early-close overrides, unstarted frames, Set cursor mutation through
rehashing, and the existing exception fixture. All native traces match the
independent ECMAScript observations; ownership returns to zero after collection.

The emitter stores typed source slots in retained frames and dispatches to
compiled resume labels over the existing regions. Generator parameters/defaults
execute at call time and bodies stay lazy. Iteration retains its source; early
exit closes it through shared completion cleanup. Finally may yield while
closing, and a preexisting throw takes precedence over a close failure. A Set
cursor pins entry positions while deletion, clear and appending retain source
order. GC never invokes source finalizers.

The full emitted JavaScript trace differs only in the default-timing prefix:
its body guards delay nonliteral defaults and their exceptions until iteration.
`generators/javascript/qualification.json` records both effort0/13 mismatches;
V1 owns that correction. This batch claims independent native correctness, not
full source parity. Initial fixture parse and stale provider-configuration
errors are retained. No broad library suite, speed or executable-size win is
claimed; frame and helper overhead require the final N2 profile qualification.

## Async frames and task queue

Algorithm83, pin `n2-tasks-3`, passes all46 steps in
`tasks/qualified/qualification.json`. The two full fixtures pass independent
ECMAScript, strict GCC and Clang18, ASan/UBSan and emitted JavaScript at efforts0
and13. The separate unhandled-rejection fixture fails with status1 after queued
work on both runtimes, while native shutdown leaves zero owners. The existing
generator completion fixture also passes the shared frame writer under sanitizers.

Frames retain typed parameters, locals, captures, prepared callees and displaced
finally completions. Async entry runs synchronously, await always queues, and
continuation/task adoption follows FIFO promise ordering. Finally preserves the
additional jobs of a returned task; Task.all preserves input order. Queued jobs
own their context/payload; task observers are traced edges. Repeated handled
rejections leave the unhandled list immediately. Tests cover managed product
snapshots/shared children, callable results, async methods, iterator cleanup
across await, generic and union adoption, self-resolution rejection and unreachable
mutual-adoption cycles. A host atexit assertion observes zero owners after normal
and exceptional shutdown.

Shared task types now describe settled values, canonicalizing nested task, union
and optional resolutions after generic substitution. Actual argument constraints
can supersede ambiguous expected Task<T> hints. The source body still checks its
declared inner return type; native widens a differing physical return only when
resolution requires adoption. Task<void>.then accepts a zero-argument callback.
The corrected type contracts agree on both output targets.

The initial contract fixture exposed the shared contextual-inference conflict;
its logs and the preceding syntax correction are retained. Generated C/header
hashes are retained instead of duplicating runtime sources. No full Rust/library
suite or performance claim; the final N2 capability/profile qualification remains.
