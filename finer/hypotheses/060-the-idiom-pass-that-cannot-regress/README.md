# 060 — The idiom pass that cannot regress

**Status: CONFIRMED. Two wins, two ties, no regression.** 059 offered the idiom conversion inside the
cleanup beam and katexlil ended +82 with every individual comparison won. Moved to the terminal
position and applied one idiom at a time under the exact codec, the same conversion is
**markedlil −48, jquerylil −40, katexlil ±0, motionlil ±0** — and the ±0s are byte-identical
artifacts, not near misses.
Lane: compiler. Objective: brotli. Ports: markedlil, jquerylil, katexlil, motionlil. Opened: 2026-09-03.

## Prior art

Terser and Oxc rank names per scope by use count; Closure's `RenameVars` optimises length and reuse.
None has a notion of agreement between two scopes, so a short run the program repeats in twenty
places gets twenty spellings from all three. 056 established why frequency ranking is nonetheless
close to right — it is a first-order entropy optimisation Brotli's literal coder collects — and 059
established that converging on the *commonest* spelling is the one form of convergence that does not
fight it.

Our own prior art is the reason this folder exists. `apply_terminal_binding_coordinate_descent`
already sits on the exact winner rather than in the beam, with the comment that names the hazard:
*the remapping that follows the cleanup is not monotone in the cost the cleanup ranks by.*

## Claim

059's conversion is sound and its placement was not. Offered in the beam, a candidate that wins its
own comparison changes which basin the run continues from, so per-step monotonicity does not give
end-to-end monotonicity — measured, katexlil +82. Offered on the final artifact, after every other
stage, nothing downstream can undo it, so the result is either strictly smaller by the codec's own
measure or byte-identical.

**Confirms** no port worse, at least one better. **Falsifies** any port worse.

## Method

Two changes, both load-bearing.

**Placement.** `apply_terminal_idiom_convergence` runs on the winner returned by the terminal
finalists, after `apply_terminal_binding_coordinate_descent`. Nothing reads the artifact afterwards.

**Granularity.** 059 applied the whole assignment at once and measured the sum, which was negative on
every port because most conversions hurt. `idiom_conversion_groups` now returns one preference map
per idiom, ranked by the novel text it would remove, and the pass hill-climbs: offer one, let the
exact codec rule, re-derive the groups against whatever was accepted, repeat. Only conversions that
pay for their own displacement survive.

The conversion itself is unchanged from 059 and shares `converge_names` with the canonical pass, so
it inherits every legality proof that path relies on.

Measured with the **Oxc terminal parser gate** cherry-picked from `4e799a8` (Oxc dependencies only,
not the source-map machinery). 047 recorded that a fold emitting a wrong program ships on main and is
refused with the gate; a pass that rewrites every occurrence of a binding should not be measured
without it.

```sh
node finer/tools/workers.mjs build --ports katexlil,markedlil,motionlil,jquerylil \
  --dist-dir finer/out/060/off
LILSCRIPT_IDIOM_NAMING=1 node finer/tools/workers.mjs build --ports ... --dist-dir finer/out/060/on
```

## Result

One binary, the knob the only variable, built on the pool:

| port | brotli off → on | raw | gzip | brotli | |
|---|---|---:|---:|---:|---|
| markedlil | 9470 → **9422** | +0 | −41 | **−48** | win |
| jquerylil | 28436 → **28396** | +0 | −16 | **−40** | win |
| katexlil | 64907 → 64907 | +0 | +0 | **±0** | byte-identical |
| motionlil | 50550 → 50550 | +0 | +0 | **±0** | byte-identical |

Raw is unchanged everywhere: this is a pure respelling. The two ties are the guarantee doing its job
— conversions were offered, the codec refused all of them, and the incumbent shipped untouched.

**These deltas are not basin noise.** Each accepted conversion was measured by the exact codec to
reduce the whole artifact, so the totals are the sum of verified reductions rather than the residue
of a search that happened to land elsewhere. That is the difference between this and 059's markedlil
−53, which came from a trajectory change and did not survive to katexlil.

The acceptance rate is low and honest about it: markedlil offers 26 conversions and accepts 1;
raising the reserve to 200 probes offers 76 and still accepts 1, for −18 rather than −11 on that
port's own level-15 build. The pass is cheap per probe and rarely right, which is exactly why it has
to be scored rather than applied.

### Two defects found building it

- **The pass was starved.** It runs last, so the terminal ledger was spent before it was reached:
  markedlil had **821 idiom groups** available and `idiom_candidates` read **zero**. It needs a slice
  reserved at the ledger the terminal stage actually uses — not the outer one, which is a different
  instance. Reserved only when the knob is on.
- **A disabled pass must not touch the budget.** Releasing the final reserve is observable: it
  changes what later work on that ledger may spend. Doing it before the enabled check moved
  `higher_effort_retains_the_lower_effort_two_binding_brotli_winner` from 52 to 55. The guard now
  returns before any ledger effect.

## Gates

- 1702 compiler tests green, with the Oxc terminal gate active.
- Built with the knob on and run against each port's own suite: **markedlil 29/29**,
  **jquerylil 6/6**, **katexlil 21 suites / 1230 tests**.
- Knob off, the ledger and the output are what they were.

## What landed

`javascript.idiom_directed_naming`, still **default false**. The evidence supports flipping it — a
structural guarantee plus two wins and two ties — but four ports is not the fleet, and the doctrine
is that a default flips on a fleet measure. `LILSCRIPT_IDIOM_NAMING=1` runs it across the pool in one
command, and `workers.mjs` forwards that kind of switch to every worker.

## Next

Flip the default only after the full fleet, and watch the cost as well as the bytes: the reserve is
`terminal_codec_probe_limit / 8`, taken from families that would otherwise spend it. The bytes here
are small enough that the trade against level 13's CPU curve (objective §3) is the real question, not
whether the sign is right.
