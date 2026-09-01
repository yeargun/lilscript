# 017 — oxc's `merge_assignment_to_declaration`, ported and then removed

**Status: IMPLEMENTED, TESTED, MEASURED AT ZERO, REVERTED.** The technique is real and LilScript
genuinely lacked it. It does not apply to LilScript's output.

## The lead

[013](../013-statement-density/README.md) concluded that the expensive shape in generated output is
**starting a new declaration**, not starting a new statement — `var`/`let` keywords cost 3–4 bytes
each where `;` and `,` both cost one. Reading oxc's statement minimizer for that specific thing found
exactly the matching pass:

`merge_assignment_to_declaration`, `_refs/oxc_minifier-0.147.0/src/peephole/minimize_statements.rs:516`

```
var a; a = b();   =>   var a = b();
```

with carefully documented refusals, quoted from the source:

- *"var a = b(); a = c();" => "var a = (b(), c());"* — not possible, `c()` may access `a`
- *"let a; a = foo(a);" => "let a = foo(a);"* — not possible, TDZ error introduced
- and it will not move an assignment above a declarator that already has an initializer

A direct test showed LilScript handles the single-declarator case but **not the multi-declarator
one**: `var a,b;a=g(),b=g()` stayed split where oxc would produce `var a=g(),b=g()`. Saving is
`k * (name_length + 1)` bytes for `k` merged declarators.

## Implemented

`fold_assignments_into_declaration`, generalizing oxc's single-declarator case to a whole declarator
prefix, keeping its safety conditions and adding the one the generalization needs:

- plain `=` only, never compound — `a+=1` reads `a` first;
- every declarator bare and uninitialized, so nothing observes the reordering;
- assignments must target declarators **in declaration order from the first** — `var a,b;b=1,a=b`
  must not become `var a=b,b=1`, which leaves `a` undefined;
- `let` additionally forbids any right-hand side mentioning a declared name: after `let a,b;` both
  bindings exist so `a=b` reads `undefined`, but hoisted into `let a=b,b` the same read is a TDZ
  `ReferenceError`. `var` has no TDZ and needs no such guard.

Five safety cases were tested and all refused correctly; the positive cases folded.

## Measured at zero

| port | before | after |
|---|---:|---:|
| markedlil | 9506 | **9506** |
| mobxlil | 16514 | **16514** |

Not one byte, on either clean-source port. The fold never fires: it does not appear in the per-fold
idle table and its rewrite count is zero on every artifact tested.

Counting the target shape directly in shipped artifacts explains why:

| artifact | occurrences of `var a,b;a=` |
|---|---:|
| `jquery.esm.js` | 1 |
| `mobx.esm.js` | 3 |
| `marked.esm.js` | 0 |

**LilScript's SSA destruction already emits initialized declarators.** It does not produce
declare-then-assign, so there is nothing for this pass to merge. oxc needs it because it minifies
arbitrary hand-written JavaScript, where `var a; a = b();` is a shape humans write. LilScript writes
its own declarations and writes them merged.

## Reverted

Removed, on the discipline established in [005](../005-idle-fold-guards/README.md): a fold that
measurably does nothing still costs a pass over every artifact, and the pipeline already carries 135
of them. Landing a correct, well-tested, zero-value fold would be exactly the mistake
[010](../010-string-pool-alias-pricing/README.md) made.

## The generalizable finding

This is the third competitor technique to survive a source reading and then die on measurement,
after `Math.pow` → `**` (LilScript never emits `Math.pow`) and per-string quote selection (worth five
raw bytes on an 89 KB artifact). All three share a shape:

> **A minifier pass exists to repair a shape a human wrote. A compiler that generates its own output
> can simply not emit that shape — and LilScript mostly doesn't.**

So the technique inventory in `_refs/competitor-techniques.md` should be read as a list of *shapes to
avoid emitting*, not a list of *passes to port*. Porting the pass is the expensive way to get a
result the code generator can give for free, and it adds pipeline cost forever.
