# finer — the loop that keeps LilScript ahead

`finer/` is the standing workstream that optimizes the **language**, the **ports** and the
**compiler** against one contract, [objective.md](objective.md). It is a loop, not a project:
measure the fleet, take the largest attributable loss, write down a claim, test one variable,
record the verdict whether or not it won, measure again.

## Read order — the context budget

A context that reads more than this before starting is spending the budget the work needs.

1. [objective.md](objective.md) — the contract. Stable.
2. [status.md](status.md) — standings, what is settled, ranked open leads. Volatile.
3. [log.md](log.md) — every hypothesis, one row, verdict first. Scan it for the idea you are about
   to have; if it is there, read that folder and nothing else.
4. `hypotheses/NNN-slug/README.md` for **the one hypothesis you are working**.

Stop there. Folders are cold storage. [refs/competitor-techniques.md](refs/competitor-techniques.md)
is opened when a technique is named, not browsed. The compiler's own documentation is reached
through the source-authority map in [docs/knowledge/README.md](../docs/knowledge/README.md) when a
file must be understood, not read ahead of time.

## Files

| Path | Holds | Written by |
|---|---|---|
| [objective.md](objective.md) | The contract | Owner intent only, recorded in [intent/](intent/) first |
| [status.md](status.md) | Standings, objective lanes, settled facts, ranked leads, known issues | Orchestrator, after each fleet measure and each verdict |
| [log.md](log.md) | One row per hypothesis: number, lane, question, verdict | Orchestrator, when a folder's Status line changes |
| `hypotheses/NNN-slug/README.md` | Claim, method, numbers, verdict — the whole experiment | The agent running it, and no one else while it runs |
| [intent/](intent/) | The owner's briefs, verbatim, dated | Appended, never edited |
| [refs/](refs/) | Competitor technique inventory; vendored Oxc sources (ignored) | The harvest task |
| [tools/](tools/) | fleet, sweep, bench, shipped-vs-compiled, repeat-coverage, new | Anyone; a tool change is an ordinary commit |
| `out/` | Generated scoreboards, artifacts, logs. Ignored | Tools |
| [templates/hypothesis.md](templates/hypothesis.md) | The folder shape. Copy it, do not improvise one | — |

One writer per file. A hypothesis agent writes only its own folder; the orchestrator updates
`log.md` and `status.md` after reading the agent's return, not the folder.

## The loop

1. **Measure.** `node finer/tools/fleet.mjs --measure --committed` for the standings and
   `node finer/tools/shipped-vs-compiled.mjs` for the gate that has caught four losses. Update
   `status.md`.
2. **Pick.** The largest loss that is *attributable* — clean source, like-for-like baseline — or the
   highest-ranked lead in `status.md`. Check `log.md`: a settled verdict is not re-run without a
   new fact.
3. **Attribute** in the order objective.md §6 gives. Most losses so far ended at step 1 or 2.
4. **Claim.** `node finer/tools/new.mjs <slug> --lane <lane>` opens the folder from the template.
   Write the claim, the number that confirms it and the number that falsifies it, the files to
   read, the files that may change, the exact commands. That top half *is* the brief a
   clean-context agent receives; if it cannot be written, the hypothesis is not ready.
5. **Test one variable.** Same binary, frozen source and config, pinned codec, deterministic
   counters (objective.md §8). Data larger than a table goes under `out/` or the folder, not into
   the README.
6. **Verdict.** Confirmed, falsified or split, with the numbers and the commit that landed or the
   reason for the revert. `env -u FORCE_COLOR cargo test --release` and the affected ports' own
   tests pass before "landed" is written. A compiler change is re-measured across the fleet: a
   local win that costs the portfolio is recorded as a loss.
7. **Record.** Status line in the folder, row in `log.md`, standings and leads in `status.md`, a
   settled fact if one was established. The folder is committed with the change it measured.
8. **Harvest**, between hypotheses. Read one competitor's source for one technique class, update
   `refs/competitor-techniques.md`, open a folder for any ABSENT technique worth its bytes. Also
   standing: the objective-purity check of objective.md §2 on every port that ships more than one
   objective.

## Lanes

