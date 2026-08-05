# LilScript Language Contract v0.1

## Identity and compilation model

LilScript is an independent statically typed language. LilScript source is never
parsed as JavaScript or TypeScript, and neither language defines LilScript
semantics. JavaScript and native object code are backend targets of the same
typed whole-program IR.

Every executable program is analyzed as a closed world. An explicit `extern`
declaration is required to cross into an untyped JavaScript boundary. Values
that reach an `extern` boundary are considered escaping and must use the
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
| `int` | signed 32-bit integer with two's-complement wrapping | number with i32 normalization | `i32` |
| `float` | IEEE-754 binary64 | number | `f64` |
| `bool` | `true` or `false` | boolean | C11 `bool` |
| `string` | immutable UTF-8 text | string | runtime string handle |
| `T[]` | mutable homogeneous array | optimized array representation | runtime array handle |
| `struct S` | positional value aggregate | scalars, tuple, or boundary object | positional C value record |
| `class C` | nominal reference value with methods | dissolved record or class at an escaping boundary | pointer to a C record |
| `func(T...)->R` | callable value | function/closure | function plus environment |
| `void` | no value | no value | no value |

`auto` is a declaration inference marker, not a runtime type. It is legal only
for a local or top-level variable with an initializer.

An `int` widens implicitly to `float`. Other conversions require an explicit
standard conversion function. Arrays and nominal types do not implicitly
coerce.

Integer arithmetic wraps to signed 32-bit two's-complement values. Integer
division truncates toward zero; division or remainder by zero produces `0` on
every backend. Float arithmetic follows IEEE-754 binary64 behavior.

## Declarations

```lilscript
int count = 5;
float ratio = 3.14;
string name = "LilScript";
bool enabled = true;
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
produce JavaScript bindings. The default backend emits one optimized application
bundle; lazy imports and runtime chunk loading remain outside this contract
because forced chunks add boundaries and bytes to fully static programs.

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

Assignments support `=`, `+=`, `-=`, `*=`, `/=`, and `%=`. Prefix `!` and `-`,
postfix calls/member/index access, and the standard arithmetic, comparison,
equality, and short-circuit logical operators are supported. Assignment is an
expression and evaluates to the assigned value.

## Standard library surface

Arrays provide typed `length`, `map`, `filter`, `reduce`, `forEach`, `push`, and
`pop`. Strings provide `length`, `includes`, `startsWith`, `endsWith`,
`toUpperCase`, and `toLowerCase`. Calls are statically checked and are intrinsic
optimization candidates; they are not untyped JavaScript dispatch.

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
