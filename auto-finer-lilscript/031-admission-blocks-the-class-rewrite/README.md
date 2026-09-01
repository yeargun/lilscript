# 031 — Why the class rewrite never lands: admission counts `constructor` as a new property

**Status: 018'S MECHANISM FOUND, and the obvious fix MEASURED AND REJECTED. The gate is wrong in
principle and load-bearing in practice, which means something downstream is worse than it looks.**

## The observation

The compiler's own output for micromarklil contained **434 `;var ` runs** that its own peephole
merges. Feeding `dist/micromark.raw.js` back through `optimize_generated_javascript`:

| | raw | Brotli | `;var ` |
|---|---:|---:|---:|
| as the compiler emitted it | 97516 | 27907 | 434 |
| one pipeline pass over that | 93942 | **27736** | 117 |
| | **−3574** | **−171** | |

The pipeline does not even converge — rounds 2 through 5 keep shrinking it (93942 → 93934 → 93926 →
93918 → 93901). So the shipped artifact was leaving a measured 171 Brotli on the table.

## Why it was being skipped

`apply_selected_canonical_peephole` exists precisely to prevent this. Its own comment says so:

> Search probes bound permutation scoring. The selected artifact still gets one canonical rewrite:
> otherwise a full ledger leaves ParsedPeephole as a no-op on the winner, even when that rewrite is
> cheaper.

Two suspects, and tracing settled it. The first was real but not the cause: the two work units were
charged against the same ledger as the codec probes, so a big artifact — the kind with most to gain —
spent its budget on permutation scoring and then skipped the rewrite it is supposed to be guaranteed.
Un-gating that is kept (mobxlil moves −1 byte, 1634 tests pass), but it changed nothing here, because
of the second:

```
CANONICAL: rejected at admission: generated JavaScript introduced
unclassified static properties: ["constructor"]     (x4)
```

**Admission rejects the rewrite because the class it produces has a `constructor`.** The rewrite
spells `function X(){...}` plus its prototype table as `class X{constructor(){...}}`, and the
property-name census counts that class element like any other property. Every object already carries
`constructor` through its prototype chain, so no candidate can make it newly observable and there is
nothing there for property mangling to get wrong.

**This is what [018](../018-mobx-admission-regression/README.md) spent seven falsified mechanisms
hunting.** It instrumented admission and reported *150 validations, 0 rejections* — but it counted at
the scoring gate, not at this one, and concluded the class-bearing candidate was "never generated or
scored". It was generated, scored, and refused, here, over a keyword.

## The fix, measured and rejected

Exempting `constructor` does exactly what it should — the rewrite lands, `;var ` falls 434 → 117 —
and the artifact gets **worse**:

| | `raw.js` raw | `raw.js` Brotli | shipped `esm.js` Brotli |
|---|---:|---:|---:|
| admission unchanged | 97516 | **27907** | **26508** |
| `constructor` exempted | 94564 | 29061 | 27334 |
| | −2952 | **+1154** | **+826** |

Nearly 3 KB of raw disappears and Brotli gets 1154 bytes worse. So the gate is wrong in principle and
was accidentally protecting the artifact. **Reverted** — shipping a measured regression to fix a
mis-stated check is the wrong trade.

## What that leaves, precisely

The same transform is **−171 Brotli** applied to the finished artifact and **+1154** applied where
the compiler applies it. That difference is the finding: it is not the rewrite that is bad, it is
where it happens. Something between `apply_selected_canonical_peephole` and the emitted file — naming,
chunk assembly, the module wrapper — turns a compressor win into a loss, and the candidate is scored
before that stage runs, so the scoring cannot see it.

Two concrete follow-ups, in order:

1. **Score the artifact that ships.** The candidate's `transfer_cost` is measured on `selected.code`
   at that point in the pipeline, not on the bytes that reach disk. Any transform after it is
   unscored, and this is a 1325-byte demonstration that the two disagree.
2. **Then fix admission.** `constructor` is genuinely not a property introduction, and once (1)
   holds, exempting it should be a −171 win rather than a +826 loss.

Ports touched: none regressed. micromarklil ends this hypothesis at **26508** Brotli
(26930 at session start), 1963/1963 tests passing.
