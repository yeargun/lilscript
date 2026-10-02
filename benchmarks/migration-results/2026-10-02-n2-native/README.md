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
