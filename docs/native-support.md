# Native capability and ABI matrix

Native consumes the same checked, semantically optimized program as JavaScript.
It has a C11 storage/call plan and writer, not a second source optimizer.
`--check --target native` and compilation use that same admission plan. A
JavaScript-only check makes no assertion about native support. Source refusals
retain their original module and span, including after shared rewriting.

The machine-readable inventory is `native_capabilities` in native policy/build
reports, generated from `src/native_capabilities.rs`. Its states deliberately
distinguish remaining native work from facilities tied to a JavaScript host.

| Family | Current boundary | Remaining owner |
|---|---|---|
| Scalars and control flow | Wrapping int32, strict binary64, bool, UTF-16, null, scalar unions, structured branches/loops | Further runtime qualification: N2 |
| Semantic optimization | Shared folding, liveness, calls/inlining, aggregate scalar replacement, and proven final-use ownership transfer; the requested tactic vetoes apply in native and `all` builds | Performance corpus: N2 |
| Functions | Direct/indirect calls, closures/captures, supported generic tagged values, defaults and references | Absent-value transport and mutable-reference callable payloads: N2 |
| Value structs | Managed C values, generic tagged fields, nullable/union snapshots, collection payloads, callback conversion and nested logical field writeback | Polymorphic array fields and native specialization: N2 |
| Collections and classes | Arrays, class objects, maps, sets, symbols, buffers and typed arrays; supported element and call recipes are checked | Remaining recipes and comparisons: N2 |
| Records, shapes and static data | Traced records, ordered Object keys/values/hasOwn/assign, checked JSON stringify, shape fields/spreads/optional writes/tag narrowing, scalar array join | Typed parsing, polymorphic collection transport and immutable graph qualification: N2 |
| Exceptions | Source-qualified refusal | Status propagation, catch/finally, throwing calls: N2 |
| Generators, async/tasks | Source-qualified refusal | Region state machines and microtask queue: N2 |
| Regular expressions | Source-qualified refusal | Pinned ECMAScript-compatible engine: N2 |
| Strings | Reference-counted UTF-16 ABI v2, owned views and temporary-conversion cleanup | Complete Unicode/runtime support: N2 |
| Memory | Reference counting, traced closures/objects/containers, final-use transfer and synchronous trial deletion | Broad cycle/performance qualification: N2 |
| Extern providers | Explicit `host_` functions and generated C headers, mapped from checked identities | Remaining extern/C library ABI and portable process/file/clock API: N2 |
| Toolchain | One library owner; strict C11 flags, explicit TOML controls, source/output receipts | Native objective measurements, sanitizer matrix and cross-target profiles: N2 |
| JavaScript host facilities | `JsValue`, `unknown`, `JS.*`, extern JS classes, `object {}`, JS module namespaces/dynamic import and ambient JS APIs | Declared JavaScript-only |

An implemented family is not a blanket claim that all representation
combinations work. Target-aware checking is the authority for a particular
program. An unsupported recipe is an error, never a silent fallback to a
different language or a success counted in native qualification.

Provider callback ABI v2 permits synchronous entry/reentry on the originating
thread. Inputs are borrowed; retaining handles acquires an owner and returned
handles transfer an owner. Retained callbacks must be released before execution
ends. Concurrent callbacks and foreign unwinding are outside this ABI. Strings
carry `{data, length, owner}` under string ABI v2: input strings borrow, retained
views own their backing allocation, and returned strings transfer one owner.
Static host storage uses a null owner; `ls_string_from_utf16` copies temporary
host storage into an owned string. ABI v1 headers must be regenerated. Provider parameters are explicit values, with
no defaults or implicit receiver. Native library exports are a separate N2
contract, not an inferred C ABI for JavaScript exports.

The old corpus's lexical native masks are coverage classifications, not a proof
that the checker/runtime is complete. In particular an `extern` can be supported
by a configured C provider even though an ordinary `.host.js` fixture cannot.
N1 inventories those exclusions; N2 must implement/qualify portable cases and
keep explicitly JavaScript-bound cases visible. Qualification never treats a
masked or refused case as a pass.
