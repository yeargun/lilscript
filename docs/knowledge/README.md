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
   and its topic pages describe the code that exists.
6. **Target and migration:** [compiler design](../compiler-design.md) records the objective, proposed architecture and open decisions; the single [migration plan](../migration/index.md) contains all implementation steps and verified progress.
7. **Proof of claims:** [verification](verification/README.md) defines valid evidence; [evidence](evidence/README.md) links results.
8. **Existing tools/history:** [finer](../../finer/README.md) retains measurement tools and old experiments; it does not coordinate the redesign.
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
| How do we decide the replacement together? | [Compiler design](../compiler-design.md) |
| Is a choice mandatory, ABI-fixed, or scored? | [Decision registry](compilation/decision-registry.md) |
| How are raw/gzip/Brotli winners selected? | [Objectives](compilation/objectives.md) -> [candidate search](compilation/candidate-search.md) |
| Is a size number publishable? | [Verification](verification/README.md) -> [evidence](evidence/README.md) |
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

- Architecture: [router](compilation/architecture.md),
  [current](compilation/current-architecture.md),
  [design discussion](../compiler-design.md)
- Policy: [objectives](compilation/objectives.md),
  [decision registry](compilation/decision-registry.md),
  [global optima](compilation/global-optima.md)
- Frontend/IR: [pipeline](compilation/pipeline.md),
  [linking/lowering](compilation/frontend-linking-lowering.md),
  [analyses](compilation/analyses.md), [optimizer](compilation/ir-optimizer.md),
  [DCE](compilation/dce-tree-shaking.md),
  [inlining](compilation/inlining-specialization-sharing.md),
  [aggregates](compilation/aggregate-lowering.md),
  [class identity](compilation/class-identity.md)
- Backend/search: [emission](compilation/javascript-emission.md),
  [mangling/layout](compilation/mangling-layout-pooling.md),
  [search](compilation/candidate-search.md),
  [peephole](compilation/peephole.md),
  [chunks](compilation/chunk-planning.md),
  [native](compilation/native-backend.md),
  [fallbacks](compilation/correctness-fallbacks.md)

### Operation

- [Config](config/README.md)
- [Delivery](delivery/README.md)
- [Verification](verification/README.md)
- [Evidence](evidence/README.md), including the
  [library proof matrix](evidence/library-proof-matrix.md) and
  [Motion](evidence/motion-compatibility.md), [Marked](evidence/marked.md),
  [MobX](evidence/mobx.md), [jQuery](evidence/jquery.md)
- [Joint design discussion](../compiler-design.md)
- [Research](research/README.md)

## Source Authority Map

| Concern | Primary source |
|---|---|
| Grammar and AST | `src/lexer.rs`, `src/parser.rs`, `src/ast.rs` |
| Semantics and effects | `src/semantic.rs`, `src/interpreter.rs` |
| Modules/packages | `src/module.rs`, `src/package.rs` |
| Typed IR and provenance | `src/ir.rs`, `src/lower.rs` |
| Compilation contract/objective/ABI | `src/compilation_contract.rs`, `src/config.rs` |
| IR optimization and proofs | `src/optimizer.rs`, `src/compress_passes.rs`, `src/value_analysis.rs` |
| Decision census and families | `src/decision_registry.rs` |
| Search and orchestration | `src/compiler.rs` |
| JavaScript emission | `src/codegen_ir_js.rs`, `src/js_syntax_target.rs` |
| Generated-JS migration layer | `src/js_peephole/` |
| Native backend | `src/codegen_native.rs` |
| Exact codecs | `src/bin/lilscript-codec.rs`, `benchmarks/codec-contract.mjs` |
| Semantic verification | `tests/cases/`, `src/bin/lilscript-differential.rs` |
| Compression verification | `comparison/cases/`, `comparison/algorithms/`, `comparison/large-libraries/` |
| Release gates | `scripts/release-check.sh`, `comparison/run-all.sh` |
