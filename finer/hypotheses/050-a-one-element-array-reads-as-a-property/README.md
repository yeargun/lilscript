# 050 — A one-element array reads as a property, and the candidate it refuses hides a miscompile

**Status: CONFIRMED, both defects fixed** (`e5ababc`, `47e57c0`). Admission read a
one-element string array as a computed property and refused the whole late-cleanup
candidate — ~900 Brotli, but only when a port edit happened to create such a literal. The
candidate it was refusing contained a wrong-program fold, so the two defects cancelled and
neither showed in any measurement until the port improved.

## Claim

katexlil's late-cleanup candidate — the whole-artifact peephole run on the terminal
finalist — was being refused by admission for a name it never introduced. The refusal
cost the comma sequencing and the single-use function elision, ~900 Brotli, *whenever a
port edit happened to create a one-element string array*. It also hid a wrong-program
fold, because the artifact admission discarded is the one that fold would have broken.

## Prior art

- Terser has no equivalent gate: it re-prints from its own AST, so a rewrite cannot
  introduce a property name it did not resolve. Our gate exists because the property
  mangler classifies names *before* the whole-artifact rewrites run.
- Closure's `AstValidator` checks structure, not name provenance; its property renaming
  runs after every optimization pass rather than validating against a pre-pass set.
- Terser's `conditionals` performs the same `x?x:y` -> `x||y` rewrite (`booleans.js`,
  `AST_Conditional`) and guards it by rebuilding the node, so precedence is re-derived
  by the printer instead of being spliced textually as ours is.

## Evidence

Two builds regressed by ~900 Brotli from unrelated local improvements:

| build | change | Brotli | functions | `;` |
|---|---|---:|---:|---:|
| Q | baseline | 65100 | 117 | 1610 |
| R | compiler: borrowed pushes -> method calls | 66077 | 255 | 4160 |
| S | port: five push-runs -> array literals | 66004 | 255 | 4190 |

`LILSCRIPT_TIMING=1` on S named it:

```
late-cleanup canonical peephole refused: generated JavaScript introduced
unclassified static properties: ["katex-error"]
cleanup_canonical_refused: 1
```

`generated_javascript_static_property_names` counted any `[` STRING `]` as a computed
member. Both changes create array literals, and `["katex-error"]` is one — an array, not
a property. A `[` subscripts a value only when it opens on an expression.

With that fixed the candidate was admitted — and rendered 123 of the 130 screenshot-corpus
items differently. Bisecting the session's 113 folds (`LILSCRIPT_SKIP_FOLDS`, five rounds)
reached `fold_ident_ternary_to_or`, which rewrites `x?x:E` to `x||E` after checking only
the else arm's *first* token:

```js
b?b:a[2]?"\\ ":" "     // KaTeX's Lexer: (a[2] ? "\\ " : " ") when b is falsy
b||a[2]?"\\ ":" "      // ours: tests (b||a[2])
```

Node oracle, minimal: `f(x,y,p,q){return x?x:y?p:q}` answers `["p","X","q"]`; the folded
form answers `["p","p","q"]`. `||` binds tighter than `?:`, so the guard has to scan the
arm out to the end of its expression — the conditional can follow a member, a call or an
operator. `??` is also refused (it cannot sit unparenthesised beside `||`) and `=>`.

## Result

Both fixed. katexlil is unchanged in size — 65100 -> 65116 -> 65062 with the borrowed-push
fold now landing — because the refusal and the miscompile cancelled exactly. What changed
is that port edits no longer fall off the cliff: builds R and S were paying 900 Brotli for
improving the source. Portfolio flat (micromark +0, mobx +0, remark-gfm +1). 1230 official
tests and 123 snapshots pass.

## Follow-up

The bisection harness is worth keeping: `examples/peep.rs` (peephole one file) plus
`LILSCRIPT_SKIP_FOLDS` / `LILSCRIPT_LIST_FOLDS` turns a six-minute pool build into a
one-second reproduction. Neither is committed; both are three lines.