| lane | optimizes | done means |
|---|---|---|
| `lang` | the language: a construct that hands the compiler a reusable proof | in `docs/language-v0.1.md`, paired cases green, measured on the port that needed it |
| `port` | a `*Lil` library: its `.lil`, its config, its build | tests green, shipped equals compiled, committed clean, smaller under its declared objective |
| `compiler` | search, admission, folds, naming, emission, budgets | suite green, byte-identical or smaller on every port under its own objective, compile cost reported |
| `measure` | harness, baselines, scoreboards, method | the number it corrects is corrected everywhere it was quoted |

## Roles

**Orchestrator** — the long-lived context. Reads objective, status, log. Never reads a folder
wholesale to summarize it: the folder's Status line *is* its summary. Spawns one clean-context agent
per hypothesis with a three-line prompt — read `finer/objective.md`, `finer/status.md` and
`finer/hypotheses/NNN-slug/README.md`; fill in Result, Verdict and Next; return at most twenty
lines — and works from the twenty lines, not the folder.

**Hypothesis agent** — a clean context. Reads the three files above and only what the folder's
Read section names. Writes its folder. Touches only what the folder's May-touch section names.
Does not edit `log.md`, `status.md` or another folder. Returns the Status line, the numbers, the
single next step.

**Harvest agent** — a clean context holding `refs/competitor-techniques.md` and one competitor
source directory. Returns a diff to the inventory and at most three candidate claims, each one line
with an estimated byte value.

## Hypothesis folders

- Numbered in order of opening; a number is never reused. The slug names the finding, not the
  task: `030-the-build-undoes-the-compiler`, never `030-micromark-investigation`.
- The **Status line comes first** and carries the verdict and its number. Everything a reader needs
  to decide whether to open the folder is on that line.
- Under about 10 KB. A folder that needs more is two hypotheses, or its data belongs in `out/`.
- Falsified hypotheses stay. A correction is an edit to the folder plus a new row in `log.md`,
  never a deletion.
- A number is quoted with its objective, its instrument (`fleet`, `sweep`, `bench`,
  `lilscript-codec`) and the commit or dist state it was measured on.

## Tools

| Command | Does |
|---|---|
| `node finer/tools/fleet.mjs` | Builds and measures every port in parallel, each pinned to a core slice; writes `out/fleet/scoreboard.md` |
| `node finer/tools/fleet.mjs --measure --committed` | Measures HEAD's artifacts, no builds: the standings |
| `node finer/tools/fleet.mjs --ports a,b --slots 2` | A subset |
| `node finer/tools/sweep.mjs --ports <p> --variants base,l13,...` | Per-port config search; every run carries its own `base` control |
| `finer/tools/bench.sh <port-dir> <entry.lil> <config> <levels...>` | Level sweep: CPU time, peak RSS, sizes, work counters |
| `node finer/tools/shipped-vs-compiled.mjs` | Fails if any port ships a less compact artifact than the compiler wrote |
| `node finer/tools/repeat-coverage.mjs a.js b.js` | Share of bytes inside long back-references — a diagnostic, not a predictor (025) |
| `node finer/tools/new.mjs <slug> --lane <lane>` | Opens the next hypothesis folder from the template and prints its `log.md` row |
| `node finer/tools/new.mjs check` | Every folder has a Status line and a `log.md` row; every row has a folder |

Compiler-side instruments: `LILSCRIPT_TIMING=1` (effort buckets and work counters, one JSON line on
stderr), `LILSCRIPT_NO_MEMO=1` (A/B the memos inside one binary), `LILSCRIPT_DUMP_CANDIDATES=<dir>`
(every distinctly scored artifact), `LILSCRIPT_VALIDATE_FOLDS=1`, and `--explain json` (families
scored and starved, budget stop reason).

## This host

- Shared with unrelated CPU-heavy processes; identical work has varied 3x in wall clock. Hence
  counters and CPU time, never wall clock (objective.md §8).
- `FORCE_COLOR` in the environment makes `node` colorize `console.log` and fails every stdout
  comparison: run the suite with `env -u FORCE_COLOR cargo test --release`.
- Sibling ports live beside this checkout as `../<name>lil`; the fleet discovers them there.
