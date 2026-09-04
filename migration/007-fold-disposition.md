# 007 — Disposition of the 139 folds

Parent: [index](index.md). Directive: [D8](001-directives.md#d8--delete-rather-than-port-and-prove-the-deletion).

---

## The finding that reframes the migration

A classification of every fold entry point in `src/js_peephole/` — what information each needs,
whether the typed SSA IR already carries that fact exactly, and what should become of it:

| Disposition | Count | Share |
|---|---:|---:|
| **delete** — emit correctly upstream | 84 | 59% |
| **delete** — duplicates an IR pass | 13 | 9% |
| **delete** — obsolete or dead | 10 | 7% |
| **port** — a genuine target-level transform | 35 | 25% |

And the fact that explains it: **123 of 142 classified entries (87%) need information the SSA IR
already knows exactly.** Four need something the IR does not have.

So the migration is not "port 47,000 lines of folds to an AST." Roughly **three quarters of the text
layer is cleanup after the emitter's own output**, and the work is teaching the emitter not to make
the mess. That inverts the effort estimate and it inverts the risk: deleting a fold whose trigger no
longer occurs is far safer than re-implementing it against a new representation.

> The classifying agent's prose summary counted 139 entry points and 73%; its structured rows count
> 142 and 75%. The discrepancy is in what counts as an entry point (`rename.rs`'s two, the
> `(String, bool)` outlier). It does not move the conclusion. Exact counts are re-derived in Phase 0
> and pinned there.

---

## The fourteen work groups

Each group is one unit of migration work: it lands together, gates together, and its folds are
deleted together. Ordered by the phase that consumes them.

| # | Group | Folds | Difficulty | Why they exist |
|---|---|---:|---|---|
| **G1** | Printer hygiene and lexical spelling | 13 | trivial | Terminators, separators, braces, parens, operator polarity, concise arrow bodies. No scope, no liveness, no types — pure rendering decisions about a node the emitter already holds. |
| **G2** | Repairs for byte-span rewriting | 9 | trivial | They repair states **a tree cannot enter**: a sequence with a hole, a ternary with an empty arm, `returna` fused into one identifier, `async async`, a declarator another fold ate. |
| **G3** | `void 0` and declaration materialization | 9 | moderate | The emitter writes `=void 0` for every `JsValue` global and one `var` per module binding, then six folds and a scored family undo it. The IR knows the first real store to each binding exactly. |
| **G4** | Out-of-SSA copies, temporaries, def placement | 17 | hard | **The largest deletable cluster.** Every copy these chase is created by the emitter's own out-of-SSA translation (`parallel_copy_temp`, `reusable_parallel_copy_temporary`). |
| **G5** | Dead code the IR optimizer already removes | 8 | moderate | Catching leftovers produced by *other folds* and by the string pool — `remove_unused_standalone_vars`' own doc names the string pool as the usual producer. Six driver call sites. |
| **G6** | int32 coercion elision | 5 | moderate | `fold_int32_coercions` is 1,554 lines re-deriving from tokens what `src/value_analysis.rs` computes exactly: `I32Range`, `can_elide_coercion`, `return_range`, `field_range`. |
| **G7** | `arguments` object dissolution | 8 | moderate | All eight exist because `emit_calling_convention_aliases` binds a `MethodRest`/`StaticRest` parameter to the literal name `arguments`, after which 900+ lines turn `arguments[0]` back into a formal. |
| **G8** | Loop header reconstruction | 11 | hard | **Two of the three known wrong-program folds live here.** They guess the latch by scanning tokens backwards, while `ControlShape::Loop { update }` carries it exactly. |
| **G9** | Branch and return shaping | 20 | hard | The codec-scored family — where "compression must not degrade" bites hardest and where *port* really means port. Several are the same CFG fact in different orientations, kept apart only because the codec may prefer one. |
| **G10** | Boolean, ternary and phi materialization | 16 | moderate | Half reconstruct a phi from the text the emitter wrote *for that same phi* — `fold_conditional_assigned_false_phi` and `fold_same_lvalue_ternary` say so in their names. |
| **G11** | Object and array literal construction | 12 | moderate | The emitter splits an allocation from its stores (`d={};d.k=v`), writes borrowed prototype calls it then un-borrows, and emits `new RegExp("…")` where a literal fits. |
| **G12** | Class recovery from prototype tables | 11 | **research** | **The one genuine exception** — see below. |
| **G13** | Function placement and inlining | 6 | hard | The IR has the call graph, use counts, arity, effect summaries and its own inliner. What the peephole adds is a *codec-scored* choice, not a missing fact. |
| **G14** | Identifier convergence *(adjacent)* | 2 | hard | Not folds, but they share the token/scope/binding machinery — and they are the **largest remaining Brotli lever** (katexlil's gap is the identifier stream, +2,113). |

### G12 is the exception, and it must be named as one

`fold_constructor_prototype_tables_to_classes` and its family are why
`src/js_peephole/folds/classes.rs` is 8,017 lines. **They do not clean up after the emitter.** They
recover ES class structure from `Constructor.prototype.method = …` tables that come from the
*source* — the untyped `JsValue` transliterations (katexlil, mobxlil, remark) that write JavaScript
idiom the IR never modelled as a class.

This is the only group where the IR genuinely lacks the fact, and it is therefore the only group
where a target AST does not automatically subsume the work. Two honest options, and the plan must
pick one on evidence rather than assume:

1. **Keep a scoped recogniser** operating on the target AST rather than on tokens — better than
   today (it would have binding identity instead of guesses), but still recovery from shape.
2. **Fix it in the ports** — typed sources produce classes the IR models directly. This is already
   the project's own finding: typed ports win, `JsValue` transliterations lose. It moves the work
   from the compiler to the ports and removes 8,017 lines.

Option 2 is the design-correct answer and option 1 is the migration-safe one. The plan's position:
port G12 to the AST **last**, keep it scoped and named, and treat every port that stops needing it
as progress. Do not let G12 block the other thirteen groups.

---

## What "delete" has to prove

A deleted fold is a silent regression risk: if the emitter did *not* actually subsume it, the bytes
move and — per [D3](001-directives.md#d3--compression-is-a-hard-constraint-and-byte-identity-is-the-only-clean-proof)
— the movement hides inside the ±100 noise floor.

So deletion is a two-commit protocol, never one:

**Commit 1 — land the emitter change, keep the fold enabled.** Then across the full sweep matrix:

- the fold's `active` count must be **0 on every port and every config** (the instrumentation
  already reports active-vs-idle per fold under `LILSCRIPT_TIMING`)
- the artifact must be **byte-identical** to the incumbent on every config not explicitly declared
- the number of distinct scored candidates must be **unchanged** — a fold that shipped a scored
  variant must not silently become a fixed choice

**Commit 2 — delete the fold and its tests.** No byte change may accompany this commit. If one
appears, commit 1 was wrong and is reverted, not patched.

A fold that still reports `active > 0` anywhere is not subsumed. That is data, not a judgement call,
and it is the whole reason the emitter change ships *before* the deletion.

---

## Ordering, and why it is not by difficulty

Deletion order follows **what unblocks the next group**, not what is easiest:

1. **G1 + G2 first.** Thirteen printer decisions and nine repairs-for-repairs. G2 is the most
   satisfying evidence for the whole migration: nine folds that exist purely because rewrites are
   byte spans, and that a tree makes unreachable rather than unnecessary.
2. **G3, G7, G11** next — three groups where the emitter writes a shape and immediately un-writes
   it. Each is self-contained and each removes a named emitter decision.
3. **G6** — needs the value-analysis facts routed to the target node, which is the first real test
   of the annotation design ([003](003-target-representation.md)).
4. **G4, G5** — out-of-SSA translation and the dead code it leaves. G4 is the largest cluster and
   should be attempted only once G1–G3 have proved the deletion protocol works.
5. **G8** — loop headers, carrying `Latch` from `ControlShape::Loop { update }`. This closes two of
   the three shipped wrong-program folds by construction, so it is high value despite being hard.
6. **G9, G10, G13** — the scored families. These need the `ShapeTransform { sites, apply(subset),
   polarities }` variant type to exist first, or the backend becomes always-emit-best-shape and
   **will** regress ports.
7. **G14** — identifier convergence, after the tree owns naming. Largest remaining lever; deserves
   its own phase and its own A/B, not a knob flip.
8. **G12** — last, scoped, and ideally shrinking as ports become typed.

---

## The number to hold the migration against

The whole text layer is worth **189 Brotli bytes on markedlil** (9,397 with all 121 folds, 9,586
with none). That is the total budget at risk across all fourteen groups on that port.

Read correctly, that is not an argument that the layer is worthless — it is an argument that **no
single group is worth a correctness risk**, and that a group whose deletion moves bytes by more than
a few dozen has almost certainly changed the program rather than its spelling. Treat a large byte
movement on a "neutral" deletion as a correctness alarm, not a win.
