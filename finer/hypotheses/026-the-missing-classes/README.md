# 026 — 018's missing `class` declarations, found

**Status: HALF ANSWERED, AND THE OTHER HALF FALSIFIED BY MY OWN TEST.** The candidate is not
missing — it is generated with all ten classes and a fold corrupts it. But removing that fold does
**not** bring the classes back, so the corruption is not why the final artifact has none. The
correction is at the bottom; the headline this document originally carried was wrong.

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

## CORRECTION: disabling the fold does not restore the classes

I predicted that suppressing `fold_unread_prototype_aliases` would bring the ten classes back and
recover 018's 7546 bytes. **Tested, and it does not.** Same source, same config, the fold made a
no-op:

| | `class` | `.prototype` | raw | Brotli |
|---|---:|---:|---:|---:|
| fold on | 0 | 120 | 64943 | 15944 |
| **fold off** | **0** | **120** | 63526 | **15944** |
| delta | 0 | 0 | −1417 | **0** |

So the corruption is real and the fold is a genuine defect — it takes a valid artifact with ten
classes and emits one that does not resolve, 36 times per build — but **it is not the reason the
shipped artifact has none.** The class-bearing candidate loses even when nothing corrupts it, which
puts the question back where 018 left it: something upstream of this fold is not scoring or not
proposing that plan. What is now ruled out is "this fold destroys the winner."

Two things survive from this investigation and are worth keeping:

1. **018's stated conclusion is still wrong.** Candidates *are* refused, by the syntax validator,
   before any admission counter sees them — that is directly observed, 36 times, and no counter 018
   added sits early enough to notice.
2. **`LILSCRIPT_VALIDATE_FOLDS` works.** It found two real defects in one run — this one and the
   `ternary_end` colon bug fixed in `c7533be` — where seven hand-instrumented mechanisms found none.

## Status of the fix

`unread_pure_alias_is_live` still misses the later reads of a reused temporary, so the fold still
emits unresolvable JavaScript on mobxlil. That is worth fixing on correctness grounds. It is not
worth 7546 bytes, and this document should not have implied it was before the test was run.

Two more folds turn valid mobxlil artifacts invalid the same way and are unexamined:
`control::fold_single_use_if_assigns` (8) and `declarations::merge_adjacent_declarations` (6).
