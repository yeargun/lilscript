# LilScript Language Contract v0.1

Reasoning (types vs glue, closed world, escape, delivery): [knowledge/language](knowledge/language/README.md). This page is the syntax/semantics contract.

## Identity and compilation model

LilScript is an independent statically typed language. LilScript source is never
parsed as JavaScript or TypeScript, and neither language defines LilScript
semantics. JavaScript and native object code are backend targets of the same
typed whole-program IR.

Every executable program is analyzed as a closed world. An explicit `extern`
declaration is required to cross into a host boundary. An `extern` function is
a typed call contract for JavaScript or C. An `extern class` plus an `extern`
global declares a typed JavaScript host object such as `document` or `window`.
Values that reach either boundary are considered escaping and must use the
boundary ABI representation.

The entry `.lil` file and every transitive static import form one compilation
unit. Module boundaries are resolved before semantic analysis and erased before
SSA optimization; they are not JavaScript wrappers in the generated bundle.

## Lexical grammar

- Source text is UTF-8.
- Identifiers begin with ASCII `_`, `$`, or a letter and continue with those
  characters or digits.
- Keywords remain reserved for bindings. Property-name positions—including
  aggregate fields and methods, enum variants, record keys, and member access—
  accept keyword names so typed JavaScript interfaces can preserve fields such
  as `async`, `generator`, or `catch` without aliases.
- `//` line comments and `/* ... */` block comments are ignored.
- Statements end with `;`, except blocks and declarations ending in `}`.
- Decimal integer and IEEE-754 decimal float literals are supported.
- Strings use double quotes. Template strings use backticks and `${expr}`.

## Types

| LilScript type      | Meaning                                                       | JavaScript representation                         | Native representation                       |
| ------------------- | ------------------------------------------------------------- | ------------------------------------------------- | ------------------------------------------- |
| `int`               | signed 32-bit integer with operator-defined overflow behavior | number with i32 normalization                     | `i32`                                       |
| `number` / `float`  | IEEE-754 binary64 web number                                  | number                                            | `f64`                                       |
| `bool`              | `true` or `false`                                             | boolean                                           | C11 `bool`                                  |
| `string`            | immutable UTF-8 text                                          | string                                            | runtime string handle                       |
| `T[]`               | mutable homogeneous array                                     | optimized array representation                    | runtime array handle                        |
| `Record<T>`         | mutable open string-keyed homogeneous record                  | null-prototype object                             | tagged-value string map handle              |
| `Map<K, V>`         | mutable insertion-ordered key/value collection                | native `Map`                                      | tagged-value map handle                     |
| `Set<T>`            | mutable insertion-ordered unique-value collection             | native `Set`                                      | tagged-value set handle                     |
| `ArrayBuffer`       | fixed-length byte storage                                     | native `ArrayBuffer`                              | owned byte-buffer handle                    |
| `SharedArrayBuffer` | fixed-length storage shared by views                          | native `SharedArrayBuffer`                        | shared-designated byte-buffer handle        |
| `Int8Array`         | signed 8-bit view                                             | native `Int8Array`                                | typed-array view handle                     |
| `Uint8Array`        | unsigned byte view                                            | native `Uint8Array`                               | typed-array view handle                     |
| `Uint8ClampedArray` | clamped unsigned byte view                                    | native `Uint8ClampedArray`                        | typed-array view handle                     |
| `Int16Array`        | signed 16-bit view                                            | native `Int16Array`                               | typed-array view handle                     |
| `Uint16Array`       | unsigned 16-bit view                                          | native `Uint16Array`                              | typed-array view handle                     |
| `Int32Array`        | signed 32-bit view                                            | native `Int32Array`                               | typed-array view handle                     |
| `Uint32Array`       | unsigned 32-bit view (`int` bit pattern)                      | native `Uint32Array`                              | typed-array view handle                     |
| `Float32Array`      | IEEE-754 single-precision view                                | native `Float32Array`                             | typed-array view handle                     |
| `Float64Array`      | IEEE-754 double-precision view                                | native `Float64Array`                             | typed-array view handle                     |
| `Symbol`            | unique opaque identity value                                  | native `Symbol`                                   | unique symbol handle                        |
| `Regex`             | exact ECMAScript regular expression                           | native `RegExp`                                   | unsupported                                 |
| `Task<T>`           | typed asynchronous result                                     | native `Promise`                                  | unsupported                                 |
| `Generator<T>`      | typed synchronous iterable yielding `T`                       | native generator object                           | unsupported                                 |
| `JsValue`           | raw dynamically typed JavaScript boundary value               | unchanged host value                              | unsupported                                 |
| `T?`                | either a `T` value or `null`                                  | `T` or raw `null`                                 | tagged `LilScriptOptional`                  |
| `A \| B`            | value belonging to either member type                         | raw member value                                  | tagged `LilScriptValue` at union boundaries |
| `enum E`            | one value from a closed named variant set                     | zero-based integer discriminant                   | `int32_t` discriminant                      |
| `struct S`          | positional value aggregate                                    | scalars, tuple, or boundary object                | positional C value record                   |
| `class C`           | nominal reference value with methods                          | dissolved record or class at an escaping boundary | pointer to a C record                       |
| `object O`          | closed public object; ABI keys, private method bodies         | named object literal or clustered assigns         | unsupported                                 |
| `extern class C`    | typed JavaScript host object interface                        | existing host object with exact member names      | unsupported without an explicit user ABI    |
| `func(T...)->R`     | callable value                                                | function/closure                                  | function plus environment                   |
| `C<T...>`           | applied generic class                                         | same nominal class layout                         | pointer with boxed polymorphic fields       |
| `void`              | no value                                                      | no value                                          | no value                                    |

