# 026 — 018's missing `class` declarations, found

**Status: MECHANISM LOCATED. The candidate was never missing — it is generated with all ten classes
and then corrupted by a fold, and the corrupted artifact is thrown away by the syntax validator.**

## What 018 left open

[018](../018-mobx-admission-regression/README.md) established that mobxlil's 7546 raw bytes are ten
`class` declarations replaced by `function` + `.prototype` tables, bisected it to the 08-29 admission
cluster, and then could not explain it. It instrumented admission (**150 validations, 0 rejections**),
direct-artifact validation (**11 calls, 0 failures**), dropped IR probes (**0 drops**), disabled the
property-introduction check (**no change**), and bought 20× the search budget (**280 bytes of 7546,
zero classes**). Its conclusion:

> Nothing is being rejected anywhere, and the output is still 7940 bytes bigger. Whatever the
> mechanism is, it changes which candidates are *generated or scored*, not which are *refused*.

That conclusion was wrong, and it was wrong in a way no counter it added could see.

## The candidate is generated, with all ten classes

`LILSCRIPT_VALIDATE_FOLDS` (added in the previous commit) re-validates the artifact after every fold
and names the first fold that turns a **valid** artifact **invalid**. On mobxlil it names three, and
the first is `declarations::fold_unread_prototype_aliases`, 36 times. Its *input* is the candidate
018 was looking for:

| | `class` | `.prototype` |
|---|---:|---:|
| input to the fold | **10** | 34 |
| output of the fold | 10 | 27 |

**Ten classes** — exactly the count 018 measured in the good build. The class-emitting candidate is
alive and well right up to this fold.

## What the fold does to it

It removes seven prototype aliases, and every one of them assigns the same reused module-level
temporary:

```
g=E.prototype   g=v.prototype   g=p.prototype
g=U.prototype   g=s.prototype   g=s.prototype   g=$.prototype
```

`g` is read 224 times in the artifact. The fold judged each assignment dead, deleted it, and left an
artifact that no longer resolves — `unresolved generated identifier`. The validator then rejects the
whole thing and the search keeps the `function`+`.prototype` spelling instead.

## Why 018's instrumentation could not see it

Every counter 018 added sits at or after **admission**. This rejection happens earlier, in
`analyze_generated_javascript`, which is the syntax validator — and it fails the artifact *whole*,
without incrementing anything admission-shaped. From admission's point of view the candidate simply
never arrived, which is precisely the shape of the conclusion 018 drew: "generated or scored, not
refused". It was refused. Just not where anyone was counting.

The general lesson is the one [024](../024-optional-chain-floor/README.md) also reached from a
different direction: **a check that fails closed and silently is indistinguishable from a search that
never had the idea.** The fix for that is not more counters at the decision point, it is a diagnostic
that names the *producer* of the invalid artifact — which is what `LILSCRIPT_VALIDATE_FOLDS` does,
and it located in one run what seven falsified mechanisms could not.

## Status of the fix

The `:`-stranding bug in `boolean::ternary_end`, found the same way and fixed in the previous commit,
was worth 4 bytes on mobxlil — the candidate it rescued was not the better one. This one is worth
7546 raw and 253 Brotli by 018's own measurement, and it is not yet fixed: `unread_pure_alias_is_live`
has to account for a reused temporary whose later reads it currently misses.

Two more folds turn valid mobxlil artifacts invalid the same way and are unexamined:
`control::fold_single_use_if_assigns` (8) and `declarations::merge_adjacent_declarations` (6).
