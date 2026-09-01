# 005 — Folds that already declined these exact bytes

**Status: LANDED (memo). Guard work continues.**

## Hypothesis

[004](../004-peephole-relex-tax/README.md) found that **82% of peephole fold invocations rewrite
nothing** while consuming 15% of the whole compile. Two mechanisms could remove that waste:

- **(a) Memoize declines.** Each fold is a pure `Fn(&str) -> (String, usize)` reached through a
  *plain function item* — verified: the pipeline contains **zero** state-capturing closures, so
  `std::any::type_name_of_val` is a unique, stable identity per fold. Therefore
  `(fold identity, input bytes)` fully determines the answer, and a fold that already declined
  these exact bytes can be skipped outright. This is sound with no per-fold knowledge.
- **(b) Guard entry.** Give each fold a cheap necessary-condition test so it never runs when its
  enabling syntax is absent. This catches the *first* occurrence too, which (a) cannot.

## Where the idle time actually sits

Per-fold profiling was added (`timing::record_fold`, keyed on the fold's Rust type name, printed by
`main.rs` under `LILSCRIPT_TIMING`). jQuery port, level 13 — idle time is extremely concentrated:

| fold | idle ms | idle/calls | ms per idle call |
|---|---:|---:|---:|
| `fold_int32_coercions` | 8617 | 58/64 | **135** |
| `declare_implicit_assignment_bindings` | 6247 | 32/38 | **195** |
| `fold_single_use_function_values` | 3842 | **80/80** | 48 |
| `remove_unused_standalone_vars` | 2375 | 100/112 | 24 |
| `fold_single_use_regex_bindings` | 1328 | **16/16** | 83 |
| `fold_constructor_prototype_tables_to_classes` | 941 | 30/48 | 31 |
| *(remaining 18 in the top 24)* | ~500 | — | — |

**Six folds account for 23.3 s of the 23.7 s total idle time.** Two of them never fire at all on this
artifact and still cost 5.2 s between them.

The high per-call figures are not lexing (0.9 ms) or index construction (bindings 16.8 ms). They are
the folds' own scans — `declare_implicit_assignment_bindings` at 195 ms calls
`name_is_declared_in_any_scope` per candidate site, which is a per-site scope walk, so it is
effectively quadratic in the artifact.

## Change landed: `DECLINED_FOLDS`

`src/artifact_memo.rs` gains a third memo keyed on `(&'static str, ContentDigest)`.
`RewriteSession` (`src/js_peephole/mod.rs`) now:

- caches the digest of `code`, recomputing it only when the bytes actually change — so the digest is
  paid ~once per *rewrite* rather than once per *fold*;
- routes `run`, `run_flag`, and `repeat` through one `run_named` path that consults the memo first;
- records a decline **only after checking both that the fold reported zero rewrites and that it
  returned byte-identical output** — a fold that returns `count == 0` while moving bytes must not be
  memoized as a no-op.

This matters because several folds appear four or five times in the 135-fold pipeline
(`fold_int32_coercions` x4, `fold_single_use_function_values` x5), and the pipeline itself runs
twice per call, over 8 peephole calls and 58 emissions.

## Measurement — jQuery port, level 13, interleaved A/B in one binary

`LILSCRIPT_NO_MEMO=1` disables all three caches (codec, generated-JS analysis, declined folds) and
the lexer cache, reproducing the pre-change behavior exactly. Work counters are deterministic; wall
clock on this shared host is not, and is omitted for that reason.

| metric | caches off | caches on | change |
|---|---:|---:|---:|
| fold bytes scanned (MB) | 234.3 | 166.3 | **−29%** |
| lex bytes scanned (MB) | 710.2 | 332.7 | **−53%** |
| generated-JS analyses | 552 | 47 | **−91%** |
| canonical codec encodes | 54 | 52 | −4% |
| **output artifact** | — | — | **byte-identical** |

## Findings

1. **Sound without per-fold knowledge.** The decline memo needed only one structural fact — that no
   fold in the pipeline is a state-capturing closure — which was checked rather than assumed. Had
   even one been `|code| fold(code, flag)`, two different captured flags would share a type name and
   the memo would have been unsound.
2. **It cannot remove first-occurrence cost**, which is why the top-6 list above still matters. A
   fold that is idle on every one of 80 invocations still pays full price on the first of each
   distinct artifact.
3. **Two of the six are algorithmically wrong, not merely unguarded.**
   `declare_implicit_assignment_bindings` (195 ms) and `fold_int32_coercions` (135 ms) do per-site
   scope walks. Guarding them helps artifacts that lack the syntax; it does nothing for artifacts
   that have it. Those want an index, not a guard.

## Mechanism (b) — entry guards — FALSIFIED

The plan was to give each hot fold a cheap necessary-condition test (does the artifact contain the
token that enables this fold at all?) so it never runs on a program it cannot rewrite. Before
implementing 135 of them, the premise was checked against the actual artifact.

Token census of the jQuery level-13 artifact:

| token | occurrences |
|---|---:|
| `var` | 481 |
| `let` | 32 |
| `function` | 598 |
| `arguments` | 58 |
| `|0` | 17 |
| regex literals | 53 |
| `.has(` | 0 |

The two folds that are idle on **100%** of their invocations are `fold_single_use_function_values`
(needs `let`/`var`/`const` plus a function-valued initializer) and `fold_single_use_regex_bindings`
(needs `let`/`var`/`const` plus a regex literal). **Every enabling token they require is present in
abundance.** They are idle because the specific *pattern* — a single-use binding of that literal
shape — does not occur, and deciding that requires substantially the work the fold already does.

**So entry guards would not have fired at all here.** The premise was wrong, and finding that out
cost one token census instead of 135 hand-written guards each carrying a silent-regression risk.
Recorded so it is not re-proposed.

The one place a guard would still pay is `fold_has_predicate_calls` inside `fold_int32_coercions`
(`.has(` occurs zero times), but that is a sub-fold, not a pipeline entry, and it is a small part of
that fold's 135 ms.

## What was done instead

The six direct fold calls in `emit_javascript_candidate` (`src/compiler.rs`) were routed through the
same decline memo via `js_peephole::fold_once_memoized`. That path runs on **every emission** — 58
of them on jQuery, the hottest loop in the compiler on a large artifact — and previously had no memo
at all, because it has no `RewriteSession` to carry a cached digest. The digest is paid per call
there, which is microseconds against folds measured in tens of milliseconds.

## Next

- Replace `name_is_declared_in_any_scope`'s per-site scope walk (the 195 ms in
  `declare_implicit_assignment_bindings`) with a precomputed name-to-scope map. This is the honest
  fix for the two quadratic folds: they are slow on artifacts that *do* contain their syntax, which
  is exactly the case a guard cannot help.
- The larger lever is not per-fold at all — it is how many emissions the search buys. See
  [007](../007-level-13-sweet-spot/README.md).