`auto` is a declaration inference marker, not a runtime type. It is legal only
for a local or top-level variable with an initializer.

An `int` widens implicitly to `number`/`float`. Use `number` for ordinary web
numeric values that do not require i32 wrapping. It is the preferred spelling
of the existing `float` representation: arithmetic stays as JavaScript number
operations without `|0` boundaries and native lowering uses binary64. Integer
literals assigned or passed to `number` are promoted before subsequent
arithmetic. Bitwise and shift operators remain `int`-only because JavaScript
itself applies i32 coercion to those operations. Other conversions require an
explicit standard conversion function. Arrays and nominal types do not
implicitly coerce.

Closed enums use declaration-order discriminants and do not emit a JavaScript
metadata object:

```lilscript
enum Status { Draft, Active, Sold }

string label(Status status) {
  return match(status) {
    Status.Draft => "draft",
    Status.Active => "active",
    Status.Sold => "sold"
  };
}
```

Every non-wildcard pattern must name a variant of the scrutinee's exact enum.
Duplicate variants, unknown variants, and arms after `_` are rejected. Without
`_`, all declared variants must be covered; `_` may occur only once and last.
The scrutinee is evaluated exactly once and only the selected arm is evaluated.
All arms must have a common assignable result type. Enum values are nominal:
they do not implicitly convert to `int` or to another enum. The numeric ABI is
intended for closed LilScript code; string-valued external protocols require an
explicit conversion such as the exhaustive `match` above.

`match` also accepts `int`, `string`, and `bool` literal patterns. Integer and
string matches require a final `_` arm; booleans are exhaustive when both
`true` and `false` are present. Duplicate or mixed-type patterns are rejected.
Negative integer patterns are written directly (`-1 => value`). Scrutinee and
arm evaluation retain the same exact-once and lazy semantics as enum matches.

`Record<T>` is an open structural record whose values all have the same static
type. A record literal uses `record { key: value, "quoted-key": value }` and
lowers directly to a null-prototype JavaScript object; it is distinct from a nominal,
fixed-layout `struct`. Duplicate literal keys and mixed value types are
rejected. An empty literal needs an expected `Record<T>` type.

`object { key: value }` is the explicit ordinary-object counterpart. Its static
type is `JsValue`; it has `%Object.prototype%`, preserves own `__proto__` as a
data key, and subsequent reads/writes retain normal getter/setter/proxy
observability. It is JavaScript-only. The optimizer may forward a statically
own data-key read only while the compiler-owned allocation never escapes and is
not written; missing/dynamic keys and escaped objects remain observable. Object
spread is not yet supported.

Member and string-index reads return `T?` because an open key may be absent.
Direct member and index writes require `T`. Compound assignment and update on a
record entry are rejected until presence has been represented by an explicit
place operation; this prevents a missing property from accidentally becoming a
JavaScript `NaN` or concatenated string. Record property names are observable
data and are never mangled. Enumeration follows JavaScript own-property order:
canonical array-index strings first in ascending numeric order, followed by
other strings in insertion order. The null prototype makes inherited names
absent and turns `__proto__` into an ordinary data key on dynamically written
records rather than a prototype mutation.

The portable static record operations are:

- `Object.keys(record)` returns `string[]`;
- `Object.values(record)` returns `T[]` in the same key order;
- `Object.hasOwn(record, key)` returns `bool`;
- `Object.assign(target, source)` mutates and returns `target`; both records
  must have the exact same invariant `Record<T>` type.

`JSON.stringify(value)` returns `string` for `int`, enum, `string`, `bool`,
`null`, nullable forms of those scalars, and homogeneous arrays or records of
those scalars. Floats are currently rejected: the native runtime does not yet
implement ECMAScript's shortest binary64-to-decimal algorithm, so accepting
them would make output target-dependent. `JSON.parse(string)` returns
`JsValue` and is therefore JavaScript-only; native compilation rejects it
instead of substituting a different dynamic representation.

`Regex` is the exact JavaScript-target ECMAScript regular-expression type:

```lilscript
Regex sale = new Regex("sale", "gi");
bool first = sale.test("SALE sale");
bool second = sale.test("SALE sale");
string source = sale.source;
string flags = sale.flags;
```

Construction accepts a pattern plus an optional flags string. `test(string)`
preserves JavaScript's stateful `global` and `sticky` behavior. The typed
metadata surface is `source`, `flags`, `global`, `ignoreCase`, `multiline`,
`dotAll`, `sticky`, and `unicode`. Construction and testing remain effectful in
the optimizer because invalid patterns can throw and stateful tests update the
regular expression. Native compilation rejects `Regex` rather than
approximating ECMAScript syntax or Unicode behavior.

