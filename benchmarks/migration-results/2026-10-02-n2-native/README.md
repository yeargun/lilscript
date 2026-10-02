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

Generic/tagged product layouts, portable records/data, exceptions/suspension,
Unicode/regex, native library/host interfaces, cross profiles and full native
performance/corpus qualification remain within N2.
