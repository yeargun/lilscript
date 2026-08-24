# Migration plan: name allocation as a colouring objective

Parent: [index](README.md). Board protocol:
[migration board](../../migration/board/README.md). Measurement meaning:
[verification](../../verification/README.md).

This plan does not change the compiler; it says what to change, in what order,
and what evidence closes each step. Rows for the ledger are at the bottom,
ready to paste when the orchestrator opens the lane.

## Phase A0 — one cost model miscompiles

**This goes first, ahead of every size question.** markedlil compiles one
source tree four times, changing a single knob each time. Scored with
`lilscript-codec` and run through all 680 CommonMark 0.31.2 and GFM 0.29 spec
cases:

| Build | knob | raw | gzip-9 | Brotli-11 | spec failures |
|---|---|---:|---:|---:|---:|
| `marked.bytes.js` | `cost_model = "raw"` | 33,548 | 10,554 | 9,456 | **208** |
| `marked.closed.js` | `cost_model = "brotli"`, `extern_fields = false` | 35,621 | 10,678 | 9,475 | 206 |
| `marked.raw.js` | `cost_model = "brotli"` | 35,901 | 10,711 | 9,543 | 206 |
| `marked.gzip.js` | `cost_model = "gzip"` | 36,220 | 10,674 | 9,543 | 206 |
| `marked.esm.js` | what the package publishes | 35,985 | 10,766 | 9,589 | 206 |

