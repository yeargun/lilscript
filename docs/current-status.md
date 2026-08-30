# Current status

Authority: current source/tests for behavior; generated fingerprinted reports for
numbers. Updated: 2026-08-29 during final-artifact admission.

This page is the single prose snapshot of current state. It is not a language
contract or a plan. When it disagrees with source/tests or a generated report,
those authorities win.

## Implemented

- Typed parse, link, semantic, CFG/SSA lowering, optimization, and JavaScript
  emission are the production path. C/native uses a separately optimized copy of
  the same lowered IR.
- `JavaScriptCompilationContract`, `JavaScriptOptimizationObjective`, and
  `JavaScriptAbiManifest` separate much of legality/ABI from profitability. The
  compiler now emits an observed final-artifact witness for syntax, exports,
  imports, free names, templates, obligations, binding ranges, and property ranges.
  Cross-objective live-binding and exported-class descriptor/order fixtures pass;
  exact receiver identity for shared property spellings remains target-JS work.
- Source/generated operation provenance exists. A live source `value | 0` has a
  lowering obligation that objective search may not erase.
- All 77 `IrJsOptions` fields are classified. The current registry exposes 48
  scored emission families plus scored IR variants, reversible priors, retained
  incumbents, reserved work, and starvation reporting.
- Named constructor-value class emission, `export constructor`, expression
  `if`, scalar literal `match`, ordinary `object{...}`, owned property identity,
  and immutable closure-snapshot alternatives are implemented.
- Final JavaScript still passes through generated-text parsing and contraction.
  A hygienic identity-bearing target-JS representation is the active
  architecture gap.

Details: [current architecture](knowledge/compilation/current-architecture.md).
Live task rows: [ledger](knowledge/migration/board/LEDGER.md).

## Last verified gates

| Gate | Result |
|---|---:|
| Rust release library tests | 1,627 passed |
| Rust release all-targets | passed |
| Canonical paired cases | 54/54 passed |
| Codec contract | 10/10 passed |
| MotionLil | 9/9 passed |
| MarkedLil | 29/29 passed |
| MobXLil | 769 enabled passed, 11 skipped |
| jQueryLil | 6/6 passed |

These counts describe only their tested boundaries. A skipped or untested API is
not proven compatible.

## Size state

The latest local comparison against the frozen pre-change compiler is mixed, not
an overall win. Brotli deltas (`after - before`) were:

| Artifact | Delta |
|---|---:|
| Motion `mini` direct | -593 B |
| Motion `animateMini` direct | -1 B after naming recovery |
| Motion `animate` direct | 0 B |
| Motion `animate+stagger` direct | 0 B |
| Motion lab direct | 0 B |
| Motion export direct | 0 B |
| Motion full direct | -69 B |
| Marked selected Brotli | -9 B versus the best committed historical artifact; exact `06b89aa`/current tie |
| MobX regular | -23 B |
| MobX `production-min` | -521 B on the first reproducible same-source/config compiler pair |
| jQuery compiler incumbent | 0 B |

The former Motion losses were package-bundled
measurements, not direct compiler boundaries; exact direct `2d2268` comparisons
are shown above. The former MobX +1,230 row mixed true
production-min output with an older regular-production artifact; exact
`2d2268` (16,012) versus `06b89aa` (15,491) improves by 521 Brotli bytes. The
unusually small committed 15,083-byte min artifact has no pinned compiler identity
and is not a legal incumbent. Current Marked is 9,506 Brotli versus the best
committed 9,515 artifact, and jQuery ties its exact migration incumbent. These
numbers must not be copied into product claims.

## Active direction

1. Make evidence replayable and distinguish direct compiler output from a
   downstream deployment pipeline.
2. Strengthen expected-versus-observed ABI and final syntax/binding validation.
3. Recover legal incumbent artifacts before expanding architecture or search.
4. Finish the smallest hygienic target-JS representation needed to stop
   rediscovering identity from generated text.
5. Use reusable language proofs and measured decision families to close
   maintained library losses without package-specific compiler logic.

Target: [planned architecture](knowledge/compilation/planned-architecture.md).
Execution order: [planned migration](knowledge/migration/planned-migration.md).
