# Executable contract model

This experiment checks the proposed compiler interfaces together before more
production feature migration. It is not a new shipping compiler, a performance
prototype in Rust, or a competitive minifier.

Read the [design gate](/home/azureuser/lilscript/lilscript-finer-structured/design/005-integrated-experiment.md)
and [measured decision](/home/azureuser/lilscript/lilscript-finer-structured/decisions/005-contract-core.md).

`model.py` owns immutable units, query dependencies, versioned use sets, atomic
edits, a small structured IR, an interpreter and a general JavaScript printer.
`check.py` constructs a library-shaped semantic fixture and explores actual
structural transformations. `check-artifacts.mjs` executes the emitted ESM
modules and measures those complete bytes with Node's codecs.

```sh
python3 finer/experiments/contract-core/check.py /tmp/lilscript-contract-result
node finer/experiments/contract-core/check-artifacts.mjs /tmp/lilscript-contract-result
```

The recorded run used worker 0 and Node 24.11.1. `report.json` includes executable
source hashes and codec/runtime versions; `model.json` includes every measured
artifact and the reference observations. Node codecs validate selection mechanics
here. They are not substituted for the fleet's pinned encoder service.

The integrated program creates private mutable state and two closures sharing it.
A host callback invokes and retains the observer. Returns and both source/host
throws cross a `finally` mutation. Separate factory calls have independent state.
A public sibling returns a mutable object whose keys remain observable.

Scalarization rewrites the allocation, field accesses and closure captures in one
transaction. Inlining captures actual arguments once in source order. Literal and
immutable-data choices are separate from exact-value queries. Data queries retain
knowledge after sharing; the old computation remains another candidate.

Assertions cover an unrelated local edit reusing facts, transitive invalidation,
a new whole-object use in an existing observer invalidating a no-escape proof,
producer-only layout rejection, write conflicts, transaction-budget recovery,
throw/divergence legality, effectful helper arguments, and the same exploration
path under zero/four/128 proposal budgets.

The model deliberately has a small admitted operation set and no source parser.
The fixture exercises wrapping i32 arithmetic and ASCII strings; it does not
validate the production canonical string/Unicode ABI.
Its structure verifier is not a complete type/equivalence verifier. It supports
direct private expression-helper inlining and retention of a shared helper, not
a general outlining pass. It has no SCC/loop analysis, native lowering, full
boundary/reflection contracts, per-result backdating, persistent cross-session
cache, hard RSS/deadline enforcement or production packaging/dependency graph.
The sample ESM exports are call/data contracts; function names, arity and
constructor/prototype reflection are not included in this fixture's public API.

Root dictionaries are shallow-copied and structural verification walks the model.
These are explicit model costs, not proposed evidence of fast production edits.
Retain this only as a bounded executable specification; do not grow it into a
second feature migration.
