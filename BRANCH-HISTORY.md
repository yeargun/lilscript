# Why `main` moved on 2026-09-01

`main` was reset to the branch that had the actual working history. The previous `main` is not
deleted — it is preserved as **`main-pre-2026-09-01`** (`fda9f11`) and can be restored or merged at
any time.

## What happened

Both lines start from the same commit:

```
bb413e0  2026-08-29  Pin terminal admission candidate
```

From there the repository ran as **two parallel lines that never met**:

| | old `main` (`main-pre-2026-09-01`) | this line (now `main`) |
|---|---|---|
| commits after the split | 30 | 56 |
| dates | 2026-08-29 → 08-30 | 2026-08-29 → **09-01** |
| where the work was done | pushed from elsewhere | this checkout |

Nobody force-pushed and nothing was lost: the working checkout simply kept committing from
`bb413e0` while a second line was pushed to `main`. It went unnoticed for three days because the
local branch never needed to pull.

**The reason for the reset is simply that the work is here.** Every hypothesis, measurement and fix
from 2026-08-31 and 09-01 — the whole `finer/` investigation (then `auto-finer-lilscript/`), 32 numbered folders —
exists only on this line. The old `main` stops on 08-30 and has none of it.

## What the old `main` has that this line does not

30 commits, entirely inside the compiler, touching 1904 insertions across 9 files. Two themes:

**Artifact admission, 08-29** (`fd3325b` → `3331a65`): enforce JavaScript syntax floors, reject
syntax and external drift, track target property provenance, resolve final property byte ranges,
validate bundle artifacts before scoring, expose a final artifact witness, report binding and dynamic
property ranges.

**Scoring and registry consolidation, 08-30** (`172851b` → `fda9f11`): recover direct library
incumbents, centralize generated JavaScript scoring, unify candidate ordering, move phase ordering
and compression contrasts into `decision_registry`, unify final challenger acceptance and terminal
JavaScript acceptance.

Concentrated in `src/compiler.rs` (+1458), `src/js_peephole/mod.rs` (+410) and
`src/decision_registry.rs` (+370).

**This overlaps directly with work on this line, and that matters.**
[031](finer/hypotheses/031-admission-blocks-the-class-rewrite/README.md) and
[032](finer/hypotheses/032-export-resolver-false-negative/README.md) diagnose two admission
validators wrongly refusing the peephole's `class` rewrite — worth 194 Brotli on micromarklil and 769
on mobxlil. Three of the old-`main` commits touch `validate_observed_javascript_artifact` and
`generated_javascript_export_witnesses`, the exact functions involved. It has now been read, and the two validators come out differently:

- **031's property census — the old line redesigns it.** Where this line compares the candidate's
  static property names against the *direct emission's* set, the old line checks each observed
  property against a **`property_provenance`** list carried from source, and reports byte ranges
  rather than bare names. That is the better model: a property is legitimate because the source
  produced it, not because one particular lowering happened to emit it. This line's `constructor`
  exemption is a patch on the design the old line replaces, so **the old line's version should win
  a merge** — with the open question of whether a class body's `constructor` appears in its
  provenance list, since if it does not, the same rejection returns in a new form.
- **032's export resolver — the two lines are identical.** `generated_javascript_export_witnesses`
  demands `Resolution::Bound` and raises the same `unresolved generated export binding` in both.
  **The 769-byte false negative on mobxlil exists on the old line too**, so merging neither fixes
  nor worsens it, and it has to be fixed on whichever line survives.

## What this line has that the old `main` does not

56 commits: the same 08-29/08-30 port work, plus everything from 08-31 onward.

- **Compiler fixes.** Unparseable class expressions ([023](finer/hypotheses/023-unparseable-class-expressions/README.md)),
  a dead ES-syntax floor check ([024](finer/hypotheses/024-optional-chain-floor/README.md)), a
  conditional arm swallowing the enclosing colon, and the admission fix in
  [031](finer/hypotheses/031-admission-blocks-the-class-rewrite/README.md).
- **Measurement corrections.** The size harness was comparing our *unminified* bundle against
  Terser's minified one for three ports — 10634 Brotli of reported loss that was never real
  ([028](finer/hypotheses/028-unminified-lil-lane/README.md)) — and micromarklil's build script
  was un-minifying the compiler's output
  ([030](finer/hypotheses/030-the-build-undoes-the-compiler/README.md)).
- **Infrastructure.** `fleet.mjs` (parallel build+measure of all 26 ports), `sweep.mjs` (per-port
  config search), `repeat-coverage.mjs`, `shipped-vs-compiled.mjs`, `parse-check.mjs`,
  `LILSCRIPT_VALIDATE_FOLDS`, `src/timing.rs`.
- **32 hypothesis folders**, including the falsified ones, and `objective.md` — the contract this
  workstream is held to.

## If the old line should come back

```sh
git log --oneline main-pre-2026-09-01        # read it
git merge origin/main-pre-2026-09-01         # conflicts in compiler.rs, js_peephole/{mod,tests}.rs
```

A merge was attempted on 09-01 and conflicts in those three files; it was not resolved because the
overlap is in scoring and admission logic that both lines rewrote, and merging it blind would need
the full fleet re-measured to know what it cost. That reconciliation is worth doing deliberately —
see the note above about 031/032.
