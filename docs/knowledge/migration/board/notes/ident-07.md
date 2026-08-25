# ident-07 — retiring the identity emulation a fused class makes redundant

Parent: [ledger](../LEDGER.md). Status: landed. Page: [class identity](../../../compilation/class-identity.md).

## Question

Once `fold_constructor_prototype_tables_to_classes` has produced a named ES
class, the port's hand-written identity emulation is dead weight — but the
compiler kept emitting all of it. How much, and what makes deleting it legal?

## Current hypothesis

Confirmed. A named ES class already provides, from the language:
`.name`, `.length`, non-writable non-enumerable `.prototype`, non-enumerable
prototype methods, and `TypeError` when called without `new`. Every user-space
construct the port wrote to emulate those is therefore unreachable once the
class exists.

## Constraints specific to this task

- The peephole must not become a general dead-binding collector: the existing
  policy (`removes_only_unreferenced_standalone_var_declarations`) keeps
  unreferenced *initialized* declarations, and folds run on strings the caller
  chooses. Only the two compiler-recognizable identity shapes are retired.
- `aggregates/class-counter` and `aggregates/class-scale` must keep dissolving.
  Verified: `comparison/cases` still reports 617/617 with strict wins
  raw 617 / gzip 612 / Brotli 613, unchanged.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-25 | error-tracking, shipped config before this work | `target/release/lilscript-codec --json dist/error-tracking.raw.js` | raw 18,835 / gzip-9 6,808 / Brotli-11 6,200 | gate |
| 2026-08-25 | after `lilscript.identity.toml` + the folds below | same command | raw 15,332 / gzip-9 5,659 / **Brotli-11 5,156** | gate |
| 2026-08-25 | Oxc hero for the same pack | `lilscript-codec --json .tmp/pack-lanes/error-tracking/official-oxc-mangle.js` | raw 14,662 / gzip-9 5,700 / Brotli-11 5,224 | gate |
| 2026-08-25 | identity surface of the winning artifact | `node --test test/error-tracking.compat.test.mjs` (posthoglil) | 5/5; names, arity, `.prototype` descriptors, throw-without-`new` all match official | gate |
| 2026-08-25 | Oxc's own identity surface | `node scratch identity-probe` on the same lane artifact | `.name === "Oe"`, `"Ae"`, … — the hero does **not** preserve class names | diag |

## Log

- 2026-08-25 — `fold_value_binding_iife`: `(function(){var v;v=<expr>;return v})()` is an identity wrapper left by inlining a one-value factory. Unwrapping it is worth bytes and, more importantly, lifts the class expression into a scope the class-shape folds can see — `fold_undefined_defaults_into_formals` stopped at every `function` head, which is why `buildFromUnknown` kept arity 2 and failed its `.length` pin. Refused when the value mentions the binding outside a nested rebinding. — **LANDED**
- 2026-08-25 — `fold_undefined_defaults_into_formals` advanced its cursor to `block_close + 1` on every exit, so one enclosing `function` hid every nested method. Now steps into the body when it makes no rewrite at a site. This alone is most of jQuery's −8,737 raw / −866 Brotli. — **LANDED**
- 2026-08-25 — `drop_redundant_class_constructor_guards`: a `guard(this,new.target,name)` call inside a fused class constructor is unreachable. Recognized only when the callee is a three-parameter arrow whose entire body is `if(a==null||!b.prototype.isPrototypeOf(a))throw new TypeError(…)`. Empties the constructor when nothing else is left. — **LANDED**
- 2026-08-25 — `drop_orphaned_class_identity_guards`: retires the declarator of a guard, or of an identity finisher (an arrow whose only free identifier is `Object` and which installs a `name`/`length`/`prototype` descriptor), once no reference remains. Deliberately narrow — see the constraint above. — **LANDED**
- 2026-08-25 — The port's `.lil` now depends on fusion for correctness: `constructorValue["prototype"]["m"] = …` is an enumerable data property, so without fusion `Object.keys(proto)` is non-empty and the compat suite fails. `lilscript.dev.toml` (opt 8, search off) fails one subtest for exactly this reason. Pre-existing, port-side, not caused by this work. — **OPEN**
- 2026-08-25 — Partial fusion is observably different from full fusion: at some settings `applyChunkIds` stays a `C.prototype.x = adapter(...)` assignment while its siblings become class methods, so it alone is enumerable. `stable_local_names = false` reaches that state. The fold should be all-or-nothing on a table it decides to fuse. — **OPEN**

## Next step

Make fusion all-or-nothing on a constructor table (the `applyChunkIds` case
above), then take phase 1 of [class identity](../../../compilation/class-identity.md):
emit the named class from IR so the port can delete `error-coercer-api.lil`'s
factories instead of having the peephole reverse-engineer them.
