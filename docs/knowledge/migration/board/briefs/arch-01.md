# Brief — arch-01

For the clean-context documentation subagent requested on 2026-08-28. Read
[mission](../../../mission.md), this brief, and
[notes/arch-01.md](../notes/arch-01.md) before inspecting the named sources.

## Task

Read-only architecture audit against the **updated** knowledge tree, then a
dense current-state pass. The product documentation already names glue vs
search. Your job is to verify every material claim against source, correct
factual errors in the three new pages, and write a dense synthesis into the
note so a later session does not re-derive the compiler.

Done means: (1) each architectural claim in the new pages is confirmed,
contradicted, or marked unverified with a file:symbol; (2) the note contains a
dense current-state section that a cold reader could use instead of the 19k-line
coordinator; (3) no compiler/test/config files changed.

## Why this matters to the objective

LilScript aims to be the world's most optimized compressor by typed proof and
exact codec scoring, not by accumulating folds. Documentation that presents an
aspiration as shipped is how the next session adds more glue.

## Read

- `docs/knowledge/mission.md`
- `docs/knowledge/README.md`
- `docs/knowledge/compilation/architecture.md`
- `docs/knowledge/compilation/decision-registry.md`
- `docs/knowledge/migration/07-global-compressor.md`
- `docs/knowledge/compilation/README.md` and its directly linked children
- `docs/knowledge/language/README.md` and its directly linked children
- `docs/knowledge/config/README.md` and its directly linked children
- `docs/knowledge/migration/README.md`
- `docs/knowledge/migration/board/LEDGER.md`
- `src/config.rs` (`js_options`, `enables_compression`, `JavaScriptConfig`)
- `src/compiler.rs` (`optimize_and_select_javascript`, `select_javascript_candidate_global`, `extend_javascript_candidate_beam`, `apply_selected_canonical_peephole`, `apply_search_off_declaration_peephole`)
- `src/optimizer.rs` (pass order, scalar replacement, inlining)
- `src/compress_passes.rs`
- `src/codegen_ir_js.rs` (`IrJsOptions`, aggregate emit)
- `src/js_peephole/` especially `mod.rs` session order and `folds/classes.rs`
- `src/semantic.rs`, `src/lower.rs`, `src/ir.rs`, `src/module.rs`
- root `lilscript.toml` compression list

## May touch

- `docs/knowledge/migration/board/notes/arch-01.md`
- `docs/knowledge/compilation/architecture.md`
- `docs/knowledge/compilation/decision-registry.md`
- `docs/knowledge/migration/07-global-compressor.md`

Everything else is read-only. Edit the three product pages **only** to fix a
claim that source contradicts. Do not add new pages. Do not soften a gap into
an aspiration.

## Must not

- The [standing refusals](../README.md#standing-refusals): no glue, no post-minify,
  no weakened gate, semantics before size.
- Do not change compiler source, tests, configs, or benchmarks.
- Do not present a documented aspiration as implemented behavior.
- Do not treat a local byte count, heuristic proxy, or retained beam score as
  proof of a global optimum.
- Do not mark 07.1–07.7 complete; they are a plan.

## Prove it

```sh
git status --short
rg -n "optimize_and_select_javascript|apply_search_off_declaration_peephole|pool_identifier_strings|pack_string_arrays|ordinary_records_safe|scalar_replacement" src/compiler.rs src/config.rs src/optimizer.rs
```

Expected: every material claim in architecture.md / decision-registry.md cites
a source symbol; git status shows no subagent changes outside the four allowed
paths.

## Report

Append to `docs/knowledge/migration/board/notes/arch-01.md`: Evidence rows for
commands run, a dense **Current state** section (implemented decision layers,
highest-risk debt, doc errors you fixed), and a Log line ending in LANDED or
OPEN. Then return at most 20 lines: confirmed architecture, highest-risk debt,
any correction you made, migration ordering check, single next step. Do not
edit `LEDGER.md`.