With the `regex-literals` compression decision enabled **and**
`javascript.assume_pristine_builtins = true`, the JavaScript emitter may
replace a constructor with a literal only for a deliberately narrow,
statically valid pattern and valid-flags subset, and only when the literal is
shorter. The explicit runtime assumption is required because a literal bypasses
the ambient `RegExp` constructor binding; open-world library builds keep the
constructor form. Unsupported escapes or grammar, slashes, line terminators,
duplicate or unknown flags, and incompatible `u`/`v` flags retain
`new RegExp(...)`, preserving runtime error timing. Constants eliminated by
this substitution must have no other uses, so the transformation cannot leave
dead bindings or alter shared-value emission. The release benchmark records raw,
gzip, and Brotli measurements plus runtime, memory, and stateful behavior. Both
variants are Brotli-target builds, so only Brotli is gated; raw and gzip are
diagnostics and may regress.

## Async tasks and exceptions

JavaScript-target functions and methods may be declared `async`; their body
retains the declared inner return type while calls have type `Task<T>`:

```lilscript
async int loadCount() {
  try {
    return await Task.resolve(4);
  } catch (auto error) {
    print(error.message ?? "rejected");
    return 0;
  } finally {
    print("settled");
  }
}
```

`await` is legal only inside an async body and accepts only `Task<T>`.
`Task.resolve(value)` returns `Task<T>`, `Task.reject(reason)` obtains `T` from
its expected task context, and `Task.all(Task<T>[])` returns `Task<T[]>`.
Tasks expose typed `then`, `catch`, and `finally` chains and lower directly to
native promises without a Lilscript scheduler or wrapper. General rejection
reasons are `JsValue`, not assumed error records; their `message` and
`specifier` reads are nullable and use null-safe JavaScript access.

`throw` accepts any non-`void` value. A try statement requires `catch`,
`finally`, or both. Catch may use `catch (auto error)`,
`catch (JsValue error)`, or omit the binding as `catch { ... }`. Each clause is
lexically scoped. JavaScript's native completion rules are preserved: finally
runs on normal completion, throw, return, break, and continue, and a completion
from finally overrides the earlier one.

Exception regions are emitted as native structured JavaScript. Their mutable
locals deliberately remain native mutable bindings instead of being promoted
to exception-insensitive SSA, so a catch observes every assignment completed
before the exact operation that threw. Exception-bearing functions are also
excluded from CFG rewrites that cannot preserve structured regions. The
`unused-catch-binding-elision` compression decision removes `(error)` only when
the checked binding has zero uses; codec-specific release fixtures require a
strict gzip/Brotli win and guard output, runtime, and retained heap. Async,
tasks, and exceptions are rejected by the native backend rather than
approximated.

## Generators

A generator declares its yielded element type after the `generator` modifier.
Calling it returns `Generator<T>`; its body may `yield` one `T`, delegate with
`yield*` to `T[]`, a compatible typed array, or another `Generator<T>`, and may
return only without a value:

```lilscript
generator int range(int stop) {
  for (int value = 0; value < stop; value++) {
    yield value;
  }
}

generator int values() {
  yield* [7, 8];
  yield* range(3);
}

for (int value of values()) {
  print(value);
}
```

Generator methods use the same modifier. JavaScript emission is direct
`function*`, `yield`, `yield*`, and `for...of`; there is no iterator helper or
state-machine runtime. Native compilation rejects generator functions. A
regular or arrow-function boundary blocks `yield`, so a nested callback cannot
accidentally suspend its containing generator. Async generators are not yet in
the portable core.

The `compact-generator-star` compression decision compares the equivalent
spellings `function*name` and `function* name`. Lilpack retains both candidates
and accepts the compact form only when the selected whole-artifact codec wins.

## Collection literals, destructuring, and iteration

Array and record literals support left-to-right shallow spread:

```lilscript
int[] copy = [0, ...values, 3];
Record<int> merged = record{...base, count: 2};
```

An array spread operand must be `T[]`; typed arrays are deliberately rejected
until their unboxed native representation can be copied without a hidden boxed
conversion. A record spread operand must have the exact invariant
`Record<T>` value type. Both forms allocate a new collection, preserve source
order, evaluate each operand once, and do not retain the source collection as
an alias.

Destructuring declarations use `auto` because their binding types are inferred:

```lilscript
auto [first, , third, ...tail] = values;
auto {name, "unit-price": price, ...remaining} = listing;
```

An ordinary array or record binding has type `T?`: arrays may be shorter than
their patterns and open records may omit a key. Missing values are normalized
to `null` in JavaScript and use the native optional representation. Array rest
has type `T[]`; record rest has type `Record<T>`. Rest is always last, produces
a shallow copy, and never aliases later source insertion or replacement.
Record rest excludes the named source keys, retains JavaScript own-key order,
and preserves the null-prototype record contract. The right-hand expression is
evaluated exactly once. Duplicate record keys and duplicate binding names are
rejected.

`for (T value of collection)` accepts `T[]` and typed arrays. It uses live
array length, matching JavaScript array-iterator behavior when the loop appends
elements. Lowering uses a direct indexed loop in both JavaScript and native
output; it does not allocate an iterator or callback. Strings are not accepted
because JavaScript string iteration uses Unicode code points while the native
string indexing contract is UTF-16-oriented, and silently combining those
semantics would be target-dependent.

`inline for (T value of [/* const list */])` unrolls at compile time. The
iterable must be an array literal of `int`, `float`, `string`, or `bool`
values. `break` and `continue` are rejected because there is no runtime loop.
A closed program uses this when the table is in the compilation unit. When
`optimization.for_of_specialize_family` is set, a residual `for` over a
function's first array parameter also gets unrolled clones plus a picker.

