# The target-tree migration

Execution design for **`arch-05`** — the last active architecture lane — and for Phase 3 of
[`planned-migration.md`](../docs/knowledge/migration/planned-migration.md), *"introduce the minimal
hygienic target JS tree."* That plan states the intent; this folder is the design and the sequencing
under it, and it inherits its rules of execution rather than competing with them.

Written 2026-09-04 against `HEAD c9d0d3c`. Citations resolve by **symbol**, not line
([D10](001-directives.md#d10--pin-citations-to-symbols-not-lines)).

---

## What this migration is actually about

The obvious framing is "replace 47,000 lines of text rewriting with an AST." That framing is wrong,
and it would produce the wrong plan. Three measurements reframe it:

**1. Three quarters of the text layer is cleanup after the emitter's own output.** Of 142 classified
fold entries, **84 exist only because the emitter emitted a shape it then has to un-emit**, 23
duplicate an IR pass or are dead, and **35** are genuine target-level transforms. You do not port
cleanup. You stop making the mess. → [007](007-fold-disposition.md)

**2. The compiler proves 21 classes of fact at the emission boundary and delivers exactly one.**
`IntegerValueAnalysis` crosses; `FunctionEffectSummary`, `FiniteValueAnalysis`, array-parameter
lengths and the escape points-to graph are computed and delivered nowhere. Then 47,000 lines rebuild
them from tokens, worse — three purity oracles now exist, and `PreserveJavaScriptBitOrZero` is
verified by *counting `|` and `0` token pairs*. → [010](010-what-this-unlocks.md)

**3. The whole text layer is worth 189 Brotli bytes on markedlil.** All 121 folds on: 9,397. All off:
9,586. That is 2.0%, and it is the total budget at risk across every group. It is not an argument
that the layer is worthless — it is an argument that **no part of it is worth a correctness risk**,
and that the migration has no reason to be rushed.

So the thesis is not "the text layer is slow." It is:

> **The compiler discards what it proved, then guesses it back. Every wrong program this compiler has
> shipped lives at that seam.**

---

## The four constraints, in priority order

1. **Correctness by construction.** The wrong-program classes this compiler has already shipped must
   become *unrepresentable*, not better-guarded. A design that relocates a conditional has failed.
   Certified against **34 known classes**: 21 unrepresentable, 8 caught by the build, 3 detected at
   runtime, **2 still possible** — and the two are named rather than aggregated.
   → [001 D1](001-directives.md#d1--correctness-by-construction-not-by-conditional), [004](004-legality-by-construction.md)
2. **Compression must not degrade.** 26 ports, 181 artifacts, 61 config files. Byte-identity is the
   only clean proof, because a semantically empty change moves Brotli by −125..+30 and every neutral
   step lands inside that band. → [001 D3](001-directives.md#d3--compression-is-a-hard-constraint-and-byte-identity-is-the-only-clean-proof)
3. **Compilation gets faster.** Emission is O(n^1.7) above ~50 KB; 97.4% of fold invocations rewrite
   nothing; the search exhausts both budgets on a 171-byte artifact. → [006](006-candidate-derivation.md)
4. **More room to optimize.** Deliver the twenty facts that currently die at the print boundary.
   → [010](010-what-this-unlocks.md)

---

## Read in this order

| | | |
|---|---|---|
| [001](001-directives.md) | **Standing directives** | Ten rules, each binding, each because something already went wrong without it. Includes the Azure pool directive. |
| [002](002-the-instrument.md) | **Phase 0: repair the instrument** | **Read this second and do it first.** Eight gates you would assume exist, and why almost none of them can currently report a true green. Plus two live wrong programs, reproduced. |
| [003](003-target-representation.md) | **The representation** | The tree itself: levels, node storage, sharing, and the type-level constructions that carry the legality argument. |
| [004](004-legality-by-construction.md) | **The correctness matrix** | Every known wrong-program class → unrepresentable / compile error / checked / still possible. The honest column is the last one. |
| [005](005-printer-and-naming.md) | **Printer and naming** | Post-layout naming (deletes `rename.rs` and the +2,113 identifier gap's blocker), and a printer that makes 22 folds unreachable. |
| [006](006-candidate-derivation.md) | **The search** | Two of three tiers are already clean. What must survive byte-for-byte, and where the speed comes from. |
| [007](007-fold-disposition.md) | **The 139 folds** | Fourteen work groups with difficulty ratings, the delete-vs-port split, and what a deletion has to prove. |
| [008](008-fleet.md) | **The build pool** | 144 cores in `lilscript-build-farm`. What exists, what is missing, the exact sweep commands, and the ports the pool cannot currently certify. |
| [009](009-phases.md) | **Phases and gates** | Nine phases, each shippable, each with an invariant, a gate and a rollback. |
| [010](010-what-this-unlocks.md) | **What it unlocks** | The 21 facts, the 8-flag annotation design, and the transforms currently refused for lack of proof. |

---

## The phases at a glance

| # | Phase | Establishes | Gate |
|---|---|---|---|
| 0 | [Repair the instrument](002-the-instrument.md) | gates that cannot report a false green | reverting a good commit turns ports red |
| 1 | Tree exists, proved against the incumbent | the tree reproduces the incumbent | IDENTICAL (output path unmoved) + witness |
| 2 | Statements, functions, module | structure leaves the `String` buffer | IDENTICAL + witness |
| 3 | **Tree becomes authoritative** | nothing re-reads emitted text | IDENTICAL + 22 folds unreachable |
| 4 | Deliver the facts | passes read proofs, not guesses | IDENTICAL |
| 5 | Naming moves post-layout | names converge; `rename.rs` deleted | DECLARED + name-request-order trace |
| 6 | The fold groups | 11 groups migrate or delete | per group: `active == 0`, then IDENTICAL |
| 7 | Candidate derivation and budgets | shared base; ladder re-measured | DECLARED |
| 8 | Retire the text layer | production never reparses generated JS | full exit criteria |

Phase 3 is the commitment point. Everything before it is reversible; everything after it is a
default-off registry row.

---

## Numbers this plan is anchored on

All measured during this investigation, on `HEAD c9d0d3c`. Absolute timings were taken on a
credit-drained burstable host and are **triage, not evidence** —
[D5](001-directives.md#d5--compiles-run-on-the-pool-not-on-this-host).

| | |
|---|---|
| Text layer vs emitter | 47,118 lines vs 38,325 — the post-processor is bigger than the backend |
| Value of the whole text layer | **189 Brotli** on markedlil (9,397 → 9,586) |
| Fold dispositions | 84 delete-upstream · 13 delete-duplicate · 10 delete-obsolete · **35 port** |
| Facts the IR proves at the emission boundary | **21**; delivered: **1** |
| Independent `lex()` sites in folds | **134** → 24,146 re-lexes per markedlil compile |
| Idle fold rate | **97.4%** on a 171-byte artifact (12,293 idle / 330 active) |
| Emission scaling | 48 KB → 380 ms; 98 KB → 1,301 ms (**3.43× for 2.05×**) |
| Search saturation | 371 plans, 366 emissions, **both budgets exhausted**, 32 of 40 families starved — on 171 bytes |
| Non-regression surface | 26 ports · 181 artifacts · **61 config files** · 7 optimization levels |
| Brotli noise floor | ±100 per build; a delta under ~150 is not evidence |
| Pool vs this host | 23-port pass: **1,533 s** on six workers vs 45–70 min **plus two timeouts** |

---

## Two things to be honest about before starting

**The instrument is broken.** The release gate runs zero port suites. 18 of 25 ports never rebuild
before testing. mobxlil tests a `--dev` build; zodlil tests at level 8 with the entire fold layer
disabled. `debug_assert` is compiled out of every build the fleet measures. A false byte-identical
row has already been published from a skipped compile. **Nothing in this repository routinely proves
that a compiler change preserves the shipped libraries' behaviour** — which is exactly how three fold
miscompiles shipped. Phase 0 is not preamble; it is the precondition.

**Seven wrong programs were found, all reproduced; three fixed so far.** Among them: the release gate
was **red at HEAD** — `optional_constructor_callback` failed to compile, and `verify-matrix.sh` runs
every case under `set -eu` — and two more fire in the *default* configuration. Two examples: the optimizer rewrites user `extern` declarations by matching their source spelling against
105 hardcoded names, and `function_spelling = "arrow"` turns a working program into a
`ReferenceError` because a legality check was conjoined with the wrong `FunctionKind`. Neither is
caught by any existing gate. They are documented in [002](002-the-instrument.md) not as bugs to fix
in passing, but as the **acceptance test for Phase 0**: when the instrument is repaired, it must
catch both.
