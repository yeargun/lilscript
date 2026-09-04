# 004 — Legality by construction

Parent: [index](index.md). Directive: [D1](001-directives.md#d1--correctness-by-construction-not-by-conditional).

> "Make sure that you create a correct system apart from optimizing suboptimal solutions."

This is the document that answers that. Every way this compiler is known to produce a wrong program,
and — honestly — whether the new design makes it *impossible* or merely *less likely*.

A design that relocates a conditional has failed. So the last column is the important one.

---

## The status vocabulary

| Status | Meaning |
|---|---|
| **unrepresentable** | The illegal state has no encoding. There is nothing to check. |
| **compile-error** | Constructing it does not build — usually an exhaustive match or a required field. |
| **checked-at-build** | A total function or generated table rejects it during compilation of the artifact. |
| **detected** | It can be built, and a witness catches it. Better than today, but still a runtime property. |
| **still-possible** | Honest admission. |

---

## The matrix

### 1. Identity recovered from spelling

**Today.** The optimizer collects every `extern` keyed on its **source spelling** and matches it
against 105 hardcoded names. `extern float mathRound(float)` silently becomes `Math.round`;
`extern void noop()` has its call deleted. Reproduced. `ExternDecl` carries only `name` and
`FunctionKind::Extern` is a unit variant, so spelling is the only identity that reaches the
optimizer.

**Under the new design: `unrepresentable`.**
Host knowledge keys on a resolved `HostBindingId` carried by the declaration
([003](003-target-representation.md#the-extern-host-binding)). An extern with no declared binding has
no id, so there is no table to consult and nothing to match. The 105 names move out of `optimizer.rs`
into a LilScript prelude shipped as source.

**Residual.** A user can still declare `extern("Math.round")` for something that is not `Math.round`
— but that is now *their assertion*, in their source, visible in the ABI manifest and the fingerprint.
That is the correct place for an unchecked claim about the host.

### 2. A profitability knob changes legality

**Today.** `emits_ordinary_function_expression` conjoins the `this`/`arguments` check with
`matches!(function.kind, FunctionKind::Function)`, so a `FunctionKind::Closure` skips it and is
spelled as an arrow. Reproduced: the same source emits `function(a){return arguments}` (returns
`[1,2,3]`) or `()=>arguments` (throws `ReferenceError`).

It is **not** confined to an opt-in knob. `function_spelling = "arrow"` is set in ~35 of the 61 port
config files, and — worse — the candidate search reaches the same spelling with **no user opt-in at
all**: a `function-spelling` family is registered whose only precondition is that the user did *not*
pin the key. Investigation 061 traced the shipped consequence: `@itslil/jquery` throws on
`scrollTop()`, `scrollTop(1)` and `scrollLeft(1)`, known since 042 and never diagnosed, because the
port's six compat tests do not cover it.

**The obvious fix is the wrong fix, and this is the important part.** Forcing an ordinary function
whenever a closure reads `this` fails `nested_lexical_js_bindings_keep_the_callback_context`, which
pins the *lexical* rule for a nested closure. 061 implemented it, watched that test fail, and
reverted. The language's closure rule **is** lexical, so `a=>this` is the correct spelling and the
port was wrong to read ambient `this` there. The defect is the other direction: the `"function"`
spelling **rebinds** `this`, so the two spellings denote different programs and the search picks by
byte count. As 061 puts it — *"The program's meaning follows the byte count."*

**Under the new design: `unrepresentable`, but via a different construction than a precondition.**

> A spelling axis may only offer alternatives that **denote the same program**. `SpellChoice` values
> for one site are interchangeable by construction, and the `function` lowering of a lexical closure
> captures the lexical receiver so that both spellings agree.

`receiver_use: ReceiverUse` is still a required field on `ShapeFn` — it is what the lowering consults
to decide whether a receiver capture is needed — but it gates *how the function spelling is
lowered*, not *whether the arrow is allowed*. That is the correct reading of the organizing rule in
[003](003-target-representation.md#the-organizing-rule): profitability may choose among programs
legality admits, and two spellings that differ in meaning were never both admitted.

**Residual.** Making the two agree costs bytes wherever a receiver capture becomes necessary. That
is a real, measurable cost and it is a fleet-rule change — 061 correctly says it "wants its own
folder". It is a byte cost paid for a meaning guarantee, which is the trade
[D1](001-directives.md#d1--correctness-by-construction-not-by-conditional) requires.

### 3. Structure guessed from tokens — the loop latch

**Today.** Two of the three shipped wrong-program folds are loop-header reconstructions
(`fold_while_trailing_increments`, `fold_for_trailing_increments`) that guess which statement is the
latch by scanning tokens backwards for `?`, `:` or `=>` at depth 0. One lifted an `h++` out of a
conditional; the record notes "no gate refuses" it.

**Under the new design: `unrepresentable`.**
`Loop { header, body, latch: Option<Node>, exit }` carries the latch, copied from
`ControlShape::Loop { update }` — which the IR already computes and the emitter already discards.
There is no scan to get wrong.

**Residual.** None for this class. The fact exists upstream and is simply carried.

### 4. Structure guessed from tokens — scope

**Today.** The third shipped miscompile (`declare_implicit_assignment_bindings`, the one that forces
zodlil to `optimization_level = 8`) was a scope guard that missed a binding the name actually
resolves to. Compounding it, `src/js_peephole/scope.rs` admits **two coexisting scope rules** — the
sibling-leak check considers only real brace bodies while enclosing visibility uses a broader legacy
span rule — with no test asserting they agree. And `rustc` reports
`identifier_is_function_parameter` and `identifier_is_catch_parameter` as **unused**: folds are making
binding decisions without the parameter checks written for them.

**Under the new design: `unrepresentable`.**
Identifiers name a `Bind`, not a string. There is no scope model to be wrong, because there is no
resolution step — the binding was never lost. `rename_ambiguous`, which fired on **14 of 14
candidates on cnlil**, has nothing to be ambiguous about.

**Residual.** Scope construction during lowering can be wrong. But it happens once, in one place,
with the SSA scope in hand — not per fold, per token scan.

### 5. Grouping guessed from tokens

**Today.** `parse_single_assignment` accepted a comma sequence as a single assignment, so a guarded
branch body became the right-hand side of `||` and ran unconditionally (`ident-06`). The general
class: depth counting that does not distinguish string, template and regex context.

**Under the new design: `unrepresentable`.**
`Seq(List)` cannot hold a hole; `Cond` cannot hold an absent arm; `Place` cannot hold a `Call`. The
nine folds of [G2](007-fold-disposition.md) exist to repair states of exactly this shape, and they
become unreachable rather than unnecessary.

**Residual.** None for the encoded cases. A transform can still build a *legal* tree that means
something different — that is class 12.

### 6. A semantic decision taken from rendered text

**Today.** The emitter decides by reading strings it just wrote. Confirmed cases: loop spelling by
`out.matches("for(").count() > out.matches("while(").count()` **over the whole accumulating buffer,
matches inside string literals included**; `is_rendered_string_literal(&rhs.code)` and
`is_constant_literal()` inside `JsExpression::binary`; `without_explicit_tostring` stripping a `+""`
suffix from rendered text; `host_wrapper_inline_is_profitable` testing `code.contains("typeof")`;
`arrow_block_as_object_method` using `code.find("=>{")`. Roughly 1,487 substring-scan sites in total.

**Under the new design: `unrepresentable`, and enforced.**
`Shape` nodes carry no text, so there is no string to inspect. The enforcement is structural: the
node struct has no `String` field, so a decision-from-text does not typecheck.

**Residual — and this is the one to watch.** The `Spell` level *does* produce text, and a lowering
could in principle read it back. The rule is that `Spell` lowering is a **single downward pass** with
no sibling text visible, and the printer is the only consumer of a rendered string. This is a design
discipline backed by the type (the printer takes `&Spell` and returns `String`; nothing takes both).
It is weaker than the guarantees above, and it is named here rather than glossed.

### 7. Obligation verified by counting

**Today.** `PreserveJavaScriptBitOrZero` — the mechanism guaranteeing a source-written `value | 0`
survives optimization — is verified by counting `|` followed by `0` token pairs and comparing
`observed >= expected`. It cannot distinguish a source obligation from a compiler-generated
normalization. A load-bearing correctness mechanism implemented as a substring census.

**Under the new design: `checked-at-build`.**
`HAS_OBLIGATION` is an inline fact on the node; the kind lives in a sparse table keyed by `Origin`.
The check becomes "every node with an obligation still exists and still carries it", walked over the
tree — an exact statement about the program rather than a count of characters.

**Residual.** A transform that rebuilds a node must propagate the flag. That is the `descend`
contract's job, and it is exactly what the IR verifier already checks for `ValueId`s. It is a
maintained invariant, not a free one.

### 8. A stage silently does nothing

**Today.** 124 production sites discard errors via `.ok()?`, `if let Ok`, `let _ =`. Four
"a stage silently never ran" bugs are on record. `repair_late_javascript_candidate` runs nine text
fixups, swallowing every error, called from thirteen sites, with five of them duplicated in a second
hand-maintained list.

**Under the new design: `detected`, not prevented.**
The honest answer. An `Outcome { Applied(n), Declined(reason), Failed(error) }` that is `#[must_use]`
makes dropping a failure a compile error at the *call site*, and `Failed` lands in
`JavaScriptSelectionMetrics`. But nothing forces a pass to actually do its job, and
`Declined(reason)` is legitimate and common.

**Residual.** Real. A pass that declines everything for a bad reason still reports green. The
mitigation is telemetry plus the starvation report, not a type. Stated so nobody mistakes this row
for the ones above.

### 9. Registry drift

**Today.** `IrJsOptions` has **80** `pub` fields; `IR_JS_OPTION_FIELDS` classifies **77**; the guard
is `assert_eq!(IR_JS_OPTION_FIELDS.len(), 77)` — a hardcoded table compared to a hardcoded literal
that never looks at the struct. Two of the three unclassified fields are ABI-affecting. Four
documents publish "all 77 are classified". Separately, `let ordinary_records_safe = false` disables
an axis permanently while a test **asserts the axis stays empty**, pinning the no-op.

**Under the new design: `compile-error`.**
One declarative macro generates both the struct and the classification table. A field without a class
is a syntax error. The length assertion deletes — the property becomes structural rather than
asserted.

**Residual.** The *classification* can be wrong (a field marked `Scored` that is really `Abi`). That
is a judgement, and judgements need review, not types.

### 10. A forked rewriter drifts

**Today.** `compress_passes.rs` holds a copy of `rewrite_control_flow_function` that omits parameter-
default and `ControlShape` operand rewriting — the exact omission the original's comment warns about
("a dangling SSA reference even though every executable operand was rewritten correctly"). Reachable
from two passes. Underneath it: **171 wildcard `_ =>` arms over a 35-variant `ControlFlowOp`**, so a
new IR node is silently ignored in 171 places.

**Under the new design: `compile-error`.**
One `IrIds` traversal — `for_each_value_mut` / `for_each_block_mut` — with exhaustive matches and no
`_` arm, and every rewriter goes through it. Adding a 36th variant fails to compile until handled in
one place. The fork is deleted, not repaired. `ControlShape` has exactly 25 id positions (20
`BlockId`, 5 `ValueId`), so the traversal is bounded and knowable.

**Residual.** Someone can still write a new hand-rolled match with a wildcard. A lint or a
`#[non_exhaustive]`-style discipline on the IR crate boundary
([009](009-phases.md)) narrows it; nothing eliminates it entirely.

### 11. A gate reports a false green

**Today.** The release gate runs zero port suites. 18 of 25 ports never rebuild before testing.
mobxlil tests a `--dev` build; zodlil tests at level 8 with the fold layer disabled. `debug_assert` is
compiled out of every fleet build. katexlil's mtime cache skipped a compile and **published a false
byte-identical row**. Full evidence: [002](002-the-instrument.md).

**Under the new design: `checked-at-build`, via Phase 0.**
Not a property of the representation at all — a property of the harness. Under-10-second builds and
missing timing lines fail the run; arm-keyed logs; compiler digests recorded with every number; the
fingerprinted `migration-incumbent` / `migration-candidate` lanes with `maxRegressionBytes: 0`.

**Residual.** A harness is code and can be wrong. This is why Phase 0's own exit criterion is
adversarial: *reverting a known-good compiler commit must turn the corresponding ports red.* A gate
that cannot fail on demand is not a gate.

### 12. A transform builds a legal tree that means something else

**Today.** This is the class that shipped three times. The source says it plainly:

> "`LILSCRIPT_VALIDATE_FOLDS` catches a fold that emits JavaScript the parser rejects. It cannot
> catch the worse kind: a fold that emits **valid** JavaScript for a different program. Nothing
> announces that one either — the artifact ships and a port's test suite fails."

**Under the new design: `still-possible`.**

This is the honest bottom of the matrix. A tree makes *encoding* errors unrepresentable. It does not
make *reasoning* errors impossible. A transform with binding identity, delivered facts and an exact
latch is far less likely to be wrong than one guessing from tokens — but "less likely" is not
"impossible", and no representation delivers that.

**What actually addresses it** is not the representation but [Phase 0.3](002-the-instrument.md#03--make-the-differential-harness-generative):
a generative differential harness whose domain includes classes, closures and prototypes, with a
random seed and a shrinker. That is why [009](009-phases.md) puts it **before** the fold migration
rather than after. You do not migrate 139 transforms without something that can tell you a migrated
transform still means the same thing.

---

## Scorecard

The twelve classes above were the hand-built set. An adversarial pass over the whole corpus — two
independent architecture syntheses, three attackers on separate lenses, and a final certification —
extended it to **34 known wrong-program classes** and classified each against the design:

| Status | Count |
|---|---:|
| **unrepresentable** — the illegal state has no bit pattern | **21** |
| **compile-error** | 2 |
| **checked-at-build** | 6 |
| **detected-at-runtime** | 3 |
| **still-possible** | **2** |

Twenty-nine of thirty-four close by construction or by the build. The five that do not are the
honest part, and they are worth naming individually rather than aggregating:

**still-possible — `fold_ident_ternary_to_or`**, the third shipped wrong-program fold. Certification's
own words: *"types stop grammar errors, context errors, stale facts and repair-after-the-fact. They
do not stop a transform that legally rewrites the wrong program."*

**still-possible — G12**, ES class recovery from `JsValue` prototype tables. It may never leave the
peephole, *and if the distinct prototype-table shapes across mobxlil / katexlil / remarklil /
react-markdownlil number more than a handful it should not.* Sequenced alone and last.

**detected-at-runtime — semantic equivalence of any ported `ShapeTransform`.** Thirty-five folds port,
and each is a fresh opportunity to rewrite `a-b` as `b-a`. Types make preconditions unrepresentable;
they do not make a rewrite correct.

**detected-at-runtime — silent fact loss.** A transform that rebuilds a node instead of respinning it
drops a fact. The loss is silent *in bytes*: partial `|0` elision costs Brotli, so one lost fact
flips a whole function and surfaces as +20..+60 — inside the noise band.

**detected-at-runtime — a "neutral" retirement that is not.** Any byte change re-derives
`IdentifierAlphabet::for_code` from the artifact's ASCII histogram, so **a one-byte difference can
cascade into a wholly different identifier assignment.** This is why a neutral phase must prove
byte-identity rather than measure closeness.

---

## The verdict

Is this a correct system, or a better-guarded version of the current one?

**For encoding errors, it is a correct system.** The states that produced classes 1–6 and 9–10 have
no representation. Those are not fixed bugs; they are removed possibilities, and several of them —
the nine repairs of G2, the 105-name table, the 57 determinism sorts, the two scope rules — delete
whole subsystems rather than shrinking them.

**For reasoning errors, it is a better-guarded version, and it must say so.** Class 12 is the class
that has actually shipped three times, and no representation closes it. What closes it is a harness
that can detect it, which is why Phase 0 is blocking and why the fold migration is sequenced behind
the differential work rather than in front of it.

The design's real claim is narrower and more defensible than "correct by construction": **it removes
the entire category of bugs caused by discarding information, and it leaves the category caused by
faulty reasoning — with a much better instrument pointed at it.**