`Map<K, V>` and `Set<T>` are mutable and invariant. `Map.get(key)` returns
`V?`; missing keys and stored `null` values therefore have the same result, as
they do after JavaScript lowering with `?? null`. Collection keys use
SameValueZero for floats and identity for reference types. Struct keys and set
elements are rejected because structs have value semantics and no portable
identity contract yet. Native collection storage currently uses deterministic
linear lookup; this is a correctness baseline that a later representation pass
may specialize without changing source semantics.

`ArrayBuffer` and `SharedArrayBuffer` accept one `int` byte length.
The nine core typed-array views (`Int8Array`, `Uint8Array`,
`Uint8ClampedArray`, `Int16Array`, `Uint16Array`, `Int32Array`, `Uint32Array`,
`Float32Array`, `Float64Array`) accept an element length or either buffer type,
support indexed reads and writes, and expose `length`, `byteLength`,
`byteOffset`, and `buffer`. `slice(start, end)` copies; `subarray(start, end)`
creates a zero-copy view. The `end` argument defaults to the view or buffer end.
Integer views wrap on store; `Uint8ClampedArray` clamps into `0..255`.
`Uint32Array` elements are still typed as `int` and use ToInt32 bit-pattern
semantics on read and write. Float views convert through IEEE-754 binary32 or
binary64. Assignment and prefix-update expressions still evaluate to the
numeric value before storage coercion, matching JavaScript typed-array behavior.
This increment deliberately does not expose resizable/growable options,
`DataView`, `BigInt64Array`/`BigUint64Array` (no `BigInt` yet), or `Atomics`.
JavaScript `SharedArrayBuffer` availability remains a host concern; the
ECMAScript host may omit its global constructor, and sharing it across web
agents requires the browser's isolation policy. Native lowering preserves
shared view identity in one process but does not yet claim concurrent or atomic
memory semantics.

`float[]|Float32Array` values support shared indexed reads, writes, and
`length`, allowing configurable numeric kernels to keep one source path while
native code dispatches between boxed arrays and binary32 views.

`Symbol` is constructed with `new Symbol()` or `new Symbol(description)`. Each
value has unique identity; `==` compares references, not descriptions. `Symbol`
and `string` keys may coexist in the same `Map` or `Set`.

Browser object interfaces are declared rather than built into the parser:

```lilscript
extern class Document {
  Element createElement(string tag);
  Element? querySelector(string selector);
}
extern Document document;
```

External globals are read-only bindings, external classes cannot be constructed,
and declared member accesses emit direct JavaScript property operations. Their
global and member names are exact by default. The current explicitly configured
closed-key mode assumes every producer and consumer shares the renamed ABI; it
must not be used for ordinary browser/host objects. Host property reads and methods are
effectful unless a method has a trusted `pure` contract. The C and native targets
reject host-object access because the Web platform has no portable C ABI. See
[web-platform.md](web-platform.md) for the complete implemented boundary and
current Web IDL limitations.

`JsValue` is the narrow escape hatch for JavaScript APIs whose public input
domain is genuinely dynamic. It may cross an `extern` or exported boundary and
accepts any non-`void` LilScript value, but it does not enable arbitrary member
dispatch. Its implemented operations are deliberately explicit:

- `value.truthy()` applies JavaScript truthiness;
- `value.isArray()` applies `Array.isArray(value)` without claiming an element type;
- `value.isObject()` applies `typeof value == "object"` (and therefore includes `null`);
- `value.length` returns the raw JavaScript numeric length as `float`;
- `value[index]`, for a numeric or string index, remains `JsValue`;
- `for (string key in value)` emits direct JavaScript `for-in`, including inherited enumerable string keys and without allocating `Object.keys(...)`;
- `value is string`, `value is float`, and `value is bool` are sound narrowing guards. JavaScript numbers must use `float`; array and function signatures cannot be proven by `typeof` and are rejected as narrowing targets.
- `JS.construct(ctor, ...args)` evaluates `new ctor(...args)`. The callee is required; up to six further `JsValue` arguments are constructor arguments. C/native targets reject it.

Thirteen typed JavaScript adapter primitives create ordinary host-callable
functions without weakening the callback's static signature:

- `JS.methodN(func(JsValue, ...N JsValue parameters) -> JsValue)`, for each
  integer `N` from `0` through `10`, passes the wrapper's `this` followed by
  its first `N` call arguments;
- `JS.methodRest(func(JsValue, JsValue) -> JsValue)` passes `this` and the
  wrapper's real JavaScript `arguments` object;
- `JS.staticRest(func(JsValue) -> JsValue)` passes only that `arguments`
  object.

Each evaluation returns a fresh anonymous, constructible ordinary function.
Each `methodN` wrapper has JavaScript `length == N`; both rest wrappers have
`length == 0`, and every callback is invoked as a plain function. Semantic analysis
resolves these operations by builtin identity and checks the exact callback
arity and types; an unrelated extern with the same spelling has no special
behavior. The JavaScript backend may fuse a private callback with its wrapper
only after proving its identity and lexical bindings do not escape; where
JavaScript would infer a function name, the fused spelling explicitly
preserves the wrapper's anonymous reflection. Otherwise it emits a
compiler-private shared factory. C/native targets reject all thirteen adapters.

