# 033 — Class member bodies that were not scopes, and 018's ten classes

**Status: FIXED. mobxlil emits its ten `class` declarations for the first time — −6786 raw, −235
Brotli — and no other port moves.**

## The chain

[032](../032-export-resolver-false-negative/README.md) established that admission refuses mobxlil's
class-bearing artifact for an `unresolved generated export binding` that node, esbuild and an actual
`import` all resolve. `Resolution` has three states and the export check demands `Bound`; probing the
real artifact returned the third, `Unresolved`, which `binding.rs` documents as *"the scanner could
not account for this scope."*

Why a scope goes unaccounted-for turned out to be simple, and the damage indirect:

> A member body the scanner does not recognise is not a scope, so the `var` declarations inside it
> attach to the nearest scope that *is* recognised. For a method on a top-level class, that is the
> **module**. A name declared both there and inside the method becomes ambiguous, every use of it
> resolves `Unresolved`, and exporting that name is refused — discarding the whole artifact.

## What was not recognised

`function_scope_at` accepted method shorthand only when the token before the name was `{`, `}`, `;`,
`,` or `async`. Probing each spelling against a fixture that declares the same name inside and
outside the member:

| member spelling | before | after |
|---|---|---|
| `m(){...}` | ok | ok |
| `constructor(){...}` | ok | ok |
| `async m(){...}` | ok | ok |
| `get p(){...}` | **FAILED** | ok |
| `set p(v){...}` | **FAILED** | ok |
| `static m(){...}` | **FAILED** | ok |
| `*m(){...}` | **FAILED** | ok |
| `[Symbol.iterator](){...}` | **FAILED** | ok |
| `["m"](){...}` | **FAILED** | ok |
| `{get p(){...}}` in an object literal | **FAILED** | ok |
| **`delete(e){...}`** | **FAILED** | ok |

Two separate gaps. Modifiers and computed names were simply not walked over. The last row is the one
mobxlil actually hits: **a member named with a reserved word.** `delete(e){...}` is a legal method
and `is_binding_identifier` rejects `delete`, so its body was never a scope and its `var i` landed in
the module — colliding with a genuine top-level `i` and poisoning the export.

## The fix

Walk back over `static`, `get`, `set`, `async` and a generator `*` to find the member boundary;
accept a computed `[...]` name; and accept a keyword as a member name.

The keyword case needs a guard, because `if(x){...}` at statement level has the *same token shape* as
a keyword-named method. Only the keywords that introduce a parenthesized control header are excluded
— `while`, `for`, `if`, `switch`, `catch`, `with`, plus `function` and `class`. Operator keywords
need no exclusion: `delete(x){` is not valid JavaScript, so the required `{` body already turns them
away. A computed *call*, `o[k](n)`, is turned away by the delimiter test, since its `[` follows an
operand rather than a member boundary. Both are pinned by tests.

## Result

| | raw | Brotli | `class` |
|---|---:|---:|---:|
| mobxlil before | 63493 | 15943 | **0** |
| mobxlil after | **56707** | **15708** | **10** |

**Ten classes**, which is exactly what [018](../018-mobx-admission-regression/README.md) measured in
the last good build before the regression, and −6786 raw for −235 Brotli.

The Brotli gain is smaller than the 769 [032](../032-export-resolver-false-negative/README.md)
measured, and I first recorded that as an unexplained gap. **It is not one, and the claim was wrong.**
032's 769 was the distance from the *old* artifact (63493 raw, zero classes) to a pipeline pass over
it; the fix has now captured 6786 of that 7049 raw. Re-running the pipeline over the **new** artifact
converges in two rounds and finds only:

| | raw | Brotli |
|---|---:|---:|
| mobxlil as it now ships | 56707 | 15708 |
| pipeline run to convergence | 56514 | 15685 |
| **remaining headroom** | **−193** | **−23** |

So 23 Brotli, not 534. The compiler is now within a rounding error of what its own peephole reaches
on this port, and there is no second mechanism hiding here.

No other port moves: micromarklil 26314, markedlil 9506 (29/29 tests), hast-util-to-htmllil 8825
(456/456). 1636 compiler tests pass. mobxlil's own suite is unchanged at 15 failures — verified
pre-existing by rebuilding with the fix stashed and getting the identical count.

## What this closes

018 asked why ten `class` declarations vanished, instrumented admission, found *150 validations, 0
rejections*, and concluded they were "never generated or scored". They were generated every time. Two
validators refused them, at gates 018 never counted: the property census objecting to a class's own
`constructor` ([031](../031-admission-blocks-the-class-rewrite/README.md)) and the binding resolver
losing a name to an unrecognised member body (here). Both are now fixed.
