# 042 — owned bags are still `JsValue`

**Status: FALSIFIED at the claimed size, confirmed in direction — honest struct views over the five
bags are −57 Brotli (support alone −68), 25 of 013's 405 stores; a typed read stops blocking but stays
non-deferrable, and the level-15 search's deterministic, source-dependent `function-spelling` flip (+99/+141
on two states) outweighs every per-bag effect. Side finding: the baseline's `scrollTop` TypeError is that
arrow candidate spelling a `this` method as an arrow. Port left at state 5.**
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
`Record` of `JsValue`, the reads become `FieldGet`/`RecordFieldGet` and stop blocking. Confirms:
**≤ −80 Brotli** on `dist/jquery.esm.js` under the port's config and **≥ 60 of the 405 surplus
stores** (013) gone, with the port's tests and the 040 `animate` smoke passing. Falsifies: **≥ 0**,
the 021 regression again — then the reason (which helper layer re-boxes the value) is the finding,
and the two follow-ups need language: a typed ordinary-prototype dictionary `object` of `T` for
`ajaxSetup` (104 reads, ≤ −150 / > −40) and a callable `object` for `jQuery`/`jQuery.fn` (263 reads,
20% of the port's, ≤ −200 / > −60).

## Read

- `finer/objective.md`, `finer/status.md`, this folder
- [013](../013-statement-density/README.md) and [021](../021-reflective-ffi-predicts-loss/README.md): the mechanism and the two regressions of partial typing
- `docs/language-v0.1.md` — `struct`, `Record` of `T`, `JsValue`, and what escapes
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

**Mechanism, read exactly** (`codegen_ir_js.rs`). `unstable_values` (`:20295`) marks every op that
`op_can_defer` (`:24703`) refuses, and *no* member read is in that list — `IndexGet`, `FieldGet`,
`RecordFieldGet`, `HostFieldGet` alike — so a typed read is still unstable and still fuses only into a
consumer reached before the next observable op or write (`can_fuse_value`, `:24316`). What a type changes
is `op_evaluation_is_observable_assuming` (`:25172`, the `IndexGet` arm at `:25198`): a typed read stops being a *barrier*, so a neighbour
may fuse across it and it may fuse across other typed reads and pure construction. The win is confined to
read–read adjacency: `use(o.x, o.y, o.z)` fuses, `x = o.a; call(); use(x)` still names `x` (probe p4,
`finer/out/042/probes/`: identical statements for `JsValue` and `Record` of `JsValue` when a call intervenes).

**`Record` of `JsValue` is the wrong type** (p1, p3): reads are `T?` and spell `o.k??null` (+6 bytes and
`undefined`→`null` per read), `.truthy()` does not type-check on `JsValue?`, and a literal costs
`{__proto__:null,`. **A struct view through `JS.assume` is the honest, byte-free shape** (p2, p5, p7, p8,
p9 under the port's own config): `Support support = JS.assume(JS.object())`, `SpecialView specialFor()`,
`AnimOptsView opts = JS.assume(mergeObject(…))`, `TweenView` parameters. Field names are kept (no
positional slot, no property mangling), a re-read after a call is re-emitted, views cross function
boundaries and closure captures as one binding with no alias, and `JS.assume` in argument position is free.
Four reads stay dynamic on purpose — `fnPromise`/`fnFinish` hooks, `delegateOf`, `originSpecial` —
because the bag may be absent and a struct view of `undefined` would let its field read hoist over the guard.

The five steps are `finer/out/042/steps/` (applied by `apply.mjs`, states by `state.sh N`; the full diff
is `all-steps.patch`, 20 files). Sizes: `build-step.sh` (working-tree scratch copy, main binary, four
cores, then `npm test`, the 039 harness copy and the 041 `animate` smoke). Stores: `census.sh`, the 013
census at level 13 with candidate search off and the memo disabled, so the counters see one emission.

Builds ran on pool workers 3–5 (`finer/tools/workers.mjs build --instances N --dist-dir
finer/out/042/state-N`, eight cores, the main checkout's binary `7fd4657e…`, the one that built the
baseline dist as 044's `new`); gates and codec ran here (`gates.sh N`). `spelling` is the artifact's
`function` / `=>` count: the port's config leaves `function_spelling` unset, so under Brotli the default is
`function` and the arrow artifact is the search's `function-spelling` flip (`decision_registry.rs:1015`).

| step | bag | raw | gzip9 | brotli11 | Δ brotli | surplus stores | spelling | tests |
|---|---|---:|---:|---:|---:|---:|---|---|
| baseline (host dist) | — | 82602 | 31882 | 28641 | 0 | 1681 (=013) | 168 / 428 | 6/6, 6/6, animate ok |
| 0 control (worker) | — | 82602 | 31882 | 28641 | 0 | 1681 | 168 / 428 | same; byte-identical to the host dist |
| 1 | support (12 reads) | 82363 | 31817 | 28573 | **−68** | 1681 (0) | 168 / 427 | 6/6, 6/6, animate ok |
| 2 | + queue hooks (11) | 82645 | 31875 | 28625 | −16 | 1673 (−8) | 168 / 428 | same |
| 3 | + special (20) | 86181 | 32102 | 28740 | +99 | 1673 (0) | 593 / 4 | same, scrollTop ok |
| 3 rerun | same source | 86181 | 32102 | 28740 | +99 | — | 593 / 4 | byte-identical to step 3: deterministic |
| 4 | + opts (36) | 86489 | 32092 | 28782 | +141 | 1661 (−12) | 593 / 4 | same, scrollTop ok |
| 5 | + tween (27) | 82510 | 31924 | 28584 | **−57** | 1656 (−5) | 168 / 427 | 6/6, 6/6, animate ok |

Stores: 25 of 013's 405 with all five bags (115 reads of jquery's 1344 string-key reads: proportional).
Bytes: only states with the same spelling compare. Support alone is −68; every later bag is within the
search's own wobble — hooks +52 on the same family for eight freed stores, special and opts flip the
search off the arrow family (+3.5 KB raw), tween flips it back. **Side finding, a miscompile:** the
baseline's `scrollTop(1)` TypeError (039, unexamined) is the arrow candidate spelling a `this`-using
method as an arrow — `ge.scrollTop=e=>N(this,…)` in the baseline against `me.scrollTop=function(e){return
z(this,…)}` in the `function`-spelled artifacts, which run `scrollTop` correctly. It ships today.

## Verdict

**Falsified at the claimed size, not at the direction.** ≤ −80 and ≥ 60 stores were the bar; the honest
types give −57 with all five bags (−68 with `support` alone) and free 25 stores. The ceiling is structural,
not a re-boxing helper (021's failure mode did not recur: struct views cross the `JsValue` helper layer for
free): a typed read stays non-deferrable, so it only stops *blocking* — the gain is read–read adjacency,
and it scales with the read count. `ajaxSetup` (104 reads) and `jQuery`/`jQuery.fn` (263) would follow the
same proportion, ≈ 25 and ≈ 65 stores, below what the level-15 search's path dependence can even resolve.
The port's `src/` is left at **state 5** (all five views, gates green, −57): the typed shape objective.md
§5 asks for, at a cost of 11 bytes against the support-only artifact that the noise does not distinguish.

## Next

1. **Compiler, before any port-typing follow-up:** the `function-spelling` flip is path-dependent on a
   16-line source change (states 3, 4 lose it; 5 regains it): a `candidate_search = "always"` build that
   lands 3.5 KB raw from its own incumbent's family is objective.md §7's "search that stopped early".
   Measure the flip's admission on states 3 and 5 (`LILSCRIPT_TIMING=1`, the decision's proposals and
   refusals), then make the family reachable from both seeds.
2. **Compiler, correctness:** the arrow candidate's `this` miscompile (`scrollTop`) must be refused by the
   admission gate (a `this`-using function may not be spelled as an arrow); the baseline dist ships it.
3. **Not a `lang` folder yet:** `object` of `T` and a callable `object` (the two follow-ups in the claim) buy
   read-adjacency fusion only; their expected size is under the search noise until (1) lands. The lever
   with a measured ceiling stays 013's item 2 / 043's redirection: making typed reads deferrable across
   provably non-writing gaps (`op_can_defer` excludes every member read; `can_fuse_value` already relaxes
   it for the first consumer), which is what would turn 013's 405 into bytes without the flag.
