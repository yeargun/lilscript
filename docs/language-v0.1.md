# LilScript Language Contract v0.1

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
- `//` line comments and `/* ... */` block comments are ignored.
- Statements end with `;`, except blocks and declarations ending in `}`.
- Decimal integer and IEEE-754 decimal float literals are supported.
- Strings use double quotes. Template strings use backticks and `${expr}`.

## Types

| LilScript type | Meaning | JavaScript representation | Native representation |
| --- | --- | --- | --- |
| `int` | signed 32-bit integer with operator-defined overflow behavior | number with i32 normalization | `i32` |
| `float` | IEEE-754 binary64 | number | `f64` |
| `bool` | `true` or `false` | boolean | C11 `bool` |
| `string` | immutable UTF-8 text | string | runtime string handle |
| `T[]` | mutable homogeneous array | optimized array representation | runtime array handle |
| `Map<K, V>` | mutable insertion-ordered key/value collection | native `Map` | tagged-value map handle |
| `Set<T>` | mutable insertion-ordered unique-value collection | native `Set` | tagged-value set handle |
| `ArrayBuffer` | fixed-length byte storage | native `ArrayBuffer` | owned byte-buffer handle |
| `SharedArrayBuffer` | fixed-length storage shared by views | native `SharedArrayBuffer` | shared-designated byte-buffer handle |
| `Uint8Array` | unsigned byte view | native `Uint8Array` | byte-buffer view handle |
| `T?` | either a `T` value or `null` | `T` or raw `null` | tagged `LilScriptOptional` |
| `A \| B` | value belonging to either member type | raw member value | tagged `LilScriptValue` at union boundaries |
| `struct S` | positional value aggregate | scalars, tuple, or boundary object | positional C value record |
| `class C` | nominal reference value with methods | dissolved record or class at an escaping boundary | pointer to a C record |
| `extern class C` | typed JavaScript host object interface | existing host object with exact member names | unsupported without an explicit user ABI |
| `func(T...)->R` | callable value | function/closure | function plus environment |
| `C<T...>` | applied generic class | same nominal class layout | pointer with boxed polymorphic fields |
| `void` | no value | no value | no value |

`auto` is a declaration inference marker, not a runtime type. It is legal only
for a local or top-level variable with an initializer.

An `int` widens implicitly to `float`. Other conversions require an explicit
standard conversion function. Arrays and nominal types do not implicitly
coerce.

`Map<K, V>` and `Set<T>` are mutable and invariant. `Map.get(key)` returns
`V?`; missing keys and stored `null` values therefore have the same result, as
they do after JavaScript lowering with `?? null`. Collection keys use
SameValueZero for floats and identity for reference types. Struct keys and set
elements are rejected because structs have value semantics and no portable
identity contract yet. Native collection storage currently uses deterministic
linear lookup; this is a correctness baseline that a later representation pass
may specialize without changing source semantics.

`ArrayBuffer` and `SharedArrayBuffer` accept one `int` byte length.
`Uint8Array` accepts a byte length or either buffer type, supports indexed reads
and writes, and exposes `length`, `byteLength`, `byteOffset`, and `buffer`.
`slice(start, end)` copies; `subarray(start, end)` creates a zero-copy view. The
`end` argument defaults to the view or buffer end. Indexed stores coerce modulo
256. Assignment and prefix-update expressions still evaluate to the numeric
value before storage coercion, matching JavaScript typed-array behavior. This
increment deliberately
does not expose resizable/growable options, `DataView`, other typed arrays, or
`Atomics`. JavaScript `SharedArrayBuffer` availability remains a host concern;
the ECMAScript host may omit its global constructor, and sharing it across web
agents requires the browser's isolation policy. Native lowering preserves
shared view identity in one process but does not yet claim concurrent or atomic
memory semantics.

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
global and member names are never mangled. Host property reads and methods are
effectful unless a method has a trusted `pure` contract. The C and native targets
reject host-object access because the Web platform has no portable C ABI. See
[web-platform.md](web-platform.md) for the complete implemented boundary and
current Web IDL limitations.

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
native output. JavaScript may omit an i32 coercion when range analysis proves it
redundant, while performance-first retains eager normalization for numeric hot
paths. Float arithmetic follows IEEE-754 binary64 behavior.

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
```

Only explicitly exported top-level functions, variables, structs, classes, and
externs can be imported. Imported names may be aliased with `as`. Module-private
bindings are namespaced by the linker, so equal private names in different files
cannot collide.

Imports must begin with `./` or `../`, resolve to `.lil` files, and form an
acyclic graph. Every module is initialized once in dependency-first order.
Side-effect-only imports therefore preserve initialization behavior.

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
Lazy imports and runtime chunk loading remain outside the language contract
because LilScript does not yet define a dynamic import expression.

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

Structs and classes that do not escape are eligible for scalar replacement.
Non-escaping class calls are statically devirtualized. Crossing `extern`
materializes the boundary representation. JavaScript uses named object fields;
native C uses generated positional value records for structs and pointer
records for classes.

## Functions and callable values

Functions use type-first declarations. Parameters are also type-first.

```lilscript
int add(int left, int right) {
  return left + right;
}

