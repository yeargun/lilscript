# 059 — The idiom census models a win the codec refuses

**Status: NO EFFECT ABOVE THE NOISE FLOOR, and one claim of mine retracted.** Applied to a finished
artifact the conversion loses at every dose. Offered as a scored candidate mid-search it wins 5 of 8
offers on markedlil and the artifact ends −53 Brotli smaller — but on the pool katexlil ends
**+82 worse**, motionlil and jquerylil exactly level, markedlil −77. Every one of those is inside the ±100 band
053 established, so the honest reading is that the knob moves nothing measurable in either
direction. Default off, and it stays off. Small recurring idioms *are* spelled apart, and
converging them toward their commonest spelling improves identifier entropy **and** removes novel
bytes — the two terms add rather than fight, which is the opposite of 056. The model puts 235–1083
Brotli on the table per port. Applied under a sound legality rule only 7–30% of the renames land,
and the codec refuses it at every dose: four claimed bindings cost +21 Brotli, sixteen +35, two
hundred and fifty-six +130. The model missed the displacement -- 124 claimed bindings move 4,674
names -- and priced single-letter identifiers as if they were novel text.
Lane: measure. Objective: brotli. Ports: jquerylil, mobxlil, markedlil, micromarklil, katexlil.
Opened: 2026-09-03.

## Prior art

Terser (`mangle.js`, base54 weighted by source character frequency) and Oxc rank per scope by use
count; Closure's `RenameVars` optimises name length and reuse. **None of them has any notion of
agreement between two scopes**, so an idiom recurring across 60 functions gets 22 spellings from all
three. 056 established why that is nonetheless close to right: frequency ranking is itself a
first-order entropy optimisation that Brotli's literal coder collects directly.

## Claim

056 falsified converging whole *function* shapes. It did not test the owner's actual claim: small
idioms — `"string"==typeof e`, an arrow header, a loop over a length — recurring across functions
that otherwise differ. 056 clustered whole AST nodes at ≥40 bytes and could not see them.

The claim has a property 056's version lacked. Converging toward the **commonest spelling** uses the
**commonest letters**, so the entropy term and the match term point the same way instead of opposing.

**Confirms** convergeable value exceeding entropy cost by more than ±100 on two ports.
**Falsifies** under that.

## Method

Three offline probes over finished text priced this wrong before the census was written, and each
error was a different illegal assumption. They are recorded because each is an easy mistake:

| probe | claimed prize | the illegal assumption |
|---|---:|---|
| naive token n-grams | 20,680 | globals and property names treated as renameable — it priced turning `Array.isArray(` into `t.call(` |
| + only mangled locals as wildcards | 11,639 | sliding windows *inside one* array literal counted as 29 separate occurrences |
| + self-overlap and empty windows excluded | 5,600 | counted occurrences whose bindings share a scope, where two bindings can never share a name |

Only the resolver knows the legal set, so the census runs inside the compiler:
`idiom_census_over_an_artifact_file` (`src/js_peephole/tests.rs`, `#[cfg(test)]`, driven by
`LILSCRIPT_IDIOM_INPUT` / `LILSCRIPT_IDIOM_OUTPUT`), reusing `BindingResolution`,
`is_property_identifier` and `names_a_function_or_class` so a slot is a wildcard only where the
rename pass could actually respell it.

Windows of 4..14 tokens, span 12..220 bytes, at least one wildcard, occurrences of one shape
non-overlapping. Shapes ranked by value, taken greedily so no byte is counted twice. **A binding has
one name**, so an occurrence whose slot is already committed to a different spelling by an earlier
idiom is refused, not credited — first-come-wins is a greedy lower bound on the assignment a real
search would find.

Gain is priced on the bytes that actually differ, not the whole span: in `"string"==typeof t` →
`"string"==typeof e` seventeen bytes already matched and one did not. Cost is the change in the
identifier stream's first-order entropy, `ΔH × identifier_bytes / 8`.

## Result

### The model confirms, and the entropy term is a gain

| port | converted | conflicts | novel bytes | gain | entropy Δ | entropy value | **net** |
|---|---:|---:|---:|---:|---:|---:|---:|
| jquerylil | 750 | 1229 | 1042 | 448 | 3.9897 → 3.8676 | −215 | **663** |
| mobxlil | 464 | 997 | 635 | 273 | 4.2409 → 4.1341 | −138 | **411** |
| markedlil | 237 | 1063 | 308 | 132 | 3.8280 → 3.6213 | −103 | **235** |
| micromarklil | 869 | 6815 | 1574 | 677 | 4.0459 → 3.7560 | −406 | **1083** |
| katexlil | 1543 | 8730 | 1472 | 633 | 4.0791 → 3.9626 | −265 | **898** |