String concatenation may consume a guarded `JsValue` and uses JavaScript's
ordinary coercion. An unguarded Symbol therefore throws exactly as it would in
JavaScript. A declared `extern JsValue arguments;` refers to the current
function's JavaScript `arguments` object; the emitter forces that function to
ordinary-function syntax even when public arrows are requested. Every
`JsValue` operation is rejected by the C/native backend rather than receiving a
different approximation.

`JsValue` does not carry a blanket purity assumption. An operation that can run
user-controlled JavaScript coercion (`Symbol.toPrimitive`, `valueOf`, or
`toString`), trigger a proxy trap or revoked-proxy check, or throw during a
dynamic conversion is an observable evaluation point. This includes applicable
dynamic arithmetic/equality/string conversion, dynamic indexing and property
inspection, and `isArray()` on an unknown host value. Such an evaluation is not
deleted merely because its result is unused, is not merged with an equal-looking
evaluation, stays in source order, and makes a declared `pure` function invalid.
Non-coercive operations such as truthiness, `typeof`-based narrowing, and nullish
tests remain pure when their operands need no observable access.

Postfix `?` makes a value type nullable. `null` is assignable only to a nullable
type, and `auto value = null;` is rejected because it has no concrete value type
to infer. Nullable values may be compared with `null`, their underlying value,
or another compatible nullable. JavaScript keeps raw `null`; native code uses a
tagged payload so nullable primitives and aggregates have the same semantics.
Direct `value != null` guards narrow `value` inside the true branch, while
`value == null` narrows it inside the false branch. Assignment invalidates the
narrowing. This permits guarded member, method, and index access without a
runtime wrapper in JavaScript; native code emits a typed tagged-payload unwrap.
When one branch of a direct null or member guard guarantees return, the other
branch's narrowing continues after the `if`; assigning the binding invalidates
that narrowing. Compound-condition narrowing is not part of this increment.

Infix `|` forms a first-class union type. Parentheses control postfix binding,
so `(string | int)[]` is an array whose elements may be strings or integers,
while `string | int[]` is either one string or one integer array. Assignments,
returns, generic inference, callbacks, fields, and arrays accept any declared
member. JavaScript erases the union after static checking. Native code keeps
concrete values unboxed and uses `LilScriptValue` only when a value crosses a
union boundary; equality and string conversion dispatch on that tag. A
default-constructed union field uses the first member's default value.

`value is Type` tests one concrete union member and narrows an identifier in
both branches. `!` swaps the branch narrowings. Guards are deliberately limited
to runtime categories that have identical JavaScript and native semantics:
`int`/`float` numbers, `string`, `bool`, arrays, and functions. A guard is
rejected when another member has the same runtime category, so `int | float`
cannot be distinguished with `is`; no backend-dependent reflection is exposed.
JavaScript lowers guards to `typeof` or `Array.isArray`, while native code tests
the union tag and unboxes the narrowed member.
For `(A | B)?`, a preceding `value != null` guard first narrows to `A | B`;
member guards then narrow that union normally and native lowering composes the
optional-payload and union-tag unwraps.

Generic functions declare type parameters after the function name. Calls infer
their type arguments from ordinary values and callback parameter/return types:

```lilscript
T apply<T>(T value, func(T)->T transform) {
  return transform(value);
}

int answer = apply(20, (int value) => value + 22);
```

Generic classes declare parameters after the class name. Applied types use
angle brackets; constructor calls may state arguments explicitly or infer them
from constructor values and the expected binding type:

```lilscript
class Cell<T> {
  T value;
  func(T)->T transform;

  init(T value, func(T)->T transform) {
    this.value = value;
    this.transform = transform;
  }
}

Cell<int> cell = new Cell(2, (int value) => value * 3);
```

JavaScript erases generic types after checking. Native C stores abstract type
parameters in `LilScriptValue` and boxes or unboxes at direct-call and field
boundaries. Native closures use one universal calling convention, so a concrete
callback remains callable through `func(T...)->R`. Polymorphic functions are
not inlined until the optimizer can substitute their call-site types.

Integer addition, subtraction, negation, `&`, `|`, and `^` wrap to signed
32-bit two's-complement values. `<<` shifts the bit pattern left, `>>` sign
extends, and `>>>` shifts the unsigned bit pattern before reinterpreting the
result as a signed `int`; every shift count is masked with `31`. Ordinary integer multiplication evaluates the operands
as IEEE-754 binary64 numbers and then applies signed-i32 normalization, matching
JavaScript's `(left * right) | 0` even when the rounded product exceeds the
exact-integer range. `Math.imul(left, right)` is a typed, pure intrinsic that
instead returns the exact low 32 bits of the product. The compiler never
rewrites ordinary multiplication into `Math.imul`, and it never rewrites an
explicit `Math.imul` into ordinary multiplication. The two operations agree
when the binary64 product is exact but can differ for large operands.

Integer division truncates toward zero; division or remainder by zero produces
`0` on every backend. These are language guarantees shared by JavaScript and
native output. A source-written live `value | 0` is an explicit JavaScript
lowering obligation and remains `|0` under every objective; dead enclosing code
may still disappear. JavaScript may drop compiler-generated, proven-redundant
signed-i32 normalization for `size-first` and `balanced`.
`performance-first`, `realistic-performance-first`, and
`javascript.integer_coercions = true` keep generated normalization too.
Overflow-capable operations still wrap. Float arithmetic follows IEEE-754 binary64 behavior.

## Declarations

