# Coverage matrix

Parent: [verification](README.md). Migration:
[00–06 standing / 07 current](../migration/README.md).

This is the ownership ledger. The executable report should eventually replace the
status column; until then `required` means a family must own at least one semantic
case, one size case, and boundary/config variants where applicable.

| Area | Required subfamilies | Strongest oracle | Strict win expected somewhere? |
|---|---|---|---|
| Integers | overflow, zero div/rem, shifts, bitwise, updates | AST evaluator + JS/C/native | yes |
| Numbers | NaN, -0, infinities, comparison, coercion | tagged value/trace | case-dependent |
| Strings/regex | literals, templates, escapes, methods, pooling | exact value/throw | yes |
| Null/union | narrowing, defaults, truthiness, tags | evaluator/value | yes |
| Bindings | scope, shadow, globals, dead stores | evaluator/trace | yes |
| Control flow | branch, loops, phi, match/switch, early exits | evaluator/trace | yes |
| Functions | direct/recursive/defaults/arity/construction | value + API descriptors | yes |
| Closures | captures, identity, factories, escape | trace/identity | yes |
| Effects | pure/impure/host/evaluation order | ordered trace | yes |
| Errors/async | throw/catch/finally/tasks/generators | ordered settle/cleanup trace | parity first |
| Struct/class | layout, mutation, methods, escape, inheritance | value/API/identity | yes |
| Records/objects | dynamic keys, enumeration, prototype, JSON | key/value/descriptors | parity first |
| Collections | arrays/maps/sets/typed buffers, mutation/aliasing | value/identity/trace | yes |
| Modules | static graph, exports, cycles, DCE | artifact + behavior | yes |
| Lazy/chunks | dynamic import, shared/preload/cycles | artifact set + network trace | case-dependent |
| Host/browser | DOM/events/network/storage/workers | browser snapshot/trace | case-dependent |
| Public ABI | ESM, script tag, names, fields, `new` | API descriptors + behavior | parity first |

## Cross-products

Every row need not multiply by every config. Use pairwise covering arrays for broad
config interaction, then hand-own high-risk triples: optimization × compression
decision × search feature; public ABI × mangle × bundle mode; codec × priority × raw
growth; and lazy boundary × preload × chunk limit.

## Structural complexity axis

Semantic coverage and structural coverage are reported separately. The micro suite
owns local rules and minimized regressions. The
[algorithm challenge lane](algorithm-challenges.md) owns small (3–7 functions),
medium (8–19), and large (20+) programs, with module count, boundary count,
call-graph depth, runtime-vector count, size band, and opportunity tags recorded for
every case. Each tier must cover propagation/DCE, mangling, inline-vs-sharing,
dictionary/order, aggregate/collection representation, and control-flow interactions;
module and host rows join once their fair baseline frontiers exist.

## Count policy

Report all of:

- unique semantic templates;
- generated parameter variants;
- boundaries/config variants;
- total executions.

“500+ tests” should mean at least 500 maintained case instances, but coverage
completion is based on this matrix, not the headline count.
