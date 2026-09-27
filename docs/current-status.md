# Current status

**Updated 2026-09-27.** Version 1 of the design is in force:
[future-architecture.md](future-architecture.md) is the design of the language
and the one compiler, [migration/index.md](migration/index.md) is the only plan
and records what landed, and [testing.md](testing.md) says how a binary is
checked. M1 closed on 2026-09-24. On 2026-09-27 M3.3a and batch A1 (M8.2)
landed, and the first tools of the plan's step 3 (M2.13, M2.5); the rest
of step 3 (M2.10–M2.12, M2.14, M2.15) is next. The
status page of 2026-09-24 is in git history; the one of 2026-09-20, from before
M1, is in [history](knowledge/history/status-2026-09-20.md).

## One compiler

There is one compiler: parse → check → the typed Program IR → JavaScript
formation, target rewrites, naming and exact codec scoring, or the native C
writer ([current architecture](knowledge/compilation/current-architecture.md)).
The route that shared the binary with it until 2026-09-23 is deleted; a frozen
reference binary (`~/lilscript-work/bin/reference-2026-09-23/lilscript
--backend legacy`) measures it as "the first bar". `--backend` fails with "there
is one compiler", and `[compiler] backend` is refused.

## Landed since M1

The plan's "Where we are" has each record with its evidence. In short:
- **M4.1** nominal identity: per-module class and enum scopes, one checker
  entry, `FieldRef` places, `export constructor`.
- **M5.4** the terminal challenger stage; **M9.1**'s first slice (choice keys
  and maps), **M9.2** (the objective as a judge), **M9.3**'s first spelling
  family and **M9.8** data tables (katexlil −2,423 Brotli).
- **M6.1–M6.3 and M6.5** effects, the call graph, the `pure` check and
  initialization order: computed on the program, **not yet consumed** by the
  tree (rule 2); **M7.2** discards effect-free calls.
- **M2.1** green CI on one Linux job; **M2.2** and **M2.6** the case and port
  runners with owned ledgers; **M2.8/M2.9** the interim release of the 13 goal
  ports on the post-M1 binary (katexlil re-released 2026-09-27 at 60,281 Brotli).
- **M3.3a** the delivery contract's first slice: one program with several
  roots, root rows, placement with its plan verifier, plans rendered, scored
  and delivered, manifest v3; `-j` is the one parallelism flag and
  `--codec-jobs` warns that it is removed.
- **Batch A1** canonical formation without tree analysis: a method is its own
  function, observed classes print with their bodies, no operand spills, one
  decoder per schema, imports by identity, `print` is never stripped
  (`strip_debug` and `strip_console_calls`), naming's seed at every level, and
  native arithmetic without per-operation barriers.
- **M2.13** the generic corpus ratchet, the NO3 library-name test and the NO4
  idiom debt ledger ([testing.md](testing.md#the-generic-corpus-ratchet));
  **M2.5** the admission parse ([testing.md](testing.md#the-admission-parse-a5)).

## Standings

The generic corpus against its bars (Brotli, `head-d1d48c4c`, 2026-09-27; the
ratchet's baseline, which no change may worsen):

| Corpus | Ours | Smallest competitor | Closure ADVANCED | Old route |
|---|---:|---:|---:|---:|
| `comparison/apps` (7) | 945 | 870 (5 lose) | 834 (7 lose) | 558 (7 lose) |
| `comparison/algorithms` (11) | 3,250 | 3,039 (9 lose) | 2,703 (11 lose) | 2,305 (11 lose) |
| `comparison/cases` (612 of 624 compile) | 49,223 | 43,541 (439 lose) | — | 603 of 611 lose |

The program rule layer is empty in production: only liveness (with M7.2) runs
on the Program IR, so constant branches, small helpers and dead functions
survive that every competitor removes (architecture §1.1). The floor slice (plan
step 5) is gated per change on this ratchet.

On the ports, the closure ledger (architecture §18.4) lists every losing
development cell with its plan items and projected residual; M2.10 re-verdicts
it under the pinned benchmark contract (SWC and the Rolldown bundler included).
motionlil's delivered files are still esbuild plus Terser over the compiler's
output (M12.2 with M3.3). No result counts unless the delivered file is
compiler-written, with no post-minifier (plan rule 6).

## Gates that block

Each is a ratchet over a ledger whose entries name an owner task (law L21):
- the case runner, `tests/cases/expected-failures.json`;
- the port runner, `tests/ports/expected-failures.json`;
- the generic corpus ratchet, `tests/ratchet/` (bars, baseline, ledger);
- NO3, `tests/no3-allowlist.json`: 31 mentions of ports and upstream libraries
  in non-test `src/`, which may only shrink (M8.7 empties it);
- the admission parse, inside the compiler: every delivered file is parsed
  again by Oxc and, printed whole, must parse to the printed tree's structure.

## Next

The plan's "Next action" is authoritative. In order: the rest of step 3's tools
(the benchmark contract M2.10, the held-out set M2.11, the perf runner M2.12,
the frozen compile-time baselines M2.14, the calibration corpus M2.15), the
floor slice of exact program rules, the budget model (M3.5), then language
slice 1.
