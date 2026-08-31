# 016 — Bisecting a 4.7% size regression in markedlil

**Status: BISECTED to one commit and one file, with the symptom characterized and three candidate
mechanisms falsified. Handing over rather than "fixing" someone else's in-flight change.**

## Why this was chased

[015](../015-does-this-work-help/README.md) found that markedlil's *committed* artifact is smaller
than what either `lilscript` HEAD or this workstream produces from **identical source and config**.
That is the signature of a compiler regression, and markedlil is the ideal subject: the objective
names it explicitly, its source has not been touched, and it compiles in about a minute.

## Setting up a clean predicate

markedlil ships one artifact per objective, and crucially `dist/marked.bytes.js` (raw objective) and
`dist/marked.gzip.js` (gzip objective) are **committed, unbundled compiler output** — no `esbuild`
step in between. So they give an exact compiler-to-compiler comparison with no bundler variable.

| artifact | committed | current HEAD | regression |
|---|---:|---:|---:|
| `marked.bytes.js` (raw objective) | 34135 raw | 35109 raw | **+974 raw** |
| `marked.gzip.js` (gzip objective) | 10732 gzip | 10792 gzip | +60 gzip |

The raw objective is the sharper signal and the cheaper predicate, so the bisect used it.

## Bisect

`git bisect` over the 30 commits between the last known-good build date and HEAD, with a predicate
that rebuilds the compiler and compiles markedlil's frozen source:

```
probe 3ffdcf6 -> 33541 bytes
probe b778353 -> 35109 bytes
probe edbdf3a -> 33541 bytes
probe 593f048 -> 35109 bytes
probe bb413e0 -> 33541 bytes
593f048 is the first bad commit
```

**`593f048` "Advance Vue port and library benchmarks" (2026-08-29 22:46).**

The bisect also revealed the regression is **larger than the committed artifact suggested**. The true
pre-regression size is **33541**, not the 34135 in `dist/` — so `dist/` was already built by a
slightly regressed compiler. Against the real baseline:

**33541 → 35109 is +1568 bytes, a 4.7% regression on the raw objective.**

## What the commit touched

Despite its message, `593f048` is not benchmark-only. It changes nine compiler files:

| file | lines |
|---|---:|
| `src/compiler.rs` | 76 |
| `src/lower.rs` | 76 |
| `src/js_peephole/mod.rs` | 53 |
| `src/js_peephole/binding.rs` | 40 |
| `src/js_peephole/scope.rs` | 32 |
| `src/module.rs` | 9 |
| `src/parser.rs` | 8 |
| `src/ast.rs` | 2 |

## First hypothesis, falsified

The most visible change is in `JavaScriptArtifactAdmission::validate`, which gained a call to
`validate_generated_javascript_syntax_floor`. That is suggestive for two reasons: an admission gate
that rejects candidates during scoring would make the search fall back to worse ones, and it is the
*same function* whose test (`rejects_generated_syntax_above_the_configured_floor`) has been failing
at HEAD throughout this workstream.

**Falsified by measurement.** markedlil sets no `ecmascript`, so it defaults to `es2022`; recompiling
with `ecmascript = "esnext"` — which cannot reject anything — still gives **35109**. The floor gate
is not rejecting any candidate here, so it is not the cause.

## Narrowed to one file

Reverting each changed file individually to its parent version, rebuilding, and re-measuring:

| file reverted to parent | markedlil raw |
|---|---:|
| `src/lower.rs` | 35109 (no effect) |
| `src/js_peephole/binding.rs` | 35109 (no effect) |
| `src/js_peephole/scope.rs` | 35109 (no effect) |
| **`src/compiler.rs`** | **33541 — recovered** |
| `src/js_peephole/mod.rs` | does not build alone |
| `src/parser.rs` | does not build alone |

**`src/compiler.rs` alone carries the regression.**

## The symptom

Diffing the two artifacts structurally — same source, same config, one file of compiler difference:

| | good (33541) | bad (35109) | delta |
|---|---:|---:|---:|
| `;` | 425 | 904 | **+479** |
| `,` | 1479 | 1091 | **−388** |
| `var` | 94 | 250 | +156 |
| `let` | 10 | 115 | +105 |
| identifier occurrences | 6694 | 7135 | +441 |
| distinct identifiers | 421 | 464 | +43 |

**The good compiler merges statements into comma-sequenced expressions; the bad one splits them into
separate declarations.** 479 statements' worth. This is the same statement-density mechanism
[013](../013-statement-density/README.md) identified as jQueryLil's whole remaining gap, appearing
here as a regression — which makes it the cheapest available handle on that mechanism.

## Three mechanisms falsified

1. **The syntax-floor gate the commit added.** Recompiling with `ecmascript = "esnext"`, which cannot
   reject anything, still gives 35109.
2. **Admission rejecting candidates.** An instrumented build counts **201 validations and 0
   rejections** on markedlil. The gate is not discarding anything.
3. **Budget displacement.** The bad revision does use fewer terminal probes (93 vs 100 against the
   same 264 limit), so admission work does consume budget — but raising
   `terminal_codec_probe_limit` to 512 or 1536 gives **35105**, recovering 4 bytes of 1568. Budget is
   not the constraint.

## What the telemetry does show

`--explain json` at both revisions, everything else equal:

| metric | good | bad |
|---|---:|---:|
| plans registered | 262 | 262 |
| emissions attempted | 250 | 250 |
| scored emission families | identical sets | identical sets |
| starved families | 30 | 30 |
| IR variants searched | identical | identical |
| **candidates evaluated** | **26** | **20** |

Same plans, same emissions, same families — but **six fewer candidates reach evaluation**, and they
are evidently the comma-sequencing ones. Candidates are being lost between emission and evaluation
through a path that is *not* `admission.validate` returning an error.

## Why this stops here

`593f048` is the owner's own in-flight work, and its code comments say parts of the change are
deliberate: *"Candidate rewrites must preserve the callable ABI emitted by the direct lowering. Some
LilScript defaults are materialized in function bodies, so their JavaScript `length` intentionally
differs from the typed arity."* The change swaps the ABI oracle from the declared manifest to the
witnesses of the direct emission. Reverting that unilaterally could reintroduce whatever it was
written to prevent, so this is a report, not a patch.

## Reproduction

```sh
git worktree add /tmp/bisect 593f048 && cd /tmp/bisect
cargo build --release
./target/release/lilscript ~/markedlil/src/entry.lil --target js-module \
  --config ~/markedlil/lilscript.bytes.toml -o /tmp/bad.js      # 35109
git checkout 593f048^ -- src/compiler.rs && cargo build --release
./target/release/lilscript ~/markedlil/src/entry.lil --target js-module \
  --config ~/markedlil/lilscript.bytes.toml -o /tmp/good.js     # 33541
```

One minute per compile, frozen source, no bundler in the loop.

## Note on the failing test

The syntax-floor test failure is therefore a **separate** pre-existing bug from this regression, not
the same one. Worth recording so the two are not conflated: `validate_generated_javascript_syntax_floor`
does not reject ES2022 class fields under an ES2021 target, and that has been failing since before
this workstream (verified against a clean HEAD checkout).
