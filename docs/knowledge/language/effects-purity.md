# Effects and purity

Parent: [Language](README.md). Related: [escape](boundaries-escape.md), [future architecture §7 (facts)](../../future-architecture.md#7-facts).

## Inference first, `pure` as a contract

The contract: every function gets an interprocedural effect summary, its inherent effects plus which parameters it mutates, and the optional `pure` keyword **checks** that inference; it does not silence it. A `pure` function that prints, mutates globals/collections/aggregates, or calls effectful code is a compile error.

**Today** the one compiler does not have that engine. Its per-unit facts treat every user call as an unknown effect, and a declared `pure` is recorded but not checked. Plan task M6.2 builds the effect fact (seeded by declared `pure` and trusted `pure extern`), M6.3 restores the `pure` diagnostic, and M7.2 removes discarded effect-free calls. The deleted route had all three; its implementation is described in [history](../history/compilation/analyses.md).

`pure extern` is a **trusted host promise**. The compiler cannot see the host. `[lint].pure_extern_allowlist` exists because of that.

## What is effectful (conservative)

Host field reads/writes and non-`pure` host calls; `StoreGlobal`; `DynamicImport`; `Await`; `throw`; unknown `CallValue`; array callbacks whose callback is effectful; `Regex.test` (stateful lastIndex); collection mutations.

Property reads on `extern class` are effectful by default: a Web IDL getter may throw or mutate. That blocks DCE of “unused” DOM reads unless `pure` is declared.

### Typed intrinsics are not host dispatch

Non-mutating typed `Math`, string, and array operations are pure language
operations. The JavaScript target may spell one with `Math`, a string prototype
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
than trusted (from M6.3). DCE retains each evaluation, value numbering does not merge them,
and scheduling, helper substitution, and region transforms may not reorder them.
Non-coercive truthiness, `typeof`, strict comparison, and nullish tests do not gain
an effect merely because their value came from JavaScript.

## Why this is a compression feature

- Unused **pure** calls are deleted (from M7.2).
- Unobserved local mutation graphs (arrays/maps/sets that never escape or get read) are deleted.
- Inlining and specialization stay sound because effects on parameters are tracked.
- Observable evaluations remain ordered even when expression-oriented codegen or
  helper substitution would otherwise combine a region.

Without explicit effects, a TS/JS minifier must assume almost every call is live. LilScript can delete more **because** the language made effects checkable.

## Exception regions

`try`/`catch`/`finally` emit as structured JS `try` statements. The Program IR keeps a `Try` region, and locals a `catch` must observe stay mutable cells, so a catch sees every assignment completed before the operation that threw. A catch binding with no use is omitted (`catch{…}`) when the syntax floor is ES2019 or later.

## Config

- `[policy.tactics]` (`dead-code-elimination`, `inlining`, …) — what the compiler may do with effect facts; `off` is a veto
- `[lint]` `effects` provider, `pure_extern_allowlist`
- Host `pure` methods in source