```lilscript
int count = 5;
float ratio = 3.14;
string name = "LilScript";
bool enabled = true;
string? subtitle = null;
auto inferred = count * 2;
int[] values = [1, 2, 3];
func(int)->int twice = (int value) => value * 2;
```

Bindings are block scoped. Reading before declaration is invalid. A binding may
shadow one from an outer scope but cannot be declared twice in one scope.
Every runtime variable declaration requires an initializer; fields are the only
declarations initialized by their aggregate or class representation.

## Modules

LilScript modules use named, relative imports. The `.lil` extension may be
omitted. A side-effect-only import is also supported.

```lilscript
import { square, Point as Coordinate } from "./math";
import "./startup.lil";

export pure int area(int width, int height) {
  return width * height;
}

export { Coordinate };
export { internalHelper as publicHelper };

// `export class Coordinate` remains type-only. Publish a runtime constructor
// explicitly when JavaScript consumers must call `new` or observe `.name`.
export constructor Coordinate;
export constructor InternalWidget as Widget;
```

Only explicitly exported top-level functions, variables, structs, classes, objects, and
externs can be imported. Imported names may be aliased with `as`. Module-private
bindings are namespaced by the linker, so equal private names in different files
cannot collide.

`export constructor Name [as PublicName];` is the runtime constructor-value
form. It preserves a named ES class, constructor arity/name/constructibility,
prototype methods, and the public export alias. It requires a non-`object`,
non-extern class. A zero-arity constructor is synthesized when a published base
class omits `init`; inherited exports require explicit `init` with `super(...)`.
Internal inheritance is preserved as named base classes plus `extends`/`super`. Ordinary `export class`
continues to export only the instance type and may dissolve completely.

Relative imports must begin with `./` or `../` and resolve to `.lil` files.
Bare imports resolve only through a verified `lilscript.lock`. Static imports
may form cycles. Interfaces in a strongly connected component are resolved
before linking, every module is initialized once, and acyclic dependencies
retain dependency-first initialization order.

A foreign ESM edge uses `import extern`. Its local binding must be backed by a
top-level `extern` declaration in the importing LilScript module:

```lilscript
import extern { add as hostAdd, version } from "./host.ts";
extern int hostAdd(int left, int right);
extern string version;
```

The import clause supplies runtime ESM identity; the extern declaration
supplies the static LilScript contract. Aliases are allowed. Conflicting
foreign sources for one local binding and imports without a matching extern are
errors. Relative `.js`, `.mjs`, `.ts`, `.mts`, `.jsx`, and `.tsx` sources are
validated by the Lilscript graph loader. Bare ESM specifiers remain runtime
package edges.
Foreign imports are JavaScript-only and are rejected by C/native targets.
`import extern "./setup.ts";` represents a side-effect-only ESM edge and has no
binding contract.

An import's external name is ABI, while its local alias is not. JavaScript
emission may give that local a different hygienic spelling—even when identifier
mangling is disabled—and maps the matching `extern` function or global to the
same spelling. Source bindings and foreign aliases therefore cannot capture
compiler-generated runtime roots such as `Math`, `Array`, `Object`, or `Promise`;
the external import/export names themselves remain unchanged.

The compiler emits the foreign source specifier as native ESM and does not parse
the foreign language. Lilpack integrates Vite to resolve, transform, bundle,
watch, and hot-reload the complete JavaScript/TypeScript/JSX/TSX and asset graph.
The `extern` contract is Lilscript's static view of that runtime binding; Vite's
TypeScript transform does not replace Lilscript type checking at the boundary.

In an executable build, an export is an accessibility declaration rather than a
retention root. Unused imported and exported functions, types, globals, and pure
initializers remain eligible for whole-program elimination, and static module
syntax does not appear in the generated bundle.

The `js-module` target instead creates a reusable ESM boundary. Runtime exports
from the root module are retention roots, their internal bindings remain
mangleable, and a compact named export clause maps them back to stable public
names. Struct and class names are compile-time type exports and therefore do not
produce JavaScript bindings. The default bundle policy emits one optimized
application artifact. A project can opt into static ESM chunks with
`bundle.mode = "preserve-modules"` or `"split"`; partitioning occurs after
whole-program optimization and produces a manifest. These imports are eager.
The dynamic expression `import("./feature")` returns a typed `Task<module>`.
`then`, `catch`, and `finally` are statically checked; contextual `auto` arrow
parameters receive the module namespace or a general `JsValue` rejection.
Split builds normalize loader-created failures to objects with stable
`specifier` and `message` fields, while user-created task rejections may carry
any non-void JavaScript value. Lazy chunks tree-shake unreferenced namespace exports. Lazy-only modules must be
initialization-free. Dynamic module tasks are JavaScript-only. The complete
delivery and package contract is in `docs/modules-and-delivery.md`.

## Aggregates and classes

Struct construction is positional and follows field declaration order.

```lilscript
struct Point {
  int x;
  int y;
}

Point point = Point{10, 20};
```

Classes may define fields, one `init` constructor, and methods. `this` is
available in constructor and method bodies.

```lilscript
class Vector {
  float x;
  float y;

  init(float x, float y) {
    this.x = x;
    this.y = y;
  }

  float lengthSquared() {
    return this.x * this.x + this.y * this.y;
  }
}

Vector vector = new Vector(3.0, 4.0);
```

Classes also support sound, non-virtual single inheritance:

