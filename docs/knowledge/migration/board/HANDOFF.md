# Cloud handoff

Updated: 2026-08-29. Parent: [board](README.md). Active task:
[gate-04](notes/gate-04.md). Canonical plan:
[compression migration](../compression-migration.md).

This file persists the live execution queue for a fresh VM. The ledger owns
status and task notes own detailed evidence; this file is only the cross-repo
resume packet.

## Read first

1. [`docs/knowledge/mission.md`](../../mission.md)
2. [`board/README.md`](README.md)
3. [`board/LEDGER.md`](LEDGER.md)
4. [`notes/gate-04.md`](notes/gate-04.md)
5. [`compression-migration.md`](../compression-migration.md)

## Pinned repositories

| Repository | Branch/commit | Purpose |
|---|---|---|
| LilScript | isolated candidate `8fd78b4` atop pushed `main` | Green targeted gates, syntax/external/property provenance, and partial V-01 |
| MotionLil | pushed `main` at `1102ba7` | Source-level evidence entries for exact direct compiler boundaries |
| MarkedLil | pushed `main` at `9911cfd` | Cross-platform synchronized lockfile |
| MobXLil | `main` at `960f2fb` | Split source, true production-min config, and synchronized lockfile |
| Closure Compiler audit | commit `73eee2481cf1dd5dea0d8c9c0561b5a61498fec4` | Source comparison only; clone outside the repository |

The earlier checkpoints were pushed without force. Candidate `8fd78b4` is pinned;
the full G2 rerun was explicitly deferred in favor of targeted development gates.

## Completed

- Full Closure compression and mangling dossier in [`differences/`](../../../../differences/index.md).
- Canonical subordinate migration plan in
  [`compression-migration.md`](../compression-migration.md).
- Three independent fresh-context plan reviews, recorded as `plan-01..03`.
- V-02 implementation: total binding resolution for terminal renames, fixed
  descendant reservation, finite name exhaustion, and conservative template
  refusal.
- V-03 implementation: fresh ordinary-object assignment collection requires the
  explicit pristine-builtins contract.
- 524/524 generated-JS peephole tests, 54/54 canonical cases, and 10/10 codec
  contract tests passed.
- Isolated behavior preflights passed: Motion 9/9, Marked 29/29, MobX 769 with 11
  intentional skips plus package smoke, jQuery 6/6, and Solid JFB on rerun.
- Large-library matrix includes direct Motion output and true MobX
  production-min artifact lanes.
- `--check-inputs` validates all 15 source/config boundaries.
- Full release Rust is green: 1,603 library tests and all binary targets.
- The matrix has 15 immutable boundaries. Seven source-level Motion cells cover
  `animateMini`, `animate`, `animate+stagger`, lab, export, mini, and full without
  esbuild or Terser.
- The current candidate passes fresh semantics in every selected cell. Thirteen
  eligible Brotli comparisons tie the migration incumbent exactly.
- Marked raw/gzip now pass all 660 corpus cases after rejecting local-phi
  recovery that read a coalesced slot before its incoming definition.
- Gate-02, V-02 (`ident-09`), and V-03 (`emit-08`) are landed.

## Known red or incomplete gates

- V-01 final-artifact admission is not implemented. Current semantic harnesses
  now require syntax and binding admission before compiler-internal codec calls;
  final bytes also witness export names/counts, static foreign module edges, and
  live source-`|0` counts. Exported callable kind, arity, constructibility, and
  inherited method signatures are witnessed. Final contractions cannot introduce
  a static property absent from direct typed emission. Typed emissions and
  prepared leaves are admitted before scoring; the witness is retained through
  terminal cleanup and final selection. Configured optional-chain, nullish,
  logical-assignment, optional-catch, selected built-in, and class-field syntax
  floors are checked, including async and object rest/spread. Owner/slot
  provenance is recorded, final property byte ranges resolve against it, and newly
  introduced free globals/templates/properties are rejected. Bundle chunks are
  syntax/binding-admitted before measurement. Serialized witness reporting remains.
- Historical Marked raw/gzip outputs from `06b89aa` fail 229 corpus checks and
  are ineligible incumbents; candidate `7128462` fixes them. The Brotli incumbent
  and all other selected rows remain byte-identical.
- Historical `baseline`/`checkpoint` rows remain frozen; current work uses the
  explicit `migration,candidate` pair.

## Live queue

1. Clone or pull LilScript, MotionLil, MarkedLil, MobXLil, jQueryLil, and SolidLil
   beside one another; verify the pinned objects with `--check-inputs`.
2. Read `notes/gate-04.md`; syntax/binding admission is centralized at codec
   call sites and has zero-call rejection tests.
3. Complete V-01 with owner-qualified property witnesses.
4. Run targeted rejection fixtures, full Rust/canonical/codec gates, then the
   selected 13-boundary `migration,candidate` G2 command recorded in gate-02.
5. Continue canonical phase 1 incumbent recovery only after V-01 lands.

## Refusals

- No stale `dist`, dirty sibling source, post-minified compiler claim, package
  matcher, objective-dependent ABI, timeout increase used as a pass, or aggregate
  win used to hide one red boundary.
- Do not mark the migration complete from diagnostic preflights.
- Do not start new compression candidates while V-01 evidence/ABI admission is
  open.