206 is this port's normal state. Four of the five builds fail exactly those 206
and produce byte-identical output on all 680 cases. The fifth does not:
`cost_model = "raw"` drops the `mailto:` prefix from email autolinks
(CommonMark #604 and #605).

**A search ranked and shipped a candidate that changes what the program
computes** — and the mechanism, once the two builds are read side by side, is
the board's own `ident` invariant: a read of a regex match group was sunk past
the reassignment of the variable holding the match
([07](07-ports.md)). The raw cost model does not cause the bug; it buys enough
statement fusion for the fold to fire. That makes this a live reproduction of
the `ident` class in a shipped port, with a one-knob trigger and a two-case
spec signature.

| Step | What | Exit signal |
|---|---|---|
| A0-1 | ~~Reproduce with today's compiler~~ **done: it reproduces.** A fresh build under `lilscript.bytes.toml` at this HEAD fails the same two cases (208 vs 206); the matching Brotli-model build passes. ~45–50 min wall each. | ✅ reproduced |
| A0-2 | ~~Is it the scorer or the beam?~~ **done: it is neither.** The two builds differ in four transform families; applied one at a time to the Brotli artifact, two of them genuinely cost Brotli bytes (`for`→`while` +19, naive outlining +126) and one is a tie that is worth 920 raw bytes. The size story is a tie-break, not a broken model. | ✅ answered |
| A0-3 | **The bug is `ident`, not cost.** The raw model's statement fusion let a read of match-group 2 sink past the reassignment of the variable it reads (`p = token` before `a(p,2)`). Route the fold through the shared receiver-rebinding check that `ident-02` is creating, and freeze CommonMark #604/#605 as a canonical case. | The two cases pass under every cost model, and the fold has a test |
| A0-4 | **Break codec ties by raw size.** On markedlil the compiler is choosing the 920-raw-byte-larger side of a ±3-byte Brotli tie. One concrete lead: `ChunkCostConfig::default()` in `src/config.rs:1623` sets `raw_weight: 0` (gzip 1, brotli 2), so raw size is worth nothing in `artifact_deploy_cost`. That function is the chunk-level ranker and marked is a single artifact, so the first question is which comparator actually decided this — and whether it, too, ignores raw. | The comparator is named; merging adjacent declarations is taken on markedlil; no corpus regresses |
| A0-5 | ~~Is markedlil's missing `compression = [...]` list the cause?~~ **done: no.** Rebuilding markedlil with jquerylil's 30-pass list under `cost_model = "brotli"` gives 36,062 raw / 9,504 Brotli against the default config's 36,018 / 9,509 — a wash — and it still leaves **230 adjacent declarations unmerged**, exactly as before. The raw-cost build leaves 1. So the merging decision follows the **cost model**, not the pass list. (It also cost ~110 CPU-minutes against ~52 without the list: the pass list roughly doubles search time for ±5 bytes.) | ✅ hypothesis refuted; A0-4 is the live one |

Everything below this line assumes the compiler computes the same program
under every cost model. While A0 stands, it does not.

Two smaller things fall out of the same table: markedlil publishes
`marked.esm.js` (9,589) when `marked.closed.js` (9,475) is byte-for-byte
equivalent and 114 Brotli bytes smaller; and no naming work is worth doing on
this port (−5 to +14).

## Why this lane exists

A legal, behaviour-verified rewrite of the public jQuery artifact is **801
Brotli bytes (2.41%) smaller** than what ships, at 542 bytes less raw, and the
only thing it changes is which name each binding holds
([05](05-concentration.md)). It spends 27 distinct spellings where the shipped
artifact spends 106. `lilscript-codec` confirms it: 33,283 → 32,482. That is
larger than every result in the
[global-mangle playbook](../brotli-global-mangle/README.md), and it is not a
new idea — it is the legal form of the collapse that folder could only probe
illegally.

The compiler already contains the regime that produces it
(`precise_cross_scope_shadowing`), off in the pinned path by design, reachable
only as a candidate-beam proposal. The lane is therefore mostly about
**reachability, proof and cost**, not about inventing an optimisation.

## Dependencies and honesty about them

- This lane touches naming, which is the same surface as the board's `ident`
  bugs. A colouring change that lands while `ident-05` is open will be blamed
  for, or will hide, identity failures. **Do not start Phase B before the
  `ident` lane is green.** Phase A is diagnosis only and is safe to run now.
- Candidate search is expensive on this artifact. A production build of the
  jQuery port with its own config (`candidate_search = "production"`,
  `candidate_byte_budget = 16 MiB`) was started as step A1 and **did not finish
  in 4.5 hours of wall time / 9 CPU-hours** on this machine before it was
  stopped, moving between multi-threaded and single-threaded phases throughout.
  Any plan that says "the beam will find it" has to account for that: A1 needs
  a recorded time budget, and A0 should use markedlil (a third of the size)
  rather than jQuery.
- Every size claim here is Node zlib Brotli 1.1.0 q11 `lgwin=22` unless it says
  `lilscript-codec`. The headline number was re-scored with the gate codec.

## Phase A — settle why the shipped artifact spells 106 names

Diagnosis only. No compiler change. Safe while `ident` is red.

| Step | What | Exit signal |
|---|---|---|
| A1 | Rebuild `ports/jquery/entry.lil` with today's compiler and its own config; record raw/gzip/brotli and the distinct-name count | The checked-in artifact is either reproduced or shown stale, with numbers in the note |
| A2 | Determine whether the beam proposed `precise_cross_scope_shadowing` for this artifact, and whether it reached the finalists | A recorded answer: not proposed, proposed and lost, or proposed and dropped by beam width |
| A3 | Score the precise regime directly by forcing it, and compare against this folder's re-mangle | The compiler's own precise emission is within noise of 32,482, or the difference is explained |

A2 is the interesting one. Three outcomes, three different plans:

- **not proposed** — the variant is gated behind a condition this artifact does
  not meet. Fix the gate; that is a small change with a large payoff.
- **proposed and lost** — the beam's scorer disagrees with `lilscript-codec`,
  which is a scorer bug and outranks this lane.
- **dropped by beam width** — the proposal is competing with families this
  folder has now measured as worthless (dictionary spellings, function layout).
  Rebalance the beam using [03](03-dictionary-as-names.md) and
  [06](06-free-order.md) before adding anything.

## Phase B — make the allocation objective explicit

Blocked by the `ident` lane. This is the actual compiler work.

Today the allocator is "first free name in alphabet order, with a reserved set
that only sometimes releases". The measurements say the objective should be
stated directly:

> Minimise the number of distinct spellings, then bias the ones you do use
> toward the letters the file already spends.

| Step | What | Gate |
|---|---|---|
| B1 | Build the interference graph the allocator implicitly has, and colour it with an explicit "fewest colours" objective rather than a monotonic counter with releases | Every corpus in `01-corpora` scores no worse; the two LilScript ports score better; `lilscript-codec` is the ranker |
| B2 | Keep `stable_local_names` as a scored alternative, not a default. Source-local affinity prevents two bindings from sharing a colour, which is the thing being optimised | A paired case where affinity costs bytes, and the search picks the merge |
| B3 | Re-sweep `local_name_reserve` under the new allocator; the port currently pins 8 | The sweep's winner is recorded, and 0 stays an ablation rather than a candidate |
| B4 | Freeze the invariant as canonical cases: two non-interfering live ranges in sibling closures must receive the same name | `node comparison/cases/run.mjs --only naming/` green |
| B5 | Whatever B1 produces must be measured on the three ports, not only on the in-tree corpora: jquerylil and solidlil's core are where the emit ships directly, and where the 2.2–2.5% actually is ([07](07-ports.md)) | Both ports improve under `lilscript-codec` with their batteries green |

The correctness rule for B1 is the one this folder's re-mangler already
implements and the differential already exercises: a name is unusable only when
it would collide with a sibling binding, capture a reference inside the scope's
subtree, or be shadowed between one of the binding's references and its own
declaration. Anything the analysis cannot prove stays unrenamable.

## Phase C — the free orders

Independent of B. Small, cheap, and each one is a scored proposal rather than a
rule.

| Step | What | Expected |
|---|---|---|
| C1 | Emit pooled literals in reversed-string order under a Brotli cost model, alphabetical under gzip | −50 to −70 br11 on the ports that have a pool ([06](06-free-order.md)) |
| C2 | Leave function layout alone | Recorded as closed: five orders, six corpora, one win of 0.5% and losses everywhere else |

## Phase D — write the closed doors down

These are `rejected` rows, and they are the most valuable part of the plan
because they stop the next context from re-deriving them.

| Idea | Verdict | Evidence |
|---|---|---|
| Identifiers spelled as dictionary words, hot | rejected | +1,953…+6,341 br11, seven corpora ([03](03-dictionary-as-names.md)) |
| Identifiers spelled as dictionary words, cold (≤3 uses) | rejected | +1,017…+2,437 br11, seven corpora |
| Naming aligned to maximise LZ copyability | rejected | +49…+673 br11; a bit-cost objective declines every move ([04](04-alignment.md)) |
| Function-declaration layout for the distance cache | rejected | Implicit-distance rate moves <1 point; br11 +13…+433 ([06](06-free-order.md)) |
| Rewriting arrows to `function` to reach the ROM | already rejected | +274 br11, [09 audits](../brotli-global-mangle/09-audits.md); the census explains it |

## Phase E — keep the instrument

`census.mjs` decodes a real stream and reports where its bits went. It cost
nothing to build once [brotli-machine](../brotli-machine.html) existed, and it
changes how proposals are argued.

Adopt one rule: **a proposal that claims a mechanism must show the census row
where the mechanism appears.** If a change claims to help the dictionary,
dictionary references must go up. If it claims to help the distance cache, the
implicit-distance rate must move. Two of the ideas in Phase D died on exactly
this test after looking plausible in prose.

## Ledger rows, ready to paste

```text
| `name-00` | todo | `cost_model = "raw"` miscompiles markedlil: email autolinks lose their `mailto:` prefix (CommonMark #604, #605), while the other three cost models produce byte-identical output on all 680 spec cases. A search ranked a behaviour-changing candidate. | The two cases reproduce and become a canonical regression; blocks the rest of the lane | [notes](notes/name-00.md) |
| `name-01` | todo | Diagnose why the shipped jQuery artifact spells 106 distinct local names where 27 suffice; a legal re-mangle is -801 br11 / -542 raw under lilscript-codec. Determine whether the beam proposes `precise_cross_scope_shadowing` for it. | A recorded answer plus a reproduced or refuted artifact | [notes](notes/name-01.md) |
| `name-02` | blocked(ident-05) | Make name allocation an explicit fewest-colours interference colouring instead of a counter with releases. | No corpus regresses under lilscript-codec; both LilScript ports improve | [notes](notes/name-02.md) |
| `name-03` | blocked(name-02) | Freeze the invariant: non-interfering live ranges in sibling closures share a name. | `node comparison/cases/run.mjs --only naming/` green | [notes](notes/name-03.md) |
| `name-04` | todo | Pooled-literal emission order as a scored proposal (reversed-string under Brotli). | -50 to -70 br11 on the ports with a pool, no regression elsewhere | [notes](notes/name-04.md) |
| `name-05` | rejected | Dictionary words as identifiers, hot or cold; copy-maximising aligned naming; function layout for the distance cache. | Kept as a rejection so it is not retried | [research](../../research/aligned-mangling/README.md) |
```

## Where the win is, port by port

Measured, behaviour-verified, and re-scored with the gate codec
([07](07-ports.md)):

| Port | Artifact | Available now | Verified by |
|---|---|---:|---|
| jquerylil | `dist/jquery.esm.js` | −776 br11 (−2.5%) | 28/28 jsdom observations |
| jquerylil | `dist/jquery.raw.js` | −770 br11 | 28/28 |
| solidlil | `reactive.generated.js` | −96 br11 (−2.2%) | 18/18 reactive observations |
| markedlil | `dist/marked.bytes.js` | −37 br11, plus −87 from the config | 680/680 spec cases |
| solidlil | any bundled app artifact | nothing; rolldown re-mangles | — |

The rule that falls out: **the naming work pays exactly where LilScript's emit
is the final artifact.** Where a bundler runs afterwards, it takes the win and
the effort should go to shape instead.

## What would change the plan

- If A1 reproduces the artifact at ~32.5 KB, the win is already in the compiler
  and only the checked-in artifact is stale. Phase B becomes a much smaller
  "make it the default" task.
- If A2 finds the scorer disagreeing with `lilscript-codec`, this lane stops
  and the scorer lane starts; nothing here is trustworthy until they agree.
- If B1's colouring finds nothing beyond A3's precise regime, that is a
  complete answer too: the objective was already implemented, and the work is
  reachability, not search.
