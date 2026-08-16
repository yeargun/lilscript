# Effects and purity

Parent: [Language](README.md). Related: [escape](boundaries-escape.md), [IR optimizer](../compilation/ir-optimizer.md).

## Inference first, `pure` as a contract

Every function gets an interprocedural effect summary (`FunctionEffectSummary` in `src/optimizer.rs`): inherent effects plus which parameters it mutates. The optional `pure` keyword **checks** that inference; it does not silence it. A `pure` function that prints, mutates globals/collections/aggregates, or calls effectful code is a compile error.

`pure extern` is a **trusted host promise**. The compiler cannot see the host. `[lint].pure_extern_allowlist` exists because of that.

## What is effectful (conservative)

Host field reads/writes and non-`pure` host calls; `StoreGlobal`; `DynamicImport`; `Await`; `throw`; unknown `CallValue`; array callbacks whose callback is effectful; `Regex.test` (stateful lastIndex); collection mutations.

Property reads on `extern class` are effectful by default: a Web IDL getter may throw or mutate. That blocks DCE of “unused” DOM reads unless `pure` is declared.

### Typed intrinsics are not host dispatch

Non-mutating typed `Math`, string, and array operations are pure language
operations. The JavaScript backend may spell one with `Math`, a string prototype
method, or an array primitive, but that target spelling does not turn the typed
operation into an unknown host call. Array/typed-array mutators still report the
receiver they mutate, and an intrinsic callback contributes its inferred effects.

The rule changes at an explicit `JsValue` boundary. Dynamic arithmetic, loose
equality, templates, or string concatenation may call
`Symbol.toPrimitive`/`valueOf`/`toString`; dynamic index/property operations may
run proxy traps; checks such as `JsValue.isArray()` can throw for a revoked proxy;
and explicit JavaScript conversions may throw on their dynamic inputs. Those are
observable evaluations even when their result is unused. Effect analysis marks
the containing function effectful, so a `pure` declaration is rejected rather
than trusted. DCE retains each evaluation, value numbering does not merge them,
and scheduling, helper substitution, and region transforms may not reorder them.
Non-coercive truthiness, `typeof`, strict comparison, and nullish tests do not gain
an effect merely because their value came from JavaScript.

## Why this is a compression feature

- Unused **pure** calls are deleted.
- Unobserved local mutation graphs (arrays/maps/sets that never escape or get read) are deleted.
- Inlining and specialization stay sound because effects on parameters are tracked.
- Observable evaluations remain ordered even when expression-oriented codegen or
  helper substitution would otherwise combine a region.

Without explicit effects, a TS/JS minifier must assume almost every call is live. LilScript can delete more **because** the language made effects checkable.

## Exception regions vs SSA

`try`/`catch`/`finally` emit as structured JS. Locals assigned in the `try` that a `catch` must observe are **not** promoted to exception-insensitive SSA. Exception-bearing functions are also excluded from CFG rewrites that cannot keep structured regions. That is a size/optimization limit accepted for correctness. `unused-catch-binding-elision` may still drop `(error)` when the binding has zero uses; the codec search keeps the explicit-binding variant because raw deletion can lose gzip/Brotli.

## Config

- `[optimization]` DCE / DSE / finite-value / path-sensitive propagation — what the optimizer is allowed to do with effect facts
- `[lint]` `effects` provider, `pure_extern_allowlist`
- Host `pure` methods in source
