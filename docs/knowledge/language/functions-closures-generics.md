# Functions, closures, and generics

Parent: [language](README.md). Contracts: [functions](../../language-v0.1.md#functions-and-callable-values)
and [types](../../language-v0.1.md#types). Compiler anchors:
`src/semantic.rs` (`infer_type_arguments`, parameter-default validation),
`src/lower.rs` (arrow/capture collection), and specialization/inlining in
`src/optimizer.rs`.

Functions use type-first parameters and returns. Typed arrows are first-class.
Non-`void` functions must return on every path. Callable types require parentheses
when combined with postfix types, for example `(func(int)->int)[]`.

Closures capture lexical **bindings**, not snapshots. Reassignment through one
closure is visible to siblings. Lowering records captures explicitly; JavaScript can
inline or share a closure only when identity and escape remain correct, while native
uses a function-plus-environment representation.

Trailing parameters may have checked defaults. Defaults are materialized before SSA
for the shared full-arity language ABI; the JS emitter may recover native default
syntax as a scored representation. A nested callable may default from an outer
local; that default is a capture evaluated when the argument is omitted. Reference
defaults allocate once per omitted call. Exported JS functions preserve public
omitted-call behavior and `Function.length`. Parameter defaults still cannot
reference a later parameter of the same callable, including a same-named inner
parameter that shadows the outer binding.

Generic functions and classes are statically instantiated by constraints inferred
from values, callbacks, constructors, and expected types. JavaScript erases the type
arguments. Native boxes abstract parameters at generic boundaries. The optimizer
does not pretend a polymorphic body is concrete: substitution must be known before
type-dependent inlining.

## Optimization contracts

- Interprocedural effects decide whether an unused call may disappear; `pure` is a
  checked assertion, and `pure extern` is a trusted host promise.
- Direct calls and statically known methods/closures may devirtualize.
- Inlining obeys instruction/control-flow/growth limits and retains an outlined
  candidate when configured search permits.
- Constant-parameter, profiled call-site, and capture-signature clones re-enter
  folding and DCE. They are transforms inside complete optimizer variants, not
  independently codec-approved functions.
- Identical folding, subsumption, and parameterized merging apply only to proven
  compatible private functions. Public identity/arity/constructibility remain roots.

`javascript.function_spelling = "arrow"` is an ABI choice for public functions, not
mere minification: arrows lose construction and `prototype`. Reusable libraries
normally leave the key unset so public functions stay constructible.
