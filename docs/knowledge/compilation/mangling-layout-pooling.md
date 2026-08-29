# Mangling, layout, and literal pooling

Parent: [compilation](README.md). Emission detail:
[JavaScript emission](javascript-emission.md). Config: [`[mangle]`](../config/mangle.md)
and [compression decisions](../config/compression-decisions.md).

## Names

Identifier mangling is scope-aware and frequency-ranked. Referenced globals, host ABI
names, extern members, and reserved words remain unavailable. Property mangling is
limited to owned LilScript fields; record keys and host properties are data/ABI.
Export mangling is a separate closed-app opt-in.

Entropy-aware candidates vary the identifier alphabet, one-character assignments,
cross-scope reuse, and stable local affinity. They are re-emitted through the real
mangler; a character-frequency proxy never becomes the winner by itself.

## Order and shape

Function layout candidates include source, similarity, and codec-window-aware order.
Exact Held-Karp ordering is bounded by `function_layout_exact_limit`; larger groups
use deterministic heuristics. Local-name reservation encourages repeated functions to
reuse the same short spellings. Aggregate layout competes only when escape/public ABI
permits both representations.

## Literals

String pooling, numeric pooling, packed string arrays, quote styles, booleans, and
regex literals all add syntax as well as remove repetition. The emitter uses a cheap
profitability filter, then candidate search measures the complete artifact under raw,
gzip, or Brotli. Pooling is not assumed to beat the codec's own dictionary.

Exact `javascript.compression` controls whether a representation is legal;
`javascript.optimizations`/level controls whether alternatives are searched; explicit
`[mangle]` flags have highest precedence. Search may retain the unpooled/unmangled
alternative. Most omitted non-search-only tactics stay off, but size-first
search-only overlays may still compete. `length-to-number-elision` stays off
when omitted. See the
[decision registry](decision-registry.md#javascriptpriority-vs-cost_model).

## Dense string-return tables

With candidate search and `dense-string-return-tables`, the emitter may replace a
return-only equality guard ladder with `[...][selector]`. This is legal only when all
arms compare the same `int` selector with distinct integer constants, every return is
a JSON-decodable constant string, and integer analysis proves the complete selector
domain is exactly zero-based and bounded (`0..max`, at most 256 entries). Unmentioned
domain points are filled with the original default string.

The proof renders the pure selector prefix exactly once and rejects leftover work,
phis, extra reachable blocks, duplicate keys, negative/out-of-range arms, an
unavailable selector, or an unbounded/non-zero-based range. Because every proven
runtime index addresses an own array element, the representation does not depend on
a pristine `Array.prototype` and cannot change the ladder's default behavior. The
ordinary ladder remains the configured baseline. Search scores the full table ×
pure-helper-policy Cartesian family before pruning so a joint dictionary win is not
lost to an individually worse intermediate.
