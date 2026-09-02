# 042 — owned bags are still `JsValue`

**Status: OPEN — can jquerylil's five compiler-owned data bags be re-typed so their reads stop
blocking fusion, with no new syntax, for ≤ −80 Brotli?**
Lane: port. Objective: brotli. Ports: jquerylil. Opened: 2026-09-02.

## Prior art

Harvest of 2026-09-02, rows in [refs/competitor-techniques.md](../../refs/competitor-techniques.md)
Section D ("Property-read purity gate", "Object literal → scalars") and Section H #24–#26.

- **Terser** treats a property read as pure only when the receiver is provably non-nullish by
  syntax or by `reduce_vars`' `fixed_value` (`lib/compress/inference.js:756-830`); its default
  `pure_getters: "strict"` (`lib/compress/index.js:261`) already *waives* getters, so 013's
  "Terser has it off too" is imprecise — our default equals Terser's `pure_getters: false`.
  `hoist_props` dissolves a literal only when it never escapes whole, is never reassigned, every use
  is `o.key` of a plain key–value, and no accessor is present (`index.js:883-948`,
  `reduce-vars.js:228-270`, `common.js:207-227`).
- **Oxc** defaults to `PropertyReadSideEffects::All` (`oxc_ecmascript/side_effects/context.rs:7-13`,
  `oxc_minifier/options.rs:221`); purity only for known globals, literal `.length` and in-bounds
  literal indexes (`expressions.rs:460-520`); no per-value tracking and no scalar-replacement pass —
  only an unread fresh literal without accessors dissolves (`remove_unused_expression.rs:814-850`).
- **Closure** (not vendored — verify): proof per *name*. `GatherGetterAndSetterProperties` builds an
  accessor summary; `CollapseProperties` needs one global assignment, no aliasing reads and no
  accessor of that name; `InlineObjectLiterals` needs a local accessor-free literal used only as
  `v.prop`.
- **LilScript** decides observability by the receiver's *type*: `IndexGet` on `JsValue` is the only
  observable read (`codegen_ir_js.rs:25198-25201, 25338`); `FieldGet`/`RecordFieldGet`/
  `HostFieldGet` are never observable (`:25267`). But no member read is deferrable (`:24703-24750`),
  so every read is `unstable` and lives only by fusion (`:24444-24468`; `HostFieldGet` is an explicit
  blocker at `:24466`). 013's flag saved 540 by letting *dynamic* reads stop blocking each other,
  which wrongly includes DOM reads (`elem`/`event`/`xhr`/`win`: 206 of jquery's 1344 string-key
  reads) that stay blockers under any honest type. The type ceiling is therefore below 540. What
  none of the three tools has: our escape graph already knows which allocations are compiler-owned
  and data-written, and a declared data type carries that proof through the port's public API.

## Claim

Five bags jquerylil allocates and writes itself — `hooks`, `specialForType`, `support`, `tween`,
`opts` (115 reads) — are typed `JsValue` and read through `IndexGet`, so each read is a fusion
blocker and each blocked value takes its own named statement (019). Re-typed as `struct` or
`Record<JsValue>`, the reads become `FieldGet`/`RecordFieldGet` and stop blocking. Confirms:
**≤ −80 Brotli** on `dist/jquery.esm.js` under the port's config and **≥ 60 of the 405 surplus
stores** (013) gone, with the port's tests and the 040 `animate` smoke passing. Falsifies: **≥ 0**,
the 021 regression again — then the reason (which helper layer re-boxes the value) is the finding,
and the two follow-ups need language: a typed ordinary-prototype dictionary `object<T>` for
`ajaxSetup` (104 reads, ≤ −150 / > −40) and a callable `object` for `jQuery`/`jQuery.fn` (263 reads,
20% of the port's, ≤ −200 / > −60).

## Read

- `finer/objective.md`, `finer/status.md`, this folder
- [013](../013-statement-density/README.md) and [021](../021-reflective-ffi-predicts-loss/README.md): the mechanism and the two regressions of partial typing
- `docs/language-v0.1.md` — `struct`, `Record<T>`, `JsValue`, and what escapes
- `../jquerylil/src/effects.lil`, `css.lil`, `ajax.lil`, `support.lil` (or wherever the five bags are declared: `grep -rn "hooks\|specialForType\|support\|tween\|opts" ../jquerylil/src/*.lil`)

## May touch

- `../jquerylil/src/*.lil` (the five bags and the helpers that receive them); this folder; `finer/out/042/`
- Not the compiler, not `lilscript.toml`

## Method

Host: one hypothesis at a time. jquerylil's level-15 `always` build takes 15–25 min on four cores;
check `pgrep -f "release/lilscript"` before starting one and never share cores with a fleet pass.

1. Baseline: the tree build of the same commit (`finer/out/041/` will hold one built by the fixed
   compiler; otherwise build once). Count surplus stores as 013 did.
2. Re-type one bag at a time, smallest first; build; measure with `./target/release/lilscript-codec
   --json`; run `../jquerylil` tests plus `node -e` `animate` smoke (039/040). Record each step so a
   regression names its bag.
3. Report the five deltas and the stores gone; the Verdict decides which of the two follow-ups, if
   any, becomes a `lang` folder.

## Result

| step | bag | raw | gzip9 | brotli11 | Δ brotli | surplus stores | tests |
|---|---|---:|---:|---:|---:|---:|---|
| baseline | — | | | | 0 | 405 | |

## Verdict

<open>

## Next

<open>
