# Current compiler architecture

Parent: [compilation](README.md). Authority: the source and its tests. Status:
[current status](../../current-status.md). Where it is going:
[future architecture](../../future-architecture.md) and the
[migration plan](../../migration/index.md).

This page describes the compiler in the checkout of 2026-09-24, after plan
phase M1. There is one compiler. The route that preceded it (typed CFG/SSA IR,
IR optimizer, string emitter, text peephole) was deleted in M1; its pages are in
[history](../history/README.md). Where today's code falls short of the
architecture, the plan task that closes the gap is named.

## Shape

```text
lilscript.toml + source graph
  -> parse                     src/lexer.rs, src/parser.rs, src/ast.rs
  -> module discovery          src/module.rs, src/package.rs
  -> check                     src/check.rs, src/check/        one module-graph checker
  -> elaborate once            src/program/from_source.rs     checked syntax -> Program IR
  -> program core              src/program/                   uses, facts, demand, families
  -> JavaScript
       formation               src/program/javascript*.rs     Program -> JS target tree
       target rewrites         src/js/                        a hand-ordered chain (below)
       naming, print, delivery src/js/
       admission and search    src/program/artifacts.rs, search*.rs
  -> native C                  src/program/native*.rs, artifact_native.rs

policy: src/config.rs -> src/compilation_policy.rs (ResolvedPolicy, BudgetLedger)
entry:  src/build.rs (compile_source, compile_path, check_source, check_path,
        with_checked_source, with_checked_path)
```

The CLI (`lilscript`), the language server (a check-only session), lint and the
playground call `src/build.rs` directly. Lilpack, the Vite plugin and
`lilscript-differential` run the CLI; the differential harness also runs the
independent reference interpreter.

## Stages

| Stage | What it does today | Open against the architecture |
|---|---|---|
| Parse | Tokens, syntax, spans. Node ids exist on expressions only | Ids on identifiers, declarations and statements (M4.4) |
| Check | Name resolution, types, definite initialization, narrowing, boundary and frame rules, `ref` places. Classes and enums are keyed by name, so two modules' private classes or enums with the same name are refused, and `export constructor` is refused | `NominalId` for every nominal kind (M4.1). `pure` is parsed and recorded but not checked (M6.3) |
| Elaborate | Converts the checked program once into the Program IR (`from_source.rs`) and verifies it (`verify.rs`) | Class fields are lowered to name-keyed members here (M4.1); `inline for` becomes an ordinary loop and `@pool` is ignored (M10.11) |
| Program IR | Units (module initializers, functions, closures) holding ordered operations in nested regions; values with one definition; cells for mutable storage; places; calls split into prepare and call. There is no CFG | A structural edit kernel (M5.1) |
| Facts | Per-unit exact values, evaluation behavior and primitive domains (`facts.rs`, `raw_domains.rs`). Every user call is an unknown effect | The fact spine: call graph, effects, values, escape, fields, initialization order (M6) |
| Demand | Whole-program liveness with an observation lattice (`demand*.rs`): the production dead-code elimination, JavaScript only | Applied as an edit, for native too (M5.1) |
| Families | Five hand-written representation choices (record, product, helper, string, function layout), held per candidate in an `ImplementationMap` | One `Choice` interface (M9.1) |
| Formation | Projects the demanded program onto the JavaScript target tree, then runs about 40 target rewrites in a fixed, hand-written order (`form_with_demand`), gated as one tactic, `target-compaction`. Most of today's bytes come from this chain | A scheduler (M5.3), program rules that remove operations (M7), canonical formation (M8) |
| JavaScript target tree | One owner per node, identities for bindings, scopes, functions and regions, retained language operations until printing, no raw-text node, a verifier with edition checks | Annotation columns carrying facts to the tree (M5.2) |
| Naming and printing | Names are allocated per scope by one of three naming styles, crossed with two literal modes; the printer renders the verified tree | Naming as a codec-judged choice (M9.5); a printer with no structural rewrites (M8.3) |
| Delivery | Entry, chunks, imports and exports, the manifest; `host_modules` decides whether foreign JavaScript travels with the output | preserve-modules chunks and lazy `import()` chunks are broken today (M3.3) |
| Admission and search | One admission function (`ResolvedPolicy::admit_evidence`) and exact codec scores: Brotli quality 11 / window 22, gzip level 9, raw. A bounded beam over family unions, naming plans and literal modes, under the effort level's budgets | An independent Oxc parse of every delivered file (M2.5); monotone selection and the terminal slot (M5.4) |
| Native | A native plan chooses layout, ownership and calls; a C11 writer spells them with runtime helpers. Strings are UTF-16 code units; the floating-point ABI is pinned. The C is clean under ASan, UBSan and LSan | Refused today: `Record<T>` and JSON (M11.4), the user-facing C extern ABI (M11.3), exceptions, async, generators and regex (M11.6). Native gets no program optimization yet (M11.5) |
| Host modules | Foreign JavaScript or TypeScript named by `import extern` is parsed by Oxc and walked as ESTree JSON (`src/host_modules.rs`) | A typed visitor producing host units (M8.4) |

## Configuration and policy

`src/config.rs` reads `lilscript.toml` in two steps: the retired-key table first
(a key of the deleted route warns "no effect in this compiler" or refuses the
build), then strict deserialization. `src/compilation_policy.rs` resolves the
result, for each requested target, into one `ResolvedPolicy`: the JavaScript
contract (`src/compilation_contract.rs`), the objective (`cost_model`), the
effort level, each tactic's permission, and resource ceilings, with a
fingerprint. `--print-policy` prints it. One `BudgetLedger` accounts logical work
and retained bytes for the whole build. See
[configuration.md](../../configuration.md).

## Verification

- `cargo test`: unit and integration tests, including the D3 clause tests.
- The case runner (`scripts/cases.mjs`) and the port runner
  (`scripts/ports.mjs`), each against an expected-failure ledger:
  [testing.md](../../testing.md).
- The reference interpreter (`src/interpreter.rs`) and
  [differential testing](../../differential-testing.md), independent of the
  compiler's lowering.
- `scripts/verify.sh`: JavaScript against C on the example programs and
  `tests/cases`, the differential batch, the LSP smoke test and the bundle
  contract.

## What the old route had and this compiler does not yet

The plan lists each with its owner ("What M1 does not restore" in
[migration/index.md](../../migration/index.md)). In short: the `pure` contract
check (M6.3), removal of discarded pure calls (M7.2), typed interprocedural
optimization (M6–M7: small closed programs still compiled smaller on the old
route), native `Record<T>`/JSON and the C extern ABI (M11.3, M11.4), native
stack storage (M11.5), and source maps (M8.6). Name-keyed host helpers are
dropped by design: an `extern` means nothing by its name.