Entropy **falls** in every case. This is the structural difference from 056, where assigning by
first occurrence raised it from 3.9785 to 4.0663 and cost +82. Converging on the commonest spelling
is not the same operation as converging on a positional one, and only the first is free of the
entropy bill.

The idioms are real. On jquerylil, `"string"==typeof e` occurs 65 times across 60 scopes in **22
spellings**; `e,t)=>{var n` occurs 64 times across 64 scopes in 21 spellings.

### The codec refuses it

Applying the greedy assignment to the artifact and measuring:

| port | model | **codec brotli** | codec gzip | renames applied |
|---|---:|---:|---:|---:|
| jquerylil | −663 | **+177** | +181 | 124 / 915 (14%) |
| mobxlil | −411 | **+36** | +60 | 41 / 545 (8%) |
| markedlil | −235 | **+10** | −1 | 15 / 208 (7%) |
| micromarklil | −1083 | **−81** | −176 | 253 / 855 (**30%**) |
| katexlil | −898 | **+269** | +293 | 125 / 1155 (11%) |

The last column is the explanation. The emitter lands a rename only under the *sufficient*
condition — the target spelling occurs nowhere in the scope's whole extent — which refuses 70–93% of
the planned assignment. A partial application breaks the matches the artifact already had without
completing the ones it was aiming for, and the entropy gain, which is only realised when the whole
assignment lands, mostly does not arrive.

**micromarklil is the tell**: by far the highest application rate, and the only port that improves.
Weak evidence, one port, inside the ±100 band — but it is the only direction the data offers.

### The pass, built, and ranked against the incumbent

`converge_idiom_names` (`rename.rs`) is the same pass as `converge_local_names` with one difference:
a binding an idiom wants spelled a particular way is offered that spelling before the canonical
sequence fills the rest. It therefore inherits every legality proof the canonical path relies on --
the scope's blocked set is already complete when the preference is honoured.

Both passes run over the finished artifact and the codec ranks all three:

| artifact | incumbent | canonical pass | idiom pass | winner |
|---|---:|---:|---:|---|
| jquerylil | 28225 | **+8** | +319 | incumbent |
| mobxlil | 15578 | **−17** | +181 | canonical |
| markedlil | 9444 | +32 | +34 | incumbent |
| micromarklil | 26097 | **−805** | −628 | canonical |
| katexlil | 64907 | **+3** | +384 | incumbent |

### The dose-response, which is what settles it

Raising the recurrence a shape must show before it may move a name softens the loss but never
reaches the canonical pass (jquerylil: occ≥4 +319, ≥8 +226, ≥16 **+127**, ≥32 +156, ≥64 +155).
Capping how many bindings the census may claim isolates the mechanism:

| bindings claimed | rewrites | brotli |
|---:|---:|---:|
| 1 (a no-op) | 836 | +8 — identical to the canonical pass |
| 4 | 852 | **+29** |
| 16 | 945 | **+43** |
| 64 | 1458 | **+44** |
| 256 | 2982 | **+138** |

**Monotone from the smallest possible dose.** Four claimed bindings already cost +21 over the
baseline. There is no threshold at which this pays, so no amount of tuning reaches one.

### Where the model was wrong

The dose-response also names the two errors, and both are visible in that table:

1. **Displacement was never priced.** Claiming a name blocks it, so every binding the canonical
   sequence would have given it to moves too. 124 claimed bindings produced **4,674 rewrites**. The
   entropy model summed only over the bindings the census renamed, not over the far larger set it
   displaced -- so it counted the benefit of the claim and none of the cost of the cascade.
2. **`lambda` was applied to the wrong text.** 0.43 bytes per novel byte was measured on mixed novel
   content. A single-letter identifier in a saturated context is already coded at roughly two bits,
   not 3.4, so the gain from de-novelising it is overstated by about half.

## Verdict

The claim survives as a model and fails as a measurement, and the two are reconciled by the
application rate rather than by the idea being wrong. What is established:

- **Small idioms are spelled apart, at scale, and the resolver-checked convergeable set is real** —
  not the artefact three cruder probes reported.
- **Converging on the commonest spelling lowers identifier entropy** (−103 to −406 bytes of value
  per port). This is a genuinely different operation from 056's positional convergence, and it is
  the reason this hypothesis was worth separating from that one.
- **Under a sound but conservative legality rule the net effect is negative on four ports of five.**
  No optimiser should be built on the model number alone.

### The result that reverses the verdict

Everything above rewrites a *finished* artifact. The compiler does not work that way: the terminal
cleanup offers a candidate against each beam member, mid-search, and the search continues from
whatever wins. Those beam members are not the finished program -- they are pre-cleanup, pre-fold,
differently shaped -- and the idiom conversion is worth something there that it is not worth at the
end.