```lilscript
class Priced {
  int price;
  init(int price) { this.price = price; }
  int total(int count) { return this.price * count; }
}

class Listing extends Priced {
  int stock;
  init(int price, int stock) {
    super(price);
    this.stock = stock;
  }
}

Listing listing = new Listing(17, 4);
Priced priced = listing;
print(priced.total(2));
```

A closed `object` is a singleton with ABI keys. Method bodies are ordinary
private functions: they nest, mangle, and fold like other helpers. Keys stay
stable unless `[mangle].exports` is enabled. Multiple files may contribute
methods to the same exported object; the compiler owns one identity.

```lilscript
object Api {
  int add(int left, int right) {
    return left + right;
  }
}

print(Api.add(1, 2));
```

`object` is distinct from `Record<T>` (open data keys), positional `struct`,
and `extern class` (host names). Objects cannot declare type parameters, fields,
`init`, or `extends`, and cannot be constructed with `new`.

Base fields are flattened first and inherited methods call their original
statically known function. Generic base applications such as
`class Child<T> extends Base<T>` substitute inherited member types, derived
values may upcast through the full base chain, and internal/extern inheritance
chains remain separate. A derived `init` must put `super(...)` first and call it
exactly once when the base declares a constructor. Inherited member shadowing
and method overriding are rejected: silently static-dispatching an override
would be unsound, while per-instance vtables would add the size and memory cost
this representation is designed to avoid. The optimized C backend rejects
inheritance until its subtype pointer ABI is fixed rather than emitting
incompatible C pointer calls.

Structs and classes that do not escape are eligible for scalar replacement.
Class calls are statically devirtualized, including inherited calls. Crossing
`extern` materializes the boundary representation. JavaScript may use SSA
scalars, positional arrays, owned named objects, or a proof-required named class;
the declared public/host ABI constrains boundary shape. Native C uses generated
positional value records for structs and pointer records for classes.

## Functions and callable values

Functions use type-first declarations. Parameters are also type-first.

```lilscript
int add(int left, int right) {
  return left + right;
}

auto increment = (int value) => value + 1;
```

Functions and typed arrows are first-class values. Closures share captured
local lexical bindings, so a closure may both read and reassign an outer local;
sibling closures observe the same binding. Objects and arrays referenced by a
capture remain mutable as usual. Top-level bindings are shared globals rather
than closure captures.
All paths of a non-`void` function must return a value.

Trailing parameters can provide scalar, typed array, struct, class-construction,
or typed arrow defaults. Omitted arguments are materialized before SSA lowering,
so JavaScript and native calls use the same full-arity ABI. Reference defaults
allocate a fresh value for every omitted call:

```lilscript
int scale(int value, int factor = 2) {
  return value * factor;
}

int doubled = scale(6);

int count(int[] values = [1, 2, 3]) {
  return values.length;
}

int defaultCount = count();

int apply(
  int value,
  func(int)->int transform = (int current) => current + 1
) {
  return transform(value);
}
```

Defaults currently accept integer, float, string, boolean, `null`, and negative
numeric literals, recursively typed array literals, and arrows assignable to the
declared callable type, plus typed struct literals and `new` class expressions
whose arguments are themselves supported defaults. Default arrows are
declaration-scoped: they may read globals but cannot capture caller locals or
`this`. A nullable callback can also use `null` as an omitted sentinel and
narrow it before invocation. Required parameters cannot follow defaulted
parameters.

Exported JavaScript functions retain declared parameters and scalar defaults in
their public signature, preserving omitted-call behavior and `Function.length`
across the ESM boundary.

Parentheses disambiguate compound callable types. For example, the following
declares an array of callbacks rather than a callback returning an array:

```lilscript
(func(int)->int)[] transforms = [increment];
```

Purity is inferred for every function by interprocedural effect analysis. The
optional `pure` modifier turns that inference into a checked contract:

```lilscript
pure int square(int value) {
  return value * value;
}

pure extern int stableHostHash(int value);
```

A declared-pure LilScript function is rejected if it can print, mutate a global,
array, struct, or class, or call code with observable effects. Calls whose result
is unused can be removed when their target is inferred or declared pure.
`pure extern` is a trusted host ABI promise; violating it is a host integration
error. Dynamic `JsValue` coercions, proxy-sensitive operations, and operations
that may throw through the explicit JavaScript boundary are observable effects;
writing `pure` cannot override that analysis.

Untyped host calls must be declared explicitly:

```lilscript
extern int hostRead(int key);
```

## Statements and expressions

The v0.1 statement set is:

- variable declaration;
- expression statement;
- block;
- `if` / `else`;
- `while`;
- C-style `for`;
- JavaScript-only `for (string key in JsValue)` enumeration;
- `break` and `continue`;
- `return`.

`if` also has a value form with mandatory braces and `else`:

```lilscript
int magnitude = if (value < 0) { -value } else { value };
```

The condition is evaluated once, only the selected arm runs, branch narrowing
applies inside each arm, and the arm types must have a common type. Lowering
creates a source conditional phi; JavaScript may emit either `?:` or structured
control according to the scored representation choice.

