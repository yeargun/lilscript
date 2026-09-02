# LilScript objective

The contract every piece of work in [`finer/`](README.md) is held to. It changes only when the
owner's intent changes ([intent/](intent/)). Where things stand is [status.md](status.md); what has
been tried is [log.md](log.md). Hand this file, not the history, to a fresh context.

## 1. Mission

For every maintained `*Lil` port, the compiler must produce a JavaScript artifact **smaller than the
best JavaScript toolchain produces from the upstream library**, under the objective the port
declares, at the default effort level, at a compile time a developer tolerates, with behavior and
public API intact. Beat, never merely tie: a tie is the floor, a loss is a bug with an owner. Three
things are optimized toward that — the **language**, the **ports**, the **compiler** (§5) — and none
is presumed innocent when a port loses.

## 2. The objective is a config value

`javascript.cost_model` — `raw` | `gzip` | `brotli` — is the one metric an invocation is judged by,
through pinned encoders (zlib 1.3.1 level 9; Google Brotli 1.1.0 quality 11, window 22). Nothing
else scores, and no number from another encoder is quoted.

- **Brotli is the present focus.** Every port declares it and the fleet is judged under it.
- **Raw and gzip are objectives, not diagnostics.** Each is its own compile and may legitimately
  choose a different program. A change is judged under the objective of the artifact it touched;
  its movement under the other two is reported, never counted for or against.
- **Objective purity is a standing check.** Built three ways, the raw build wins raw, the gzip
  build wins gzip, the Brotli build wins Brotli. An off-diagonal winner means one objective's search
  or cost model is wrong, and fixing that outranks any size lead.

## 3. What bytes cost

Bytes are bought with CPU first and runtime shape second. `javascript.optimization_level` 0..15 is
the effort ladder.

- **13 is the default and must remain the sweet spot**: 1.4% of level 15's bytes on the reference
  port for a twentieth of its CPU. Every memo, budget split, screening pass and early-out that
  holds 13 near 15 is in scope. A lever unaffordable at 13 is a level-15 option, not a default.
- **14–15 buy the last bytes with wall clock.** A port pins them only on its own measured curve;
  curves do not transfer between artifacts.
- **No knob may silently disable a stage.** A configured `0` means "none of this work", never "skip
  the rewrite this budget guards", and `--explain` distinguishes a configured zero from an exhausted
  budget.
- **Runtime performance is a constraint, not the current objective.** Generated code stays close to
  idiomatic hand-written JavaScript under `[javascript.performance] max_regression_percent`. The
  performance-first priorities must keep working; the work now is compression.

## 4. Whom we beat, in which world

Baselines are the strongest eligible pinned toolchains on the same boundary — Terser, Oxc, esbuild,
SWC — and Closure ADVANCED is the ceiling to exceed, not to match. Its whole-program moves are table
stakes: collapse namespaces and nested access into flat names, split object literals into scalars,
devirtualize methods, extract prototype prefixes, rename every private property, remove everything
unreachable. All of it we do; then what only a typed source permits — positional slots for owned
aggregates, dissolving non-escaping objects into SSA values, exact boundary ABI with private
specialization behind it.

| world | config | held fixed | eligible baseline |
|---|---|---|---|
| **open** — reusable library, the default | `mangle.exports = false` | public ESM names, public aggregate fields, and `Function.name` / arity / constructibility where observable: the programmatic API and its DX are unchanged | Terser / Oxc / esbuild / SWC, identifier mangling on, property mangling off |
| **closed** — an app, or a library shipped as one | `mangle.exports = true`, static imports linked | behavior only | the same, plus Closure ADVANCED and property-mangled lanes |

We must win in both. Open world is the fair fight the fleet scores today; closed world is where
types, ownership and effects give room no minifier has, so a loss there is inexcusable.

## 5. Three levers, and what each may do

- **Language.** A construct is added when a measured loss needs a shape LilScript cannot spell — a
  hook-free plain-data object (013), a perf-critical JavaScript idiom — and it must hand the compiler
  a reusable proof, not syntax. Never a package matcher, never a Terser-shaped source workaround.
- **Ports.** A port is idiomatic LilScript, not JavaScript transliterated through `JsValue` bags
  (019, 021); its config is part of the port; its build ships what the compiler wrote. A port is
  measured from a committed, clean tree — a size against a mid-migration source is not a compiler
  measurement.
- **Compiler.** Proposes and proves; the codec decides (§7).

## 6. Where a loss comes from, in the order to look

Stop at the first step that explains the number.

1. **Shipped ≠ compiled.** Build scripts and bundlers have re-printed, un-minified and escaped the
   compiler's output four times (006, 028, 030, 034). `tools/shipped-vs-compiled.mjs` checks the
   fleet for it.
