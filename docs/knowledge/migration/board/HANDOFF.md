# Cloud handoff

Updated: 2026-08-29. Parent: [board](README.md). Active task:
[gate-02](notes/gate-02.md). Canonical plan:
[compression migration](../compression-migration.md).

This file persists the live execution queue for a fresh VM. The ledger owns
status and task notes own detailed evidence; this file is only the cross-repo
resume packet.

## Read first

1. [`docs/knowledge/mission.md`](../../mission.md)
2. [`board/README.md`](README.md)
3. [`board/LEDGER.md`](LEDGER.md)
4. [`notes/gate-02.md`](notes/gate-02.md)
5. [`compression-migration.md`](../compression-migration.md)

## Pinned repositories

| Repository | Branch/commit | Purpose |
|---|---|---|
| LilScript | `compression-objective-lane` at `06b89aa` plus the pending matrix/handoff commit | Full architecture/evidence snapshot and V-02/V-03 safety implementation |
| MotionLil | `main` at `fde1aed` | Tree-shakeable entries plus retained direct-compiler artifact hook |
| MobXLil | `main` at `820c9a8` | Split source, true production-min config, and synchronized lockfile |
| Closure Compiler audit | commit `73eee2481cf1dd5dea0d8c9c0561b5a61498fec4` | Source comparison only; clone outside the repository |

Verify remote heads after pulling; never force-push. MotionLil and MobXLil had
diverged from `origin/main` before integration, so preserve both histories with
ordinary merges.

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
- Large-library matrix expanded to six pinned boundaries, including direct
  Motion animate and true MobX production-min artifact lanes.
- `--check-inputs` validates all six source/config archives.

## Known red or incomplete gates

- Full `cargo test --release --lib`: 1,595 passed and four existing tests failed:
  `keeps_js_push_and_empty_array_factories_prototype_observable`, two config-policy
  tests, and `stringify_elision_crosses_intervening_constants`.
- Historical checkpoint compiler `979dc90` times out on the newly pinned Motion
  and MobX production-min sources; those rows correctly emit no artifact.
- A migration compiler lane pinned to LilScript `06b89aa` is being added. The
  first combined run was user-aborted after both build commands started; rerun it.
- Only Motion `animate` direct output is in the matrix. Add `animateMini`,
  `animate`, `animate+stagger`, lab, export, mini, and full boundaries without
  relabeling esbuild/Terser package output as direct compiler output.
- V-02 and V-03 remain `blocked(gate-02)` until a complete five-fork G2
  selected-metric checkpoint passes.

## Live queue

1. Commit this handoff and matrix/compiler-lane changes.
2. Integrate all three repositories onto their local `main` branches using
   ordinary merges with `origin/main`; resolve without dropping either history.
3. Push LilScript, MotionLil, and MobXLil `main`; verify remote heads.
4. On the cloud VM, run `node scripts/board.mjs check` and
   `node comparison/large-libraries/run.mjs --check-inputs`.
5. Rerun the migration compiler on `motionlil,mobxlil-production-min`:

   ```sh
   export PATH="$HOME/.cargo/bin:$PATH"
   source "$HOME/.nvm/nvm.sh"
   nvm use 24.11.1
   node comparison/large-libraries/run.mjs --run \
     --compiler migration \
     --only motionlil,mobxlil-production-min \
     --output /absolute/temp/migration-new-lanes.json \
     --keep-temp
   ```

6. Pin the next migration compiler commit as the `after` side for V-02/V-03 and
   compare it with the `06b89aa` incumbent under zero-byte selected-metric policy.
7. Complete gate-02, then mark or reject `ident-09` and `emit-08` from evidence.
8. Resume canonical phase 0/0.5/1 work before starting target-JS or new syntax.

## Refusals

- No stale `dist`, dirty sibling source, post-minified compiler claim, package
  matcher, objective-dependent ABI, timeout increase used as a pass, or aggregate
  win used to hide one red boundary.
- Do not mark the migration complete from diagnostic preflights.
- Do not start new compression candidates while V-01 evidence/ABI admission and
  gate-02 remain open.
