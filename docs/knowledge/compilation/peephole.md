# Parsed peephole

Parent: [Compilation](README.md). Architecture:
[current architecture](current-architecture.md). Target:
[planned architecture](planned-architecture.md). Source: `src/js_peephole/`
(`optimize_generated_javascript` in `mod.rs`). Feature: `parsed-peephole`
(minimum `optimization_level` 9).

## Why it exists

IR emission can still leave legal JS that a **parsed** rewrite can contract. The
peephole tokenizes generated output, Pratt-parses eligible expressions, and runs
targeted structural/binding checks over the complete artifact.
It does **not** do unparsed text substitution.

It is also, today, a **second optimizer**: class identity fusion, copy
coalescing, ASI, integer coercions, declaration merging, and more. That second
job is architectural debt
([target-JS migration](../migration/planned-migration.md#phase-3-introduce-the-minimal-hygienic-target-js-tree)).
The intended end state is contraction of already-legal JS, always codec-scored
or skipped. Reconstructing `class` identity, inventing ternaries, or cloning
Terser `collapse_vars` as an always-on pass is glue
([objectives](objectives.md)).

## Three application modes

| Mode | Function | Codec-scored? |
|---|---|---|
| Search-on terminal leaves | `finalize_javascript_candidates` may clone a plan through `optimize_generated_javascript_assuming` | Yes, against the untouched declaration, inside `terminal_codec_probe_limit` |
| Late cleanup beam | `late_generated_javascript_cleanup_pass` per `LateJavaScriptCleanupPass` | Yes. Skipping a rewrite is a first-class branch. An all-pass synergy proposal is pinned because an individually losing precursor can enable a later win. |
| Canonical rewrite of the winner | `apply_selected_canonical_peephole` | Yes: requires remaining terminal work and uses the full priority rank/startup guards. |
| Search-off | `apply_search_off_declaration_peephole` | Yes: one function-preserving challenger is exactly measured against the untouched emit and retained only by the configured rank. |

`repair_late_javascript_candidate` additionally forces a short repair list
(or-assignment parens, fresh `Object.assign` fold, …) on strings being
validated. That is not a scored family.

## Implemented fold families

The session in `optimize_generated_javascript` is the authority, not this
bullet list. Modules under `src/js_peephole/folds/`:

| Module | Role (approximate) |
|---|---|
| `classes.rs` (~7.7k) | Prototype tables → `class`; `fold_named_class_identity`; drop orphaned `new.target` / identity helpers; async method spelling |
| `copies.rs` | Rematerialization, coalescing; identity bugs live here (`source_receiver_overwritten_between`) |
| `control.rs` / `boolean.rs` / `returns.rs` | `if`/`?:`/`||`, assigned truthy ternaries, return tails |
| `loops.rs` | `while`/`for` contraction, arguments-length countdown |
| `asi.rs` | ASI-safe semicolon elision |
| `declarations.rs` | `var` merging, unused bindings |
| `inline.rs` | Single-use function values, forwarding wrappers |
| `integers.rs` | `|0` / `+` coercions after emit |
| `calls.rs` / `members.rs` / `bodies.rs` / `syntax.rs` / `json.rs` / `arrays.rs` | Call/member/body/syntax/JSON/array local shapes |

Historical docs listed only compound assignment, unused bindings, declaration
fusion, arrow guards, `if`→conditional, flag-while rotation, and dead-`var`
reuse. Those still exist; they are not the majority of the pipeline.

Class fusion is **not** a substitute for IR named-class emit. It currently
parses a subset of function shapes, can fail on `async`, and is why
identity-observed ports froze search until the fold was made legal. See
[class identity](class-identity.md).

## Startup guard (`startup-cost-guard`, min level 1)

Compares syntax-derived parse / engine-compile / memory estimates to the configured baseline. Overhead percents are **hard rejects**. Weights break equal-transfer ties. Optional `javascript.startup.max_nesting` is an absolute ceiling even if the guard feature is not in an exact allowlist; `0` is invalid.

This is a deterministic proxy, not a browser measurement. Browser parse/compile remains a roadmap item.

## Performance shape (`performance-shape-model`, min level 3)

Static weights: deopt-sensitive control flow, allocations, unresolved indirect calls, hot-code (optional PGO). Combined with transfer according to `javascript.priority`. Not a runtime benchmark.
