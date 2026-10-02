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
| Functions | Direct/indirect calls, captured declarations, supported generic tagged values, per-argument absence/defaults and managed/callable references | Polymorphic container/callback bridges implemented; complete qualification: N2 |
| Value structs | Managed C values, generic tagged fields, nullable/union snapshots, collection payloads, callback conversion and nested logical field writeback | Native specialization and full qualification: N2 |
| Collections and classes | Shared-identity concrete/generic arrays, owned callbacks and sparse copies; class objects, maps, sets, symbols, buffers and typed arrays | Remaining recipes and comparisons: N2 |
| Records, shapes and static data | Traced records, ordered Object keys/values/hasOwn/assign, scalar/array/record JSON output including floats, owned JSON parsing with typed representation views, shape fields/spreads/optional writes/tag narrowing, scalar array join and once-created immutable static graphs | Full capability/ABI qualification: N2 |
| Exceptions | Owned status through calls and callbacks, catch/rethrow, finally completion overrides and checked bounds/range failures; explicit C provider pending/take/raise | Remaining native error recipes and full corpus qualification: N2 |
| Generators, async/tasks | Source-qualified refusal | Region state machines and microtask queue: N2 |
| Regular expressions | Pinned QuickJS libregexp; typed construction/test/search/string replacement, metadata and shared lastIndex; catchable syntax/resource errors | Broad corpus qualification: N2; JsValue match objects use the JavaScript host ABI |
| Strings | Reference-counted UTF-16 ABI v2, owned views, temporary-conversion cleanup and Unicode 17 case conversion | Broad runtime qualification: N2 |
| Memory | Reference counting, traced closures/objects/containers, final-use transfer and synchronous trial deletion | Broad cycle/performance qualification: N2 |
| Extern providers | Explicit `host_` functions and generated C headers, mapped from checked identities | Remaining extern/C library ABI and portable process/file/clock API: N2 |
| Toolchain | One library owner; strict C11 flags, explicit TOML controls, source/output receipts | Native objective measurements, sanitizer matrix and cross-target profiles: N2 |
| JavaScript host facilities | `unknown`, host-specific `JsValue`/`JS.*` operations, extern JS classes, `object {}`, JS module namespaces/dynamic import and ambient JS APIs | Declared JavaScript-only; portable tagged value transport and checked representation views also support native catches |

An implemented family is not a blanket claim that all representation
combinations work. Target-aware checking is the authority for a particular
program. An unsupported recipe is an error, never a silent fallback to a
different language or a success counted in native qualification.

Provider callback ABI v3 permits synchronous entry/reentry on the originating
thread. Inputs are borrowed; retaining handles acquires an owner and returned
handles transfer an owner. Retained callbacks must be released before execution
ends. Concurrent callbacks and foreign unwinding are outside this ABI. Strings
carry `{data, length, owner}` under string ABI v2: input strings borrow, retained
views own their backing allocation, and returned strings transfer one owner.
Static host storage uses a null owner; `ls_string_from_utf16` copies temporary
host storage into an owned string. Older callback headers must be regenerated. Provider parameters are explicit values, with
no defaults or implicit receiver. Native library exports are a separate N2
contract, not an inferred C ABI for JavaScript exports.

Optional callback signatures carry `ls_native_arguments {count, absent}`.
`count` is the number of supplied arguments; a non-null borrowed `absent` array
has one boolean per supplied argument and marks absence at optional positions.
A null pointer means all supplied values are present. Missing physical slots
hold inert zero sentinels until the callee evaluates its own defaults; they are
never a source default. The descriptor has no fixed arity limit and is borrowed
only during invocation. Ordinary calls without defaults keep their existing ABI.

Legal `ref` parameters borrow the caller's lexical location, including managed
strings, products and callable slots. Callee assignment updates that owner in
place; parameter entry/exit does not acquire or release a separate pointee.
The source language forbids capturing a reference parameter, suspending with
one or exposing it through an opaque public/foreign callable contract. Native
preserves those restrictions and does not add heap storage to ordinary references.

The old corpus's lexical native masks are coverage classifications, not a proof
that the checker/runtime is complete. In particular an `extern` can be supported
by a configured C provider even though an ordinary `.host.js` fixture cannot.
N1 inventories those exclusions; N2 must implement/qualify portable cases and
keep explicitly JavaScript-bound cases visible. Qualification never treats a
masked or refused case as a pass.

