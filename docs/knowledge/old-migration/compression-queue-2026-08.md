# Archived compression queue (2026-08)

> Archived on 2026-08-13. This page preserves the former jQuery-focused queue and
> its historical measurements. It is not the active plan. Start at
> [the migration index](../migration/README.md); archive context is in
> [the archive index](README.md).

## Original document

Parent: [knowledge tree](../README.md). Evidence: [jQuery](../evidence/jquery.md),
[Closure](../evidence/closure-comparison.md). Philosophy:
[global optima](../compilation/global-optima.md).

This is the working queue for getting closer to a **global** gzip/Brotli optimum — especially on jQuery — without treating “locally shorter JS” as a win.

A step that does not shrink the current jQuery artifact is not automatically wrong. It may be (a) correct infrastructure, (b) a win on other programs, (c) blocked by port shape so the compiler never sees a legal rewrite, or (d) a raw win that loses Brotli. Record the measurement and keep or revert with a reason.

## How to judge a step

1. **Same observable behavior** (tests, jquery verifiers). Changing *how* the code works is allowed.
2. Measure the **same lane**: `lilscript.app.toml` (closed mangle) vs `lilscript.toml` (public names). Do not mix them.
3. Report raw / gzip-9 / Brotli-11 of the **measured** artifact (`measure-jquery.mjs` or the terser-min lib). Served bytes are the objective.
4. If raw drops and Brotli rises, keep the old spelling unless the project `cost_model` is `raw`.
5. Do not post-minify LilScript output with Terser/Oxc/Closure as a “free” last pass. That has already worsened Brotli on real rows.

## What is actually leaving bytes on the table

jQuery 3.7.1 npm min is **87533** raw / **~27 KiB** Brotli. Best LilScript densify checkpoint is **~95038** terser raw / **~35 KiB** Brotli. The remaining ~7.5 KiB raw is not one missing peephole.

| Bucket | Why it costs | Who can fix it |
|---|---|---|
| `JsValue` + `setProp`/`getProp` bags on the public API | Field names stay strings; no positional dissolve | Port reshape + thin facade |
| ~230 extra functions vs npm | bindMethod adapters, class+facade twins, host wrappers | Port + compiler sharing |
| Host TS wrappers still imported | `mathRound`, `dateNow`, `parseFloatValue`, `objectHasOwn`, … are real functions esbuild keeps | Compiler known-extern → JS builtin (codec-scored) |
| `this`-method vs `bindMethod` | Local “more native” spelling **regressed** terser | Do not blindly convert; score complete artifact |
| Aggressive inlining | `lean-inline-96` worse than `lean-balanced` | Do not raise default inline caps |
| `print` → `console.log` | Language oracle; should not ship in a library | Config `strip_console` (this file, step 1) |
| Closure `ADVANCED` on jquery | Lane not run; public API must stay | Later, same boundary as npm |

Terser/Oxc/Closure tricks we should **steal only when they are proofs + codec-scored**:

| Trick | Source | LilScript stance |
|---|---|---|
| `drop_console` | Terser | Step 1. Strip `print` / `debugLog`. **Keep** `console.warn` (jQuery exceptionHook is observable). |
| `pure_funcs` | Terser | Already: inferred/`pure`/`pure extern`. |
| `unsafe`: `new Object()`→`{}`, `String(x)`→`x+""` | Terser | We already emit `{}` / `x+""` for `JS.object` / `JS.string`. Extend to **named host externs** with the same JS meaning (step 2). |
| `unsafe_comps` (flip `<`/`<=`) | Terser | Only as a complete-artifact candidate. Can change `valueOf`/`NaN` order. |
| `void 0` vs `undefined` | Terser `unsafe_undefined` | Already `void 0` for `JS.undefined`. |
| Known method inlining | Oxc / Closure | Same as step 2. **Use-count matters**: `Math.round` × 50 can lose to a 1-char helper. |
| Collapse properties / disambiguate | Closure | We delete names via indexes. Remaining cost is `JsValue` bags, not missing rename. |
| Repeated simplify↔inline loop | Closure / Oxc | We already re-inline after compress. More fixed-point is P2, not the jquery gap. |
| Frequency rename + occurrence-order ties | Closure `RenameVars` | Already have frequency + layout search. |
| Strip `goog.debug` / loggers | Closure StripCode | Step 1 analogue. |

## Clear wins (do these first)

### Step 1 — `javascript.strip_console` (policy, not a search)

**What.** Production JS must not emit `print()` as `console.log`, and must drop `debugLog`. Argument side effects stay (Terser `drop_console` that deletes `print(foo())` is a behavior change; we do not do that).

**What we do not strip.** `console.warn` / `consoleWarn3` — jQuery’s exception hook is part of the library contract.

**Default.** Compiler / no-TOML default is **on**. `print` is the language test oracle (`verify-matrix`, unit `compile()`), so the root `lilscript.toml` and unconfigured `compile_program_to_js` paths set **off**.

**Success.** Flag works; jquery verifiers still pass; size may be ~0 on jquery (the port barely uses `print`). That is still a correct step.

### Step 2 — Known host externs → JS builtins (same output)

STATUS claims `createEmptyObject`→`{}`, `createArray`→`[]`, `callN(f,null,…)`→direct call, rare DOM getters→`.prop`. The port already uses `JS.object` / `JS.string` / `JS.typeOf` for much of this. Remaining **imported** wrappers in the current raw artifact include `mathRound`, `mathCeil`, `mathMax`, `dateNow`, `parseIntRadix`, `parseFloatValue`, `isFiniteValue`, `encodeURIComponentValue`, `mathCos`, `mathPI`, `objectHasOwn`, `getPrototypeOf`, `objectCreate`, `newRegexp`.