markedlil, level 15, `LILSCRIPT_IDIOM_NAMING=1`, one binary, one variable:

| | raw | gzip9 | brotli11 |
|---|---:|---:|---:|
| knob off | 34663 | 10521 | 9379 |
| knob on | 34562 | 10454 | **9326** |
| delta | **−101** | **−67** | **−53** |

`LILSCRIPT_TIMING=1`: `idiom_candidates 8`, **`idiom_won 5` (sum 150)**, `idiom_lost 3` (sum 108).
The candidate is offered eight times, wins five, and the artifact ends 53 Brotli smaller.

On the pool, one binary, the knob the only variable:

| port | brotli off → on | raw | gzip | brotli |
|---|---|---:|---:|---:|
| markedlil | 9470 → 9393 | −101 | −67 | **−77** |
| motionlil | 50550 → 50550 | +0 | +0 | **+0** — offered, never won, incumbent survived |
| jquerylil | 28436 → 28436 | +0 | +0 | **+0** — same |
| katexlil | 64907 → 64989 | +26 | +9 | **+82** |

One win, two exact ties, one loss, and the largest of them is 82 bytes against a ±100 floor. The
two ties are the interesting rows: on both ports the candidate was proposed against every beam
member and beaten by all of them, so the artifact came out unchanged to the byte. Where the
candidate cannot help it costs nothing but a codec probe.

### A claim of mine, retracted

I said the scored slot makes the knob's floor a tie: the codec ranks the candidate and the incumbent
survives when it loses, so nothing can get worse. **katexlil disproves that**, and the reason is
recorded in status.md already — the terminal search is basin-sensitive. The slot does guarantee each
individual comparison. What it cannot guarantee is the end of the run: a candidate that wins one
comparison enters the beam, the search continues from there, and a locally better artifact can lead
to a globally worse one. Per-step monotonicity is not end-to-end monotonicity when the steps that
follow depend on what won.

So the correct statement is narrower: the candidate never *replaces* a cheaper artifact at the point
it is offered, and the run as a whole can still end either way. That is the difference between a
guarantee and a tendency, and only the fleet can tell you which one you have.

All three deltas are inside the ±100 noise floor, so none of them is evidence on its own. The knob
stays off.

This is why the slot matters. A conversion that loses on the finished artifact is not a conversion
that loses in the pipeline, and the only way to tell the two apart is to let the codec rank it where
it is actually proposed. Nothing about the dose-response above is wrong; it measures a different
question.

## What landed

`javascript.idiom_directed_naming`, **default off pending a fleet measure**, wired into the terminal
cleanup beside the canonical convergence (`compiler.rs`). The slot is what makes it safe: the
candidate is scored by the codec and the incumbent survives whenever it does not win, so the knob's
floor is a tie and it can never regress an artifact. `LILSCRIPT_IDIOM_NAMING=1` forces it on for
measurement without touching a port's config, and `workers.mjs` now forwards that kind of switch to
the pool.

Counters `idiom_candidates` / `idiom_won` / `idiom_lost` / `idiom_idle` report under
`LILSCRIPT_TIMING=1`. They were declared and then silently unreported at first — `EVENT_BUCKETS` is
a fixed-size array and a `Bucket` that is not listed in it never prints, which is exactly the class
of defect 036/041/043/047 keep finding. Fixed in the same branch.

**Verified output-neutral with the knob off**: built from a clean worktree at this commit against its
parent, markedlil is byte-identical (34663 / 10521 / 9379 both sides). The +26 an earlier
working-tree build showed was a concurrent session's uncommitted `codegen_ir_js.rs`, not this
change.

Sweep knobs `LILSCRIPT_IDIOM_MIN_OCC` and `LILSCRIPT_IDIOM_MAX_BINDINGS` reproduce the dose-response
above in one command. 1700 compiler tests green.

## Gates

Correctness is not in question. Built with the knob on and run against each port's own suite:
**markedlil 29/29**, **katexlil 1230 tests + 123 snapshots, all green**. 1700 compiler tests green.
With the knob off, markedlil is byte-identical to the parent commit built from a clean worktree.

## Next, if anyone re-opens it

One bounded experiment decides it. The emitter's legality rule is the strong sufficient condition;
`converge_local_names` already implements the *precise* one — Terser's question, "which names spoken
here resolve somewhere else?" (`rename.rs:136-160`) — which permits shadowing wherever nothing reads
the outer binding. Porting that into the census emitter should raise the application rate from
7–30% toward the rate the pass itself achieves, and the codec can then rule on a full assignment
rather than a tenth of one.

Confirms at ≥ −150 on two ports with a full application. Falsifies otherwise, and then the greedy
assignment is wrong rather than merely partial, and the conflict counts (1229–8730) say a search
would have to do the work a greedy pass cannot.
