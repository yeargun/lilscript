# N2 native completion

N2 is complete after N1 (`62856e0c`) under the [retained qualification](../../benchmarks/migration-results/2026-10-02-n2-native/README.md). Its dependencies
were completed in the order below before V1. This is one milestone with implementation batches,
not permission to mark portable omissions complete by refusing them.

1. **Runtime ownership and ABI.** Move runtime recipes into C source/header
   files with declared dependencies and one native symbol namespace owner.
   Implement managed product layouts, reference-counted UTF-16 strings (ABI 2),
   traced retained objects and synchronous trial-deletion cycle collection at
   allocation thresholds. Preserve literal sharing, borrowed call arguments,
   escaping owners, final-use transfers and deterministic cleanup. Document
   configurable thresholds/profiles and their memory/CPU tradeoffs.
2. **Portable data.** Complete concrete generic/managed products, nullable and
   tagged product transport, shapes, records, Object keys/values/assign, typed
   JSON and immutable graph storage. Preserve value copies, identity, absence,
   key ordering, defaults, mutation and effects. Remove the corresponding native
   capability refusals only with implemented recipes and focused evidence.
3. **Control and text.** Implement exception status propagation through calls
   and Try/catch/finally, with shared throw-free effects removing needless
   checks. Implement generators and async/tasks as region state machines with
   an owned microtask queue. Vendor pinned MIT QuickJS libregexp and Unicode
   support; retain its license and source identity, and compare Unicode/regex
   behavior with an independent JavaScript runtime.
4. **Native interfaces.** Complete explicit extern providers and C library
   exports with generated headers, stable allocated symbols and documented
   ownership. Supply typed argv/env/standard streams/files/clock/exit bindings.
   Default/absent and mutable-reference callable transport must follow the
   language contract, not acquire a different native meaning.
   Capturing a `ref` parameter is a shared source error, as are suspension and
   public opaque reference ABIs; N2 preserves that contract. It must support
   managed/callable payloads through legal synchronous references, rather than
   adding escaping reference ownership and heap allocation to ordinary calls.
5. **Optimization and profiles.** Native speed/size/balanced controls consume
   the shared optimized program. Qualify hot generic specialization, borrowed
   closures, ownership/stack storage and exception-check omission on portable
   workloads. Implement and exercise a cross-target profile including
   wasm32-wasi. Keep strict arithmetic and independent GCC/Clang oracles.
6. **Closure.** Run coherent focused batches after substantial changes; run
   the complete native capability/runtime and sanitizer matrices once their
   prerequisites exist. Account for every N1 masked/refused case as supported,
   missing test-provider setup, obsolete fixture configuration (V1), or a
   declared JavaScript-only facility. Qualify portable performance against the
   JavaScript output, including the named parser/algorithm cores. Update
   matrix, coverage, history and evidence, then commit/push before V1.

Native performance and JavaScript compression are separate measured outcomes.
No runtime interpreter fallback, erased source semantics, workload-name tuning,
blanket test masks or unmeasured compiler-flag win satisfies these gates.

The closure audit found that array `indexOf`/`includes` had accidentally admitted
value-struct identity observations, although struct equality, Map keys and Set
elements already have no such portable contract. The shared checker now applies
that existing identity rule to array searches, including unconstrained generic
parameters. Explicit field predicates remain supported through `findIndex`.
This resolves an inconsistent language boundary; it is not a native size or
coverage win and does not introduce a structural equality contract.