Concrete and generic array views share one traced identity and mutable buffer.
A storage descriptor preserves typed elements on the direct path and uses checked
tagged conversion at a representation boundary; it does not copy the array.
Sparse callback results keep a separate presence bitmap, allocated only when
needed. Callbacks retain the current element across source mutation and release
per-iteration conversion temporaries; managed reduce accumulators own their
current value. These are semantic guarantees, not measured performance wins.

Native exceptions use a status on the originating thread and an owned tagged
payload. Calls return through ordinary C frames; generated callers branch to
their lexical handler and release exited owners. Each `finally` saves both its
exception and return value, so nested completion replacement preserves the
outer pending completion. Shared throw-free function summaries can omit call
checks under the dead-code-elimination control; disabling it retains checks.

A C provider that calls a source callback must inspect
`ls_native_exception_pending()` before continuing. It can return immediately
to propagate the status, or use `ls_native_exception_take()` to clear it and
receive an owned `ls_value`. `ls_native_exception_raise(value)` retains its
borrowed argument and replaces the pending exception. These additive ABI 2
functions never unwind a foreign C frame. Resource exhaustion and violated
runtime ownership invariants remain fatal. Runtime type/bounds/range failures
are ordinary catchable failures; source precondition checks on JavaScript are
selected independently with `[javascript] checks="development"`.

Native case conversion and regular expressions use Unicode 17.0.0, pinned with
QuickJS libregexp at `535a7c250ff4a577ec36c3e103daab6dadeea650`. The independent
comparison runtime must use the same Unicode version: older JavaScript engines
can differ on newly assigned characters. Case conversion preserves lone UTF-16
surrogates, expansions and contextual final sigma. Regex supports the engine's
ECMAScript flags, including `d` and `v`, while typed metadata remains the source
language's declared surface. `exec`/JS match objects and callback replacements
remain JavaScript host operations. No target silently approximates those objects.

The library sources and MIT license live in `third_party/quickjs`; the checked
manifest and `scripts/vendor-native-text.py` reproduce standalone amalgamations.
Their private declarations have internal linkage. Vendor-only unused/sign-compare
warnings are isolated; generated program and provider warnings remain enabled.
Unicode tables are included only when case conversion or regex needs them.
TOML stack/backtracking limits report catchable exhaustion;
resource rejection does not turn a regex operation into an ordinary non-match.

JSON parsing creates the same owned tagged arrays/records used by ordinary
native code; typed views use the existing `JS.assume` precondition, without a
new schema-validation promise. Invalid text raises a catchable syntax error
and releases partial containers. Nesting uses an explicit heap stack. Decimal
input and shortest binary64 output use the pinned numeric library with documented
rounding corrections, independent of the host locale. These are semantic
guarantees, so compilation effort and native speed/size flags cannot weaken them.

Polymorphic callable views use demand-driven signature bridges across tagged
storage, arrays and generic product fields. Nested callback parameters and
returned closures preserve their original function identity. Bridges own their
inner callable; a reverse view recovers that callable instead of adding another
wrapper. Equal physical conventions need no allocated adapter. These are
representation recipes under the source contract, not configurable relaxations.

Native trusted views check the physical allocation witness needed for safe C
access. A class view accepts that class or a subclass; record/map/regex/buffer
views require their own runtime layout, and typed arrays preserve their element
width. These checks enforce memory safety even when a source precondition is
violated. They are not recursive validation of every field/element or an implicit
coercion. Failed conversions propagate before user callbacks or constructors
run. The native optimizer propagates this failure fact over the shared call
graph; it does not infer that a source throw-free summary erases a necessary
representation check.

Checked immutable arrays, records, shapes, classes and products created once by
module initialization can use static storage under `target.native.static_data`.
Aliases and typed product copies keep their original meaning. Repeated function
and loop activations create fresh owned objects. Static owners never enter
reference-count or cycle-collector worklists. Mutable outer copies from `slice`,
`concat` and `Object.values` preserve the protection of shared const children.
The [focused qualification](../benchmarks/migration-results/2026-10-02-n2-native/README.md#immutable-static-graphs)
records startup allocation and the executable-size cost of each setting.
