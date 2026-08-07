# Differential Semantic Testing

LilScript has an independent Rust reference evaluator for the typed scalar and
control-flow core. It walks the checked AST directly and does not call CFG
lowering, SSA optimization, constant-folding helpers, JavaScript codegen, or C
codegen. This separation makes it an oracle for transformations shared by both
production backends.

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
- short-circuit evaluation and the observable `print` intrinsic.

Struct/class instances, maps, sets, binary memory, and host calls are rejected
explicitly. They continue to be covered by the checked-in conformance matrix
until their independent evaluator models exist. A step budget and recursion
budget make a generated infinite program fail deterministically instead of
hanging a gate.

## Generated corpus

`lilscript-differential` uses a dependency-free deterministic PRNG to generate
typed functions containing nested integer and boolean expressions, every
integer binary and compound-assignment operator, overflow, zero divisors,
negative and oversized shift counts, branches, bounded loops, `break`,
`continue`, short-circuit side effects, shadowing, function calls, updates,
array aliases, indexed mutation, push/pop, captured arrows, and all four array
callback pipelines. Each callback appends to its receiver, checking that the
original iteration length is respected. The complete generated source and
oracle output remain under
`target/differential` after each run for reproduction.

```sh
cargo build --release --bins
target/release/lilscript-differential --cases 64
target/release/lilscript-differential \
  --cases 96 \
  --seed 0xdeadbeefcafebabe \
  --output-dir target/differential-deadbeef
```

For one generated batch, the harness requires exact output agreement from:

1. the checked-AST Rust evaluator;
2. production optimized JavaScript in Node;
3. JavaScript emitted with optional optimization disabled;
4. the native executable produced by `--target all`;
5. the emitted C compiled independently with the configured `CC`.

The fixed release seed is `0x6c696c7363726970`. During implementation, that
seed found an invalid `a--626380242` token boundary and two integer-expression
precedence failures involving nested shifts and `|0` coercions. Widening the
oracle to arrays also found native callback loops consuming elements appended
during `reduce`; all array callback loops now snapshot their entry length.
Regression tests pin these cases, and `scripts/verify.sh` runs the 64-case batch
on every release verification. Seeds `0xdeadbeefcafebabe` and
`0x0123456789abcdef` also pass 96 generated functions each.

This gate proves agreement only over generated programs in the documented
subset. It complements rather than replaces module, nominal aggregate,
map/set, binary-memory, browser, and library behavior suites.