2. **Comparison ≠ like-for-like.** Same graph, same bundling, same minification, same codec, both
   sides; committed artifacts (014, 028).
3. **Source shape.** Diff the `.lil` against the cloned upstream: an idiom upstream spells once and
   the port spells eight times is the port's loss. If the needed shape cannot be spelled, it is the
   language's (§5).
4. **Compiler.** Search, admission, folds, naming, emission — sized by what Terser can still extract
   from our own finished artifact (035).

Correlations are measured, not assumed: "too `JsValue`-typed" scored −0.09 against the losses,
emitted volume +0.92 (025).

## 7. Compiler doctrine

- **Representation is measured, never preferred.** Closure's class-to-table and our table-to-class
  are both candidates; the compressor votes through `cost_model`. A family the search never
  proposes, a candidate a validator wrongly refuses, and a family that starves in the budget are
  equally invisible to it (031, 032, 036); their reachability comes before any new family.
- **Portfolio over artifact.** A change is scored across the fleet under each port's own objective.
  A local win that moves the fleet the wrong way is a loss (031: −194 local, +826 fleet).
- **Budget is allocated, not added.** Level 13 buys bytes by spending probes where the hit rate is
  (009), not by raising ceilings.
- **Harvest is a precondition, not a phase** (§10). Terser, Oxc, esbuild, SWC and Closure are read,
  not remembered. Every technique is PRESENT / PARTIAL / ABSENT with file:line evidence in
  [refs/competitor-techniques.md](refs/competitor-techniques.md), and an ABSENT one enters as a
  semantics-preserving candidate the codec can vote on, never as a local cost rule.
- **The target is the global optimum.** The search exists to find the smallest artifact the
  objective admits, not the nearest local minimum. A family never proposed, a region the beam prunes
  at its first plateau, a budget that stops when the incumbent survives one round: each is a search
  that stopped early, and each is measured as such. The effort ladder (§3) bounds the spend per
  level; it never lowers the target, and what 13 cannot afford is 15's job, not nobody's.
- **Legality first**, as [mission.md](../docs/knowledge/mission.md) refuses: no unsafe getter, proxy
  or pristine-host assumption on by default; no post-minifier; no library-shaped fold; no
  objective-dependent public API; no semantic gate weakened to keep a byte.

## 8. Evidence

- **Sizes** from `lilscript-codec` only; Node's encoders disagreed on 96 of 279 artifacts.
- **Cost** as deterministic work counters (`LILSCRIPT_TIMING=1`), then CPU time as the minimum of an
  interleaved A/B; wall clock is never a result on a shared host.
- **One variable per experiment**, in one binary (`LILSCRIPT_NO_MEMO`, config flips, source
  frozen), output confirmed deterministic across thread counts.
- **Gates before wins**: the port's own tests and the shipped-vs-compiled check pass first.
- **Every hypothesis is a numbered folder**, falsified ones included; a negative result is what
  stops the next context paying for the idea twice.

## 9. Compute

Compiles are the loop's clock, and the loop runs on a pool of Azure machines, not on one host. Fleet
builds, level curves, config sweeps and A/B measurements are dispatched across the pool, one port
or one variant per machine, and a tool that serializes them on the orchestrator's host is the
defect, not the workload. A hypothesis is scoped so its builds can run side by side. The rules of §8
hold on every machine: the same commit, the pinned codec, counters over clocks. Wall clock is still
never a result; the wall clock of the *loop* is a cost the owner pays, and it is bought down with
machines before patience.

## 10. Understanding before work

The bar is Terser today and Closure ADVANCED at the end, and both are open source: a hypothesis or a
migration step designed without reading how they handle the same class of shape is guessing with a
codec. So the reading comes first, and it is written down.

- **Before a hypothesis folder is opened**, the technique class it touches is read in the competitors'
  source — Closure first, since it is the ceiling; then Terser, Oxc, esbuild, SWC — and the folder's
  *Prior art* section says, with file:line, what each does, what each refuses and why, and what that
  implies for the claim's confirming and falsifying numbers. A brief that cannot say what Closure
  does for its class is not ready, and `tools/new.mjs check` refuses the folder.
- **Before a migration step in a port**, the upstream library's own source for that module and the
  way the baseline toolchain shapes it are read, so the `.lil` is written to the shape the compiler
  can prove, not transliterated (§5).
- **Before a measurement is trusted**, the instrument's own assumptions are read: the harness, the
  encoder, the build script between the compiler and the artifact (§6, step 1).
- The reading accrues in [refs/competitor-techniques.md](refs/competitor-techniques.md), one row per
  technique with its status against us, so no context pays for the same file twice. A closed folder
  leaves the inventory richer than it found it.