Rewrite `CallDirect` to those externs into existing or new JS intrinsics **in IR**, so foreign-import DCE drops the TS wrapper from the esbuild bundle.

**Global-optimum rule.** Do not always inline. A shared 1-char helper can beat `Math.round` at high use count. First implementation: always-lower only when the builtin spelling is short **or** use count ≤ 4 (same spirit as the documented DOM-getter rule). Later: compete helper vs builtin under `cost_model`.

**Success.** Same jquery verifiers. Expect a real drop if wrappers leave the bundle. If Brotli is flat, keep the pass (raw/helper trade) and note it.

**Shipped.** IR rewrite of known host `CallDirect`s. Always-inline short spellings (`{}`, `[]`, `typeof`, `x+""`, `Object.create(null)`, `Math.PI`, `jsAssume` erase, `obj[k]`, `.length=`, `void 0`, DOM `.prop` / `.method()`, `throw` / `throw new Error`). For `Math.*` / `Date.now` / `parse*` / `Object.getPrototypeOf` / `Object.create` / `RegExp` / `Array.prototype.*`: inline when use count ≤ 4 and the function is not taken as a value; otherwise emit a mangled `const a=Math.round` (or `const a=Array.prototype,b=a.push` when several methods share a root) and drop the TS import. `objectHasOwn` / `toString` aliases use `.call`. `isFunctionValue` stays a helper (jQuery excludes `nodeType`/`item`). `console.warn` is not stripped.

### Step 3 — Stop paying for dual class + `JsValue` facade (port)

Deferred / Callbacks / jqXHR still have a typed object **and** a string-key bag. One representation, one thin export shim for the public API. This is the largest *legal* size move that does not require new compiler theory.

Do it module by module (Callbacks, then Deferred, then jqXHR). Verify after each.

### Step 4 — Shrink `bindMethod*` / `call*` density without `this`-method religion

`this`-methods already **regressed**. Prefer: one shared adapter where many methods share a shape; `bindMethod`+arrow when that is the only copy; compiler calling-convention elision (already exists) on more sites.

Measure after each cluster (wrap/showHide is the known walk-back).

## Later / smaller (do not start until 1–4 are done)

5. **Codec-scored comparison flip** (`a<b` vs `b>a`) — Terser `unsafe_comps`, complete artifact only.
6. **Joint mangling + string-pool + chunk layout** — already on the roadmap; compile-time heavy.
7. **Relational SCCP / memory SSA** — more DCE, not jquery’s main tax.
8. **Region outlining default** — keep off; helpers often lose gzip/Brotli.
9. **Raise inline caps** — jquery audit says no.
10. **Closure ADVANCED jquery lane** — only with the same public names as npm.
11. **Cross-function substring dictionaries** — measure-first, easy to hurt Brotli context.

## Implementation log

Measured with `lilscript.toml` (public) and `lilscript.app.toml` (app), esbuild bundle + Terser `compress.passes=3`. npm `jquery.min.js` is 87533 / gzip 30342 / Brotli **27445**.

| Step | Status | app terser raw | app terser brotli | public terser brotli | Notes |
|---|---|---|---|---|---|
| baseline (pre-this-queue) | recorded | ~97836 bindMethod restore / ~95038 densify | ~35 KiB popular lab | see RESULTS.md | Not an eligibility win |
| 1 strip_console | done | — | — | — | Compiler default **on**. Root `lilscript.toml` + unconfigured `compile_program_to_js` set off for the `print` oracle. Size ~0 on jquery. |
| 2 known-extern builtins | done | 102502 | 31722 | 31761 (public terser raw 98432) | Dropped `mathRound` / `dateNow` / `parse*` / `Object.*` / `RegExp` / `Math.PI` from `js-host.ts` imports. High-count sites emit mangled `const a=Math.max`. Keep: use-count gate, not always-inline. Flat vs lean-balanced ~98.5k is expected, not a revert. |
| 2b host methods / throws / prototype roots | done | 102839 | 31933 | 31757 (public terser raw 98452) | Closure-style: `.call`/`.apply` aliases, clustered `const a=Math,b=a.max`, `throw`/`throw new Error`, `void 0`, `obj[k]`, `.length=`, DOM `.prop`/`.method()`. MethodCall aliases are skipped when the extern is taken as a value (`var/push.lil` etc. keep the TS helper — correct). Public Brotli flat vs 2. App +~200 Brotli: keep (legal rewrites, codec trade). |
| 2c bound methods + drop wrapper re-exports | done | 102547 | 31786 | 31645 (public terser raw 98176) | `BoundMethodCall` emits `a=hasOwnProperty,b=a.call.bind(a)` when a host method is a first-arg value. Low-count MethodCall inlines `Object.prototype.hasOwnProperty.call`; high-count aliases. jQuery dropped `var/push`/`hasOwn`/`toString` wrappers; `fn.push = arr.push`. Peephole hoists `console.log` out of if/else when raw-shorter (codec still scores both). Public −276 raw / −112 Brotli vs 2b. App −292 raw / −147 Brotli. Micro suite: 400/400 vs Terser/esbuild, 377 strict raw+Brotli wins. |
| 3 facade collapse | queued | | | | Still the largest *legal* port move |
| 4 bindMethod density | queued | | | | Do not revive this-methods |

Side fix while verifying: `speed()` used `opt = mergeObject(opt, speedArg, false)` after `opt = JS.object()`. `mergeObject` already mutates `opt`. The reassignment hit a coalescing miscompile (`var x=extend(x,…)`) that broke `animate` in development **and** production. Calling `mergeObject(opt, speedArg, false)` without reassignment matches jQuery.extend and unblocks `verify-jquery-effects.mjs`. Same class of bug may still exist at other `x = merge(x, …)` sites; do not “fix” it by raising inline caps.
