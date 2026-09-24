# Current status

**Updated 2026-09-24.** Plan phase M1 is active on branch `one-compiler`; no
phase after M0 is verified. The architecture is
[future-architecture.md](future-architecture.md); the plan and its progress are
[migration/index.md](migration/index.md); how a binary is checked is
[testing.md](testing.md). The status page of 2026-09-20, from before M1, is in
[history](knowledge/history/status-2026-09-20.md).

## One compiler

There is one compiler: parse → check → the typed Program IR → JavaScript
formation, target rewrites, naming and exact codec scoring, or the native C
writer ([current architecture](knowledge/compilation/current-architecture.md)).
The route that shared the binary with it until 2026-09-23 (typed CFG/SSA IR, IR
optimizer, string emitter, text peephole) is deleted. `--backend` fails with
"there is one compiler", and `[compiler] backend` is refused.

## What M1 did

| Task | Result |
|---|---|
| M1.1 Shared pieces | The delivery manifest, diagnostics, codec helpers and the reference-parameter span left the old route for their owners; output unchanged |
| M1.2 Tools | The language server runs a check-only session; the playground compiles with `strip_console = false`; lint runs on the checker and the Program IR; `--write-lock` no longer writes effect summaries |
| M1.3 CLI | `--backend` refused, `--profile-template` deleted; one target-to-request function serves `--print-policy` and the build; `-j` and `--codec-jobs` are accepted and warn that they have no effect yet |
| M1.4 Configuration | One retired-key table (61 keys, `RETIRED_KEYS` in `src/config.rs`) is applied before strict reading: each old key warns "no effect in this compiler" or refuses. Refused: `[compiler] backend`, `priority` other than `size-first`, `[policy.constraints]`, `public_aggregate_abi = "positional"`, a nonzero `for_of_specialize_family`. See [configuration.md](configuration.md#retired-keys) |
| M1.5 Tests | 288 regression cases harvested from the old route's executing tests (`tests/cases/regressions/`), each with an expected output derived from the test's own assertions |
| M2.2, M2.6 (brought forward) | The case runner (`scripts/cases.mjs`: 361 cases × 18 lanes) and the port runner (`scripts/ports.mjs`), each against an expected-failure ledger whose entries name an owner task |
| M1.6 Delete | The old route (`compiler.rs`, `lower.rs`, `ir.rs`, `optimizer.rs`, `codegen_ir_js.rs`, `codegen_native.rs`, `js_peephole/`, `decision_registry.rs` and the rest), the module linker and the AST's linker fields, the test-only annotated-tree experiment, the fixed two-file resource cut, search pairs and the pass-ablation benchmarks. `src/` went from 362K to 187K lines (−174.9K) |
| M1.7 Names and docs | The rename by role lands as one announced commit: `src/check`, `src/program`, `src/js`, `src/build.rs`, with `compile_source` and `compile_path`; the docs already use these names. Docs describing the old route moved to [history](knowledge/history/README.md); the contracts, this page and the architecture index describe the one compiler |
| M1.9 Correctness debts | Fixed: an `async` body folded to its value; a field default evaluated before a constructor argument; `JsValue == 0` lowered to `===`; a raw-objective build failing with "assignment requires a reference". Open: struct values passed to an `extern` are refused; a wrapper's inferred name disagrees with the spec in one of two sibling cases |

**Verification of the post-deletion binary** (`f749f6db…`, 2026-09-24):
- Case runner: no failure outside the ledger (229 ledgered case-lanes in 27
  cases; the C lanes mask 236 JavaScript-only cases). Against the pre-M1 binary,
  263 of 6,498 artifacts changed and no lane total grew: production Brotli, module
  lane, 31,056 → 30,833. Explaining each changed artifact is M1.8's open exit item.
- Port runner: the seven reference ports' suites are green (markedlil, zodlil,
  katexlil, jquerylil, posthoglil, motionlil, micromarklil).
- Before the deletion, all 27 maintained ports were built and tested on the
  pre-M1 binary; each failure is an owned entry in `tests/ports/expected-failures.json`.

## Standings

From the plan's "Where we are" (binary b80, 2026-09-23). "Old route" is the
frozen reference binary `~/lilscript-work/bin/reference-2026-09-23/lilscript`,
kept only to measure the first bar to clear.

| Measure | Old route | This compiler |
|---|---|---|
| 72 census cases, Brotli, `strip_console = false` | 5,866 | 8,576 |
| `comparison/cases`, Brotli wins/ties/losses against the smallest competitor | 53/1/0 | 9/6/38 (+1 refusal, 1 wrong output) |
| `comparison/algorithms`, Brotli total | 2,305 | 3,345 |

The old route's typed interprocedural optimization is still ahead on small closed
programs; plan phases M6 and M7 rebuild it as facts and rules on the Program IR.

On the 13 goal boundaries (the six reference ports and the react-markdown
family), patched scratch builds beat the strongest pinned bar on 11:
- katexlil, markedlil, posthoglil and jquerylil beat theirs under both objectives.
  katexlil's Brotli margin, −374, is inside D4's strict-win threshold of 630.
- The react-markdown family beats all seven bars (react-markdownlil browser:
  27,248 against 31,082).
- These come from scratch builds with `finer/port-migrations/*.patch` applied.
  The ports' committed `dist/` files and Pages sites were last rebuilt
  2026-08-29 to 09-04, on the old route.

## Open losses

- **motionlil:** +8,612 Brotli. Its shipped 49,644 is esbuild plus Terser over our
  output, not a compiler-written file.
- **zodlil:** the package is +16,396 Brotli as compiled (+12,211 after esbuild's
  whitespace minifier); the core is about 1.5–1.7K above the upstream slice.

Both are on the plan's work list (M12.3). No result counts unless the delivered
file is compiler-written, with no post-minifier (plan rule 6).

## Not restored by M1

Each has an owner task ("What M1 does not restore" in the plan): the `pure`
contract check (M6.3); removal of discarded pure calls (M7.2); native
`Record<T>`/JSON and the C extern ABI (M11.3, M11.4); native stack storage
(M11.5); same-named private classes or enums in two modules, and
`export constructor` (M4.1); `inline for` unrolling and `@pool` (M10.11); record
spread (M10.8); preserve-modules and lazy `import()` chunks (M3.3); source maps
(M8.6). Name-keyed host helpers are dropped by design.

## Next

- **M1 exit:** land the rename, explain every artifact difference against the
  pre-M1 binary (M1.8), fix the two open M1.9 debts, and pass the exit grep.
- **M2, verification ladder, baseline and interim release:** green CI on one
  Linux job (M2.1; CI has not been green since 2026-08-25), interpreter oracles
  and their extension to structs, classes, enums and collections (M2.3, M2.4), an
  independent Oxc parse of every delivered file (M2.5), a type-directed
  differential (M2.7), every port's rewrite committed to its own repository and
  the fleet rebuilt on the post-M1 binary as the baseline (M2.8), and the interim
  release with honest losses (M2.9).