Assignments support `=`, `+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `|=`, `^=`,
`<<=`, `>>=`, and `>>>=`. Numeric assignable locations support prefix and
postfix updates: `++value`, `--value`, `value++`, and `value--`. Prefix updates
evaluate to the new value; postfix updates evaluate to the old value. Members
and array elements are valid update targets, while literals and computed
expressions are not.

Prefix `!` and `-`, postfix calls/member/index access, and the standard
arithmetic, comparison, equality, bitwise, shift, and short-circuit logical
operators are supported. Assignment is an expression and evaluates to the
assigned value.

Nullable values support lazy `value ?? fallback` and `place ??= fallback`.
Only `null` selects the fallback; falsy non-null values such as `false`, `0`,
and `""` are preserved. The left value or assignment place is evaluated once,
and `??=` accepts only a fallback assignable to the nullable target. Applying
either operator to a statically non-nullable left operand is rejected.

Nullable data receivers also support `value?.field` and `value?.[index]`. An
absent receiver produces `null`; the index expression is not evaluated on that
path. Combining either form with `??` is lowered as one branch, so no
intermediate nullable value or second test is required. Optional method calls
are deliberately rejected until their receiver binding, argument laziness, and
portable native-call semantics are implemented.

## Standard library surface

Arrays provide typed `length`, `map`, `filter`, `reduce`, `forEach`, `push`,
`pop`, `indexOf`, `includes`, `join`, `some`, `every`, `findIndex`, `concat`,
`slice`, `splice`, `fill`, `copyWithin`, and `reverse`. Callback methods snapshot the receiver length when the call begins, so
elements appended by a callback are not visited by that call. Reads of existing
future elements remain live, matching JavaScript's dense-array iteration
behavior. `indexOf` uses strict equality; `includes` uses SameValueZero, so it
finds `NaN`. Both accept a normalized negative starting index. `some` and
`every` short circuit, while `findIndex` returns the first matching index or
`-1`. `join(separator = ",")` is portable for integer, string, boolean, null,
nullable, and matching union elements; float and nominal-element joins are
rejected because native formatting cannot promise JavaScript's exact text.
`slice(start = 0, end = length)`
returns a shallow copy and accepts negative indices. `splice(start, deleteCount)`
removes `deleteCount` elements beginning at `start` (negative `start` counts from the end)
and returns an array of the removed elements. `fill(value)`, `copyWithin`, and
`reverse` mutate in place and return the receiver. `concat` produces a new
same-element-type array. Strings provide UTF-16 code-unit `length`,
`charCodeAt`, and `charAt`, plus `includes`, `startsWith`, `endsWith`,
`indexOf`, `lastIndexOf`, `repeat`, `toUpperCase`, `toLowerCase`, `trim`,
`trimStart`, `trimEnd`, `search(regex)`, `slice(start, end?)`,
`replace(regex, replacement)`, `split(separator)`, and `codePointLength()`
(Unicode scalar count; JavaScript emits `[...s].length`). This
matches JavaScript string indexing while native storage uses UTF-8 plus WTF-8
for lone surrogate code units produced by `charAt`.
`charCodeAt` returns `0` for an out-of-range index. `charAt` returns an empty
string out of range, otherwise a one-code-unit string.
`Regex` provides `test`, `exec`, readable flag/source metadata, and mutable
`lastIndex`. Calls are statically checked
and are intrinsic optimization candidates; they are not untyped JavaScript
dispatch.

Non-mutating typed array, string, and `Math` operations are pure LilScript
language operations, even when JavaScript output uses a compact built-in
spelling. Their behavior is derived from the receiver's static type rather than
from `JsValue` dispatch. Array and typed-array mutators still carry their precise
receiver-mutation effect, and effectful callbacks remain effectful.

Every core typed-array view provides checked same-kind `set(source, offset =
0)`, plus fluent `fill(value, start = 0, end = length)` and
`copyWithin(target, start, end = length)`. Overlapping source and destination
ranges use snapshot/memmove semantics on every backend. Cross-kind `set` is
rejected until an element-wise conversion contract is implemented.

Integers provide `toString(radix = 10)` for signed output and
`toUnsignedString(radix = 10)` for the unsigned 32-bit bit pattern. Radices from
2 through 36 are supported identically by JavaScript and native targets.

Floats provide optimizer-known `abs()`, `floor()`, `ceil()`, `round()`,
`sqrt()`, `sin()`, `cos()`, `acos()`, `exp()`, `log()`, `tan()`,
`atan2(other)`, `hypot(other)`, `min(other)`, and `max(other)` methods.
`toInt()` applies JavaScript `ToInt32` conversion. These lower to compact
JavaScript operators or `Math` operations and equivalent native C operations.

Maps provide `size`, `get`, `set`, `has`, `delete`, and `clear`. Sets provide
`size`, `add`, `has`, `delete`, and `clear`. `set` and `add` return their
receiver for chaining. Binary-memory operations are the typed intrinsics listed
in the Types section rather than arbitrary JavaScript property dispatch.

String `+` accepts strings, numbers, and booleans. Template strings evaluate
embedded expressions left to right and apply the same string conversion rules.

The `print(value)` intrinsic is the portable observable-output operation used
by examples and backend equivalence tests.

## Compiler conformance

This contract defines behavior, not a required optimization pass list. Optimized
and optimizer-disabled paths must preserve these semantics. Backend-specific
representations and target contractions may differ only where this contract and
the selected boundary permit them.

The implemented pipeline is documented in
[current architecture](knowledge/compilation/current-architecture.md). The
Closure responsibility comparison is
[optimization-coverage.md](optimization-coverage.md). Project-wide completion
criteria and current state live in [roadmap](roadmap.md) and
[current status](current-status.md), not in the language semantics contract.
