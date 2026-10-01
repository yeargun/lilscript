# Functions, closures, and generics

Parent: [language](README.md). Contracts: [functions](../../language-v0.1.md#functions-and-callable-values)
and [types](../../language-v0.1.md#types). Compiler anchors:
`src/check.rs` (`infer_type_arguments`, parameter-default validation),
`src/program/from_source.rs` (closures and their captured cells), and the
helper and product families plus the JavaScript target's inliners in
`src/program/` and `src/js/`.

Functions use type-first parameters and returns. Typed arrows are first-class.
Non-`void` functions must return on every path. Callable types require parentheses
when combined with postfix types, for example `(func(int)->int)[]`.

Closures capture lexical **bindings**, not snapshots. Reassignment through one
closure is visible to siblings. Elaboration records captures explicitly as shared cells; JavaScript can
inline or share a closure only when identity and escape remain correct, while native
uses a function-plus-environment representation.

Trailing parameters may have checked defaults. Defaults are materialized by the
checked call contract for the shared full-arity language ABI; the JavaScript
target may spell them as native default parameters where that is equivalent. A nested callable may default from an outer
local; that default is a capture evaluated when the argument is omitted. Reference
defaults allocate once per omitted call. Exported JS functions preserve public
omitted-call behavior and `Function.length`. Parameter defaults still cannot
reference a later parameter of the same callable, including a same-named inner
parameter that shadows the outer binding.

Generic functions and classes are statically instantiated by constraints inferred
from values, callbacks, constructors, and expected types. JavaScript erases the type
arguments. Native boxes abstract parameters at generic boundaries. The compiler
does not pretend a polymorphic body is concrete: substitution must be known before
type-dependent inlining.

Private generic bodies can transport structs through branches, recursive
helpers, arrays, maps, nested generic structs and captured closures. The shared
transport check follows the closed typed interface; it does not require the
function to return one unchanged parameter. Concrete owned callback arguments
are admitted when the complete call set proves their origin. The check grants
neither purity nor permission to inline, and optional optimization vetoes do not
disable this language support.

An abstract value that could contain a struct still cannot escape through
`JsValue`, `unknown`, unqualified host storage or an unknown callable. Such a
crossing needs a concrete public codec: exposing the private product backing
would break value snapshots. Public opaque generic crossings and aliased mutable
host collections remain migration work.

An exported function may accept a callback with concrete struct parameters or a
struct result when it only calls that callback. Local aliases, defaults and
rest arguments keep the same rule. The boundary adapter encodes arguments and
snapshots returned fields once; exceptions propagate. Development checks validate
the original callable and returned fields before conversion. A callback that
escapes, is captured or has its identity observed still needs a wider ABI and is
refused; a fresh wrapper would change its meaning.

## Optimization contracts

- Interprocedural effects decide whether an unused call may disappear; `pure` is a
  checked assertion, and `pure extern` is a trusted host promise. Both wait for
  the effect fact (plan M6.2, M6.3, M7.2).
- Direct calls and statically known methods/closures may devirtualize.
- Inlining is exact where it removes operations and a codec-judged choice where
  its value depends on printed length (plan M7.5).
- Constant-parameter and constant-capture clones re-enter folding and DCE on
  complete call sets (plan M7.3, M7.5). Profile-guided cloning is removed.
- Identical, permuted and one-constant function folding apply only to proven
  compatible private functions, as codec-judged choices (plan M9.9). Public
  identity/arity/constructibility remain roots.

An exported function keeps the callable kind its source declares (D2): a declared
function stays constructible, and arrows lose construction and `prototype`. The
old `javascript.function_spelling` key is retired; the compiler chooses the
spelling of private functions itself.
