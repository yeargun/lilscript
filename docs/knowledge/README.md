# LilScript knowledge tree

Parent: [`docs/README.md`](../README.md). Read top-down. Each level answers a
more specific question and links to detail instead of repeating it.

## Retrieval Order

1. **Intent:** [mission](mission.md) explains what the user wants and refuses.
2. **Current truth:** [`docs/current-status.md`](../current-status.md) states what
   exists, what is green, and what is not yet a win.
3. **Contracts:** [`language-v0.1.md`](../language-v0.1.md),
   [`configuration.md`](../configuration.md),
   [`modules-and-delivery.md`](../modules-and-delivery.md), and
   [`web-platform.md`](../web-platform.md) define public behavior.
4. **Durable rationale:** [design decisions](decisions/README.md) explain why the
   architecture chooses contracts, proofs, exact scoring, explicit ABI, and a
   narrow target representation.
5. **Implementation:** [current architecture](compilation/current-architecture.md)
   describes the one compiler as it exists now.
6. **Architecture and plan:** [future-architecture.md](../future-architecture.md) is
   the compiler's architecture and the size-relevant language design; the single
   [migration plan](../migration/index.md) contains all implementation steps and
   verified progress.
7. **Verification tools:** [testing.md](../testing.md) (the case runner, the port
   runner and their expected-failure ledgers); [verification](verification/README.md)
   defines valid evidence; [evidence](evidence/README.md) links results.
8. **History:** [history](history/README.md) describes the compiler route deleted in
   plan M1, as prior art. [finer](../../finer/README.md) retains measurement tools
   and old experiments; it does not coordinate the redesign.
9. **Research:** [research](research/README.md) contains experiments and rejected
   ideas. Load it only when the canonical pages cite a specific finding.

## Question Router

| Question | Start here |
|---|---|
| Is a proposal consistent with product intent? | [Mission](mission.md) |
| What does the language guarantee? | [Language contract](../language-v0.1.md) |
| What does a config key do? | [Configuration contract](../configuration.md) |
| Why is a semantic rule not a codec choice? | [Contracts before objectives](decisions/contracts-before-objectives.md) |
| Why not add a library-specific fold? | [Typed proofs, not glue](decisions/typed-proofs-not-glue.md) |
| What pipeline exists now? | [Current architecture](compilation/current-architecture.md) |
| What is the architecture we are building? | [Future architecture](../future-architecture.md) |
| How are raw/gzip/Brotli winners selected? | [Future architecture §9](../future-architecture.md#9-choices-search-and-the-objective) |
| How did the old route do something? | [History](history/README.md) |
| Is a size number publishable? | [Verification](verification/README.md) -> [evidence](evidence/README.md) |
| How do I check a compiler binary? | [Testing](../testing.md) |
| What happens next? | [Single migration plan](../migration/index.md); see readiness, dependencies and unverified gates |

## Domain Tree

### Language

[Language index](language/README.md)

- Intent: [types are not glue](language/types-not-glue.md),
  [compressor surface](language/compressor-surface.md)
- Values and control: [numerics](language/numerics-values.md),
  [functions](language/functions-closures-generics.md),
  [control/errors](language/control-flow-errors.md),
  [effects](language/effects-purity.md)
- Data: [aggregates](language/aggregates.md),
  [collections](language/collections-intrinsics.md),
  [async/generators/regex](language/async-generators-regex.md)
- Boundaries: [closed world](language/closed-world.md),
  [packages/ABI](language/packages-exports-abi.md),
  [escape/host](language/boundaries-escape.md),
  [modules/lazy](language/modules-lazy.md),
  [JavaScript vs native](language/js-vs-native.md)

### Compilation

[Compilation index](compilation/README.md)

- [Current architecture](compilation/current-architecture.md)
- [Future architecture](../future-architecture.md)
- [History of the deleted route](history/README.md)

### Operation

- [Config](config/README.md)
- [Delivery](delivery/README.md)
- [Verification](verification/README.md) and [testing](../testing.md)
- [Evidence](evidence/README.md), including the
  [library proof matrix](evidence/library-proof-matrix.md) and
  [Motion](evidence/motion-compatibility.md), [Marked](evidence/marked.md),
  [MobX](evidence/mobx.md), [jQuery](evidence/jquery.md)
- [Research](research/README.md)

## Source Authority Map

After plan M1.7's rename. Where a path moves again, the
[architecture's source layout](../future-architecture.md#15-source-layout-at-the-end-of-the-migration)
says where to.

| Concern | Primary source |
|---|---|
| Grammar and AST | `src/lexer.rs`, `src/parser.rs`, `src/ast.rs` |
| Checking (names, types, effects declared, boundaries) | `src/check.rs`, `src/check/` |
| Modules and packages | `src/module.rs`, `src/package.rs` |
| The Program IR: elaboration, verifier, facts, demand, families, search, artifacts | `src/program/` |
| JavaScript formation | `src/program/javascript*.rs` |
| JavaScript target tree: rewrites, naming, printing, delivery | `src/js/` |
| Native plan and C writer | `src/program/native*.rs`, `src/program/artifact_native.rs` |
| Public API (CLI, LSP, lint, playground) | `src/build.rs`, `src/main.rs`, `src/bin/` |
| Configuration, policy, contract | `src/config.rs`, `src/compilation_policy.rs`, `src/compilation_contract.rs` |
| Primitive semantics shared by both targets and the interpreter | `src/primitive.rs`, `src/typed_array.rs` |
| Reference interpreter | `src/interpreter.rs` |
| Exact codecs | `src/compression.rs`, `src/bin/lilscript-codec.rs`, `benchmarks/codec-contract.mjs` |
| Semantic verification | `tests/cases/`, `scripts/cases.mjs`, `src/bin/lilscript-differential.rs` |
| Port verification | `scripts/ports.mjs`, `tests/ports/expected-failures.json` |
| Compression verification | `comparison/cases/`, `comparison/algorithms/`, `comparison/large-libraries/` |
| Release gates | `scripts/release-check.sh`, `comparison/run-all.sh` |
