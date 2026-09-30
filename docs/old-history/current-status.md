> Archived on 2026-09-29. Historical design and evidence; use the [active migration](../migration/index.md) for current work.

# Current status

**Updated 2026-09-27.** Version 1 of the design is in force:
[future-architecture.md](future-architecture.md) is the design of the language
and the one compiler, [migration/plan.md](migration/plan.md) is the only plan,
and [testing.md](../testing.md) says how a binary is checked. This page describes
the checkout: the compiler, its standings and its gates. Which migration tasks
have landed and what is next is kept in one place only,
[migration/index.md](migration/index.md). The status page of 2026-09-24 is in
git history; the one of 2026-09-20, from before M1, is in
[history](../knowledge/history/status-2026-09-20.md).

## One compiler

There is one compiler: parse → check → the typed Program IR → JavaScript
formation, target rewrites, naming and exact codec scoring, or the native C
writer ([current architecture](knowledge/compilation/current-architecture.md)).
The route that shared the binary with it until 2026-09-23 is deleted; a frozen
reference binary (`~/lilscript-work/bin/reference-2026-09-23/lilscript
--backend legacy`) measures it as "the first bar". `--backend` fails with "there
is one compiler", and `[compiler] backend` is refused.

## Migration

[migration/index.md](migration/index.md) lists every task as done or open, the
next step and the open items each batch left. What each batch landed, with its
evidence, is in [migration/history.md](migration/history.md). The tools in use
are described in [testing.md](../testing.md): the generic corpus ratchet, the NO3
library-name test and the NO4 idiom debt ledger
([ratchet](../testing.md#the-generic-corpus-ratchet)), and the admission parse
([A5](../testing.md#the-admission-parse-a5)).

## Standings

The generic corpus against its bars (Brotli, `head-a1`, 2026-09-27; the
ratchet's baseline, which no change may worsen):

| Corpus | Ours | Smallest competitor | Closure ADVANCED | Old route |
|---|---:|---:|---:|---:|
| `comparison/apps` (7) | 945 | 870 (5 lose) | 834 (7 lose) | 558 (7 lose) |
| `comparison/algorithms` (11) | 3,250 | 3,039 (9 lose) | 2,703 (11 lose) | 2,305 (11 lose) |
| `comparison/cases` (612 of 624 compile) | 49,170 | 43,541 (439 lose) | — | 603 of 611 lose |

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

See "Where we are" in [migration/index.md](migration/index.md). The order of the
remaining work is the plan's [order of work](migration/plan.md#order-of-work).