auto increment = (int value) => value + 1;
```

Functions and typed arrows are first-class values. Closures capture local
lexical values when the closure is created. Captured bindings are read-only
inside the closure, while objects and arrays referenced by a capture remain
mutable. Top-level bindings are shared globals rather than closure captures.
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
error.

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
- `break` and `continue`;
- `return`.

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

## Standard library surface

Arrays provide typed `length`, `map`, `filter`, `reduce`, `forEach`, `push`, and
`pop`. Callback methods snapshot the receiver length when the call begins, so
elements appended by a callback are not visited by that call. Reads of existing
future elements remain live, matching JavaScript's dense-array iteration
behavior. Strings provide UTF-16 code-unit `length` and `charCodeAt`, plus
`includes`, `startsWith`, `endsWith`, `toUpperCase`, and `toLowerCase`. This
matches JavaScript string indexing while native storage remains UTF-8.
`charCodeAt` returns `0` for an out-of-range index. Calls are statically checked
and are intrinsic optimization candidates; they are not untyped JavaScript
dispatch.

Integers provide `toString(radix = 10)` for signed output and
`toUnsignedString(radix = 10)` for the unsigned 32-bit bit pattern. Radices from
2 through 36 are supported identically by JavaScript and native targets.

Floats provide optimizer-known `abs()`, `floor()`, `ceil()`, `min(other)`, and
`max(other)` methods. They lower to the corresponding `Math` operations in
JavaScript and equivalent C math operations in native output.

Maps provide `size`, `get`, `set`, `has`, `delete`, and `clear`. Sets provide
`size`, `add`, `has`, `delete`, and `clear`. `set` and `add` return their
receiver for chaining. Binary-memory operations are the typed intrinsics listed
in the Types section rather than arbitrary JavaScript property dispatch.

String `+` accepts strings, numbers, and booleans. Template strings evaluate
embedded expressions left to right and apply the same string conversion rules.

The `print(value)` intrinsic is the portable observable-output operation used
by examples and backend equivalence tests.

## Whole-program optimization requirements

The optimized IR pipeline must run, in a fixed-point schedule where relevant:

1. constant folding and propagation;
2. branch simplification and unreachable-block removal;
3. escape analysis;
4. scalar replacement of aggregates;
5. call-graph analysis and dead function elimination;
6. function and method inlining;
7. class method devirtualization;
8. dead instruction and dead binding elimination;
9. representation selection;
10. frequency- and compression-aware symbol assignment.

These requirements apply after the complete static module graph has been
linked, so propagation, inlining, devirtualization, scalar replacement, effect
analysis, and DCE operate across source-file boundaries.

The detailed Closure `ADVANCED` responsibility mapping and pass schedule are in
[optimization-coverage.md](optimization-coverage.md).

JavaScript emission consumes optimized IR only. Backend-specific peephole
rewrites may run after IR optimization but may not change LilScript semantics.

## Completion criteria

The v0.1 implementation is complete only when:

- every construct in this contract has parser, semantic, positive, negative,
  and runtime tests;
- JavaScript and native executions agree on the conformance suite;
- the browser playground compiles and runs LilScript examples and reports source
  diagnostics;
- the benchmark suite executes equivalent LilScript and JavaScript programs,
  verifies their output, invokes Closure Compiler in `ADVANCED` mode, and
  reports raw, gzip, and Brotli sizes from fresh release builds.
