# Differential Semantic Testing

LilScript has an independent Rust reference evaluator (`src/interpreter.rs`)
for the typed scalar and control-flow core. It walks the checked AST directly
and never calls the compiler's elaboration, Program IR, JavaScript formation,
target rules or C writer. That separation makes it an oracle for every
transformation the compiler makes, on both targets.

The evaluator currently covers:

- signed 32-bit arithmetic, division and remainder by zero, bitwise operations,
  masked shifts, comparisons, booleans, floats, strings, null, and templates;
- lexical bindings through semantic symbol IDs, including shadowing and global
  mutation;
- direct functions, defaults, return, recursion limits, blocks, branches,
  `while`, `for`, `break`, `continue`, assignments, and prefix/postfix updates;
- first-class named functions and value-capturing arrow functions;
- reference-identity typed arrays, aliases, length, indexing, indexed
  assignment/update, `push`, `pop`, `map`, `filter`, `reduce`, and `forEach`,
  including callback-time mutation with entry-length snapshot semantics;
- fixed `ArrayBuffer` and `SharedArrayBuffer` storage, `Uint8Array` byte
  coercion, view metadata, copying slices, aliasing subarrays, and buffer/view
  identity;
- open `Record<T>` values with null-prototype key order and `??` reads;
- short-circuit evaluation and the observable `print` intrinsic.

Struct/class instances, maps, sets, and host calls are rejected explicitly.
Plan task M2.4 extends the interpreter to structs, classes, enums, generics and
collections, feature by feature.
They continue to be covered by the checked-in conformance matrix until their
independent evaluator models exist. A step budget and recursion budget make a
generated infinite program fail deterministically instead of hanging a gate.

## Generated corpus

`lilscript-differential` uses a dependency-free deterministic PRNG to generate
typed functions containing nested integer and boolean expressions, every
integer binary and compound-assignment operator, overflow, zero divisors,
negative and oversized shift counts, branches, bounded loops, `break`,
`continue`, short-circuit side effects, shadowing, function calls, updates,
array aliases, indexed mutation, push/pop, captured arrows, and all four array
callback pipelines. Each callback appends to its receiver, checking that the
original iteration length is respected. Every batch also starts with a fixed
prologue of pinned regression shapes: `Record<int>` snapshot, rebind and
captured-rebind functions, and a binary memory kernel covering byte coercion,
indexed updates, buffer/view aliasing, copying slices, shared storage, and
negative range indices. The complete generated source and oracle output remain
under `target/differential` after each run for reproduction.

```sh
cargo build --release --bins
target/release/lilscript-differential --cases 64
target/release/lilscript-differential --cases 64 --random-seed
target/release/lilscript-differential \
  --cases 96 \
  --seed 0xdeadbeefcafebabe \
  --output-dir target/differential-deadbeef
```

Without a seed flag the pinned seed `0x6c696c7363726970` makes the batch a
regression corpus: the same programs every run. `--random-seed` draws a fresh
seed, prints it before starting and repeats it on every divergence, so a failure
replays with `--seed <printed value>`. `scripts/verify.sh` draws a fresh seed
unless `LILSCRIPT_DIFFERENTIAL_SEED` is set.

For one generated batch, the harness requires exact output agreement between
the checked-AST reference evaluator and three JavaScript lanes of the one
compiler, each run in Node with `--target js` and a named configuration:

| Lane | Flags | What it checks |
|---|---|---|
| production | `--mode production --config lilscript.toml` | The repository's policy, which keeps `print`, with the candidate search |
| development | `--mode development --config lilscript.toml` | The same policy without the candidate search |
| formation-only | `--mode production --config tests/config/no-optimization.toml` | Every optional tactic vetoed and no search: formation and the mandatory work alone |

The formation-only lane is the optimizer-disabled baseline: a divergence that
appears only in production points at an optional transformation, one that
appears in all three at formation or the printer.

The native lanes are masked. Every generated program uses `Record<int>` (the
prologue above), which the native target refuses until native records land;
plan task M11.4 owns them and restores the native executable and
independently compiled C lanes. The case runner's C lanes cover the native
target meanwhile ([testing.md](testing.md)). Plan task M2.7 makes the generator
type-directed, with per-target masks, and moves its pinned prologue shapes to
`tests/cases`.

During implementation, the pinned seed found an invalid `a--626380242` token
boundary and two integer-expression precedence failures involving nested shifts
and `|0` coercions. Widening the oracle to arrays also found native callback
loops consuming elements appended during `reduce`; all array callback loops now
snapshot their entry length. Regression tests pin these cases. Those findings
were made on the compiler route deleted in plan M1; the prologue keeps their
shapes.

This gate proves agreement only over generated programs in the documented
subset. It complements rather than replaces module, nominal aggregate,
map/set, binary-memory, browser, and library behavior suites.
