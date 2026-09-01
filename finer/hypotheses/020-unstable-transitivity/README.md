# 020 — Narrowing the `unstable` transitive closure

**Status: IMPLEMENTED, SOUND, MEASURED, REVERTED.** Fewer unstable values does **not** mean fewer
bytes. That is the finding.

## Hypothesis

[019](../019-one-mechanism/README.md) established that `unstable` is 48–54% of stored values across
both losing port families, and that instability is a **transitive closure**: one observable property
read poisons every value downstream, and each poisoned value takes its own name and statement.

`unstable_values` (`src/codegen_ir_js.rs`) propagated from *every* unstable operand:

```rust
|| op_values(&instruction.op).iter().any(|value| unstable.contains(value))
```

That looked over-conservative. Instability means "cannot be deferred to the use site". If an operand
is unstable **and used more than once**, it is stored and named before this instruction runs, so this
value's expression refers to a *name* — deferring it moves nothing observable. Only a **single-use**
operand can be inlined here, and only then would deferring drag the operand's observable evaluation
later.

## The soundness argument, checked rather than assumed

The refinement is only sound if `unstable ∧ use_count > 1 ⟹ stored`. Checking the store condition:

```rust
|| (use_count > 1 && !structured_iteration_input)
```

and `structured_iteration_input` is itself defined as `use_count == 1 && …`, so for any multi-use
value it is `false` and the clause reduces to `use_count > 1` — **unconditionally stored**. The only
escape is `inlined_values`, which holds `Const(Int|Float|Bool)` literals; a `Const` has no operands
and is never observable, so it is never unstable.

The implication holds. **1631 tests pass**, including the behavioral `node_stdout` cases.

## Measured

`unstable` on micromarkLil drops **1139 → 1003, a 12% reduction**, exactly as predicted. Bytes:

| port | Brotli delta |
|---|---:|
| micromarkLil *(search off)* | **−56** |
| mobxlil | **−38** |
| remark-breakslil | **−3** |
| remark-mathlil | 0 |
| markedlil | **+29** |
| **jQueryLil** | **+81** |
| **net across shipped ports** | **+69** |

Sound, does what it says, and **net negative**.

## Why — and this matters for 019

Fusing a value into its consumer is not free. A named temporary used once costs its declaration plus
one reference; fusing it substitutes the whole expression at the use site. When that expression is
longer than the name, or when the name was helping a later fold or the compressor's match history,
fusing *loses*. The bigger the artifact, the more the reshuffle dominates — which is exactly the
ordering seen above, with the two largest ports the two that regress.

**So 019's "90% is `unstable` + `cross_block`" identifies the right mechanism but not a lever that can
be pulled directly.** Reducing the unstable count is not a proxy for reducing bytes. Any future work
here has to be scored per candidate against the real codec, not reasoned about from the census.

## Reverted

By the discipline from [010](../010-string-pool-alias-pricing/README.md): a change whose measured
effect is smaller than its cross-port variance has not been measured yet. Here the spread is −56 to
+81 and the net is positive — worse than a wash.

This time the check happened **before** committing, and the two ports that mattered were the two
biggest. 010's mistake was generalizing from one small artifact; that is what the jQueryLil
measurement is for, even at forty minutes a run.
