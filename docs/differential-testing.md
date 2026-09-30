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
- nominal structs with copies on stores, classes/constructors/inheritance and
  methods, enums/matches, generic calls, and identity-aware Map/Set operations;
- reference-identity typed arrays, aliases, length, indexing, indexed
  assignment/update, `push`, `pop`, `map`, `filter`, `reduce`, and `forEach`,
  including callback-time mutation with entry-length snapshot semantics;
- fixed `ArrayBuffer` and `SharedArrayBuffer` storage, `Uint8Array` byte
  coercion, view metadata, copying slices, aliasing subarrays, and buffer/view
  identity;
- open `Record<T>` values with null-prototype key order and `??` reads;
- UTF-16 strings, including lone surrogates, source identity/hex/Unicode escapes,
  slicing, code-unit indexing and splitting;
- short-circuit evaluation and the observable `print` intrinsic.

Host calls without a declared independent model, async/generator/exception
execution, dynamic JavaScript operations, unpaired-surrogate property keys and
some library operations are explicitly uncovered. They require independent
JavaScript host/observer fixtures. The evaluator never imports compiler IR,
formation or optimization helpers to fill a missing model. Its step/recursion
limits reject unbounded executions instead of inventing a result.

## Measured oracle coverage

```sh
cargo build --release --bin lilscript-oracle
target/release/lilscript-oracle tests/cases tests/cases/regressions \
  --json /tmp/oracles-new.json
# On subsequent qualifications, coverage may grow but must not silently shrink:
target/release/lilscript-oracle tests/cases tests/cases/regressions \
  --baseline /tmp/oracles-new.json --json /tmp/oracles-next.json
```

The command executes each source through the checked-AST evaluator and compares
its output byte for byte with the existing `.out`. A disagreement fails the
batch. The receipt hashes the evaluator/source/expected output and reports
actual covered cases, unsupported cases and their reasons; a feature name or
an unexecuted branch is not coverage. A `.host.js` or `.module-probe.mjs` case
requires that separate declared host model and is never counted as covered by
standalone interpretation. The case runner executes those fixtures.

`--write-missing` can create an absent `.out` **only from the independent
evaluator**, after all existing covered outputs agree. It never overwrites an
existing oracle. No compiler executable or compiler output is accepted by this
command. The report path must be new. Every optimization adds or selects the
independent oracle for the language and boundary behavior it changes before
its compiler-generated output becomes evidence.

## Generated corpus

`lilscript-differential` uses a dependency-free deterministic PRNG to generate
typed functions containing nested integer and boolean expressions, every
integer binary and compound-assignment operator, overflow, zero divisors,
negative and oversized shift counts, branches, bounded loops, `break`,
`continue`, short-circuit side effects, shadowing, function calls, updates,
array aliases, indexed mutation, push/pop, captured arrows, and all four array
callback pipelines. Each callback appends to its receiver, checking that the
original iteration length is respected. Expression generation is directed by
integer/boolean result type. A separate JavaScript mask generates varying
record keys, aliases, snapshots and writes; it cannot disable native lanes for
portable programs. The old pinned record/binary-memory prologue is now the
ordinary `tests/cases/differential_pinned_regressions.lil` conformance case.
Sources, independent expected output, configurations, artifacts, seed, compiler
identity and lane outcomes remain in a fresh output directory after each run.

```sh
cargo build --release --bins
target/release/lilscript-differential --cases 64 --features all
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

Each selected mask runs production JavaScript independently for raw, gzip and
Brotli, plus development and formation-only JavaScript. Production uses effort
13 and explicit policy version 3. Formation-only names
`tests/config/no-optimization.toml`, which vetoes optional tactics. Config files
are explicit, so the output directory cannot silently select another policy.

The portable mask also runs native executables and independently compiled C
for production, development and formation-only: eleven execution lanes. The
JavaScript record mask runs five JavaScript lanes and records its native
capability exclusion. `--features portable|javascript|all` selects the masks;
`all` is the default. `LILSCRIPT_NATIVE_CLANG`, then `CC`, selects the independent
C compiler. Inputs, integer wraparound and observable output are identical in
all eligible lanes.

During implementation, the pinned seed found an invalid `a--626380242` token
boundary and two integer-expression precedence failures involving nested shifts
and `|0` coercions. Widening the oracle to arrays also found native callback
loops consuming elements appended during `reduce`; all array callback loops now
snapshot their entry length. Regression tests pin these cases. Those findings
were made on the compiler route deleted in plan M1; the checked-in regression cases retain their shapes.

This gate proves agreement only over generated programs in the documented
subset. It complements rather than replaces module, nominal aggregate,
map/set, binary-memory, browser, and library behavior suites.
