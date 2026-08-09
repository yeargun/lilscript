# LilScript

LilScript is a standalone, statically typed language with type-first declarations,
nominal structs and classes, typed closures, and JavaScript-style array and
string methods. Generic functions and classes use inferred, statically checked
type arguments. It has its own lexer, parser, type system, SSA IR, optimizer,
JavaScript backend, and native C backend. It does not parse or emit TypeScript.

```lilscript
class Vector {
  float x;
  float y;

  init(float x, float y) {
    this.x = x;
    this.y = y;
  }

  float lengthSquared() {
    return this.x * this.x + this.y * this.y;
  }
}

int[] values = [1, 2, 3, 4];
auto doubled = values.map((int value) => value * 2);
int sum = 0;
for (int index = 0; index < doubled.length; index++) {
  sum += doubled[index];
}
Vector vector = new Vector(3.0, 4.0);

if (vector.lengthSquared() == 25.0) {
  print(`sum=${sum}`);
} else {
  print("invalid");
}
```

The optimized JavaScript for this program is 111 bytes. The class is
devirtualized and scalar-replaced, the method and constructor disappear, and
the output preserves signed 32-bit `int` behavior.

Generic values keep the same type-first declaration style:

```lilscript
class Box<T> {
  T value;

  init(T value) {
    this.value = value;
  }

  T get() {
    return this.value;
  }
}

T apply<T>(T value, func(T)->T transform) {
  return transform(value);
}

Box<int> box = new Box(7);
print(apply(box.get(), (int value) => value + 1));
```

## Toolchain

Rust 1.85 or newer is recommended. Native output additionally requires a C11
compiler such as Clang. The Vite 8 projects require Node.js 20.19+, 22.12+, or
newer; the benchmark suite additionally requires Java and curl.

```sh
cargo build --release

# Optimized JavaScript to stdout or a file
target/release/lilscript examples/v01.lil
target/release/lilscript examples/v01.lil -o app.js

# Reusable ESM with retained, mangled named exports
target/release/lilscript tests/modules/esm-entry.lil --target js-module -o library.mjs

# Static ESM chunks plus app.manifest.json, controlled by lilscript.toml
target/release/lilscript src/main.lil --target js-module -o build/app.mjs

# Portable C or a native executable
target/release/lilscript examples/full_conformance.lil --target c -o app.c
target/release/lilscript examples/full_conformance.lil --target native -o app

# Parse and check once, then produce optimized app.js, app.c, and app
target/release/lilscript examples/full_conformance.lil --target all -o build/app

# Generate stable function/loop keys for an optional external PGO profile
target/release/lilscript src/main.lil --profile-template lilscript.profile.json
```

The native target invokes `${CC:-clang}` with C11 and `-O3`.

Compiler policy is configured in an auto-discovered `lilscript.toml`, or with
`--config path/to/lilscript.toml`. Presets and per-pass overrides control
folding, CSE, global optimization, inlining, scalar replacement, DCE, identifier
and boundary-property mangling, public-export mangling, and string pooling.
Internal aggregates lower to scalars or positional arrays; escaped values use
their named boundary ABI, and `extern class` host names stay exact. The default
JavaScript-specific `priority = "size-first"` policy minimizes the
selected release transfer metric. Four profiles, numeric
inline budgets, a JavaScript search-effort level from 0 to 15, exact
`optimizations` and `compression` allowlists, and deterministic startup-cost
limits control the performance/size tradeoff without changing C/native
optimization.
Production builds use an exact configurable raw, gzip-9, or Brotli-11 cost
model to select among bounded pooling, literal-table packing, coercion-elision,
boolean-literal, identifier-alphabet, quote-style, structured-closure, and
standards-valid grammar-elision, declaration, conditional/comma, loop/update,
switch-dispatch, mutation, and
SSA-copy-layout candidates, including declaration orders aware of gzip's
32-KiB and Brotli's configured 4-MiB history windows, plus configured,
closure-factory-preserving, unspecialized, and fully outlined optimizer IRs. A
parsed final peephole and
syntax-derived parse/compile/memory guard run before selection. A separate
optional generated-syntax nesting ceiling provides a hard cross-engine guard.
The typed-IR performance model separately scores deoptimization-sensitive
control flow and runtime shapes, allocation pressure, unresolved indirect
calls, and known monomorphic calls. Optional versioned profile counters weight
hot functions and loops and drive bounded constant/callback specialization and
constant-capture closure cloning without source annotations;
`--mode development` skips that compressor loop. `--explain human|json`
reports optimizer passes, transfer/startup/performance metrics, candidate and
rewrite counts, and compiler time. The
size default omits signed-32-bit coercions only where range analysis proves
them redundant. It never introduces `Math.imul`: ordinary `int` multiplication
uses JavaScript multiplication followed by signed-i32 normalization. A
source-written `Math.imul(left,right)` is preserved as the explicit exact
low-32-bit operation for code that deliberately needs it.
Integer `&`, `|`, `^`, `<<`, `>>`, and `>>>` operate on signed 32-bit values;
shift counts are masked to five bits. Use `value.toUnsignedString(radix)` when
the unsigned bit pattern, rather than the signed integer, must cross a string
boundary.
Bundle policy selects a single artifact, source-module-preserving ESM chunks,
or deploy-cost-scored shared and lazy chunks. Exact raw/gzip/Brotli bytes,
requests, dependency depth, preload policy, reachability, and cache reuse feed
the chunk score. See
[docs/configuration.md](docs/configuration.md) for the complete schema and exact
chunk eligibility rules.

## Modules and tree shaking

Static imports are compiler inputs, not emitted JavaScript wrappers:

```lilscript
// math.lil
export pure int square(int value) {
  return value * value;
}

// main.lil
import { square as sq } from "./math";
print(sq(5));
```

Compiling `main.lil` discovers the complete relative module graph, validates
exports, isolates private names, and optimizes the linked program once. Cross-file
inlining and DCE can reduce this example to `console.log(25)`. Exported code is
removed when unreachable. Purity is inferred automatically; `pure` is an
optional checked contract, and `pure extern` is a trusted host promise.

`--target js-module` is the reusable-library mode. It preserves the root
module's runtime exports as optimization roots, still removes private dead code,
and emits compact ESM aliases such as `export{b as square,a as answer}`. Export
lists support aliases (`export { internalName as publicName };`). Struct and
class exports are compile-time type exports; functions and globals are runtime
ESM exports. The default `js`, `c`, `native`, and `all` targets remain
closed-world executable builds, so their exports do not prevent DCE.

`import("./feature")` returns a typed asynchronous module value. Its `then`,
`catch`, and `finally` callbacks are checked, unused namespace exports are tree
shaken, and split builds emit a real lazy chunk with normalized load failures.
Lazy-only modules are initialization-free by contract.

When `bundle.mode` is `split` or `preserve-modules`, JavaScript output becomes
an ESM entry plus sibling chunks and a deterministic manifest containing
transport sizes, dependency edges, content hashes, preload files, and deploy
costs. The whole program is still optimized before partitioning. C and native
output stay single artifacts; dynamic module tasks themselves are JavaScript
only.

Bare imports resolve from content-verified path dependencies in
`lilscript.lock`. Run `lilscript src/main.lil --write-lock -o build/app.js` to
pin the transitive semver/ABI graph. Normal builds reject stale source hashes
and never rewrite the lock. The full contract is in
[docs/modules-and-delivery.md](docs/modules-and-delivery.md).

## Playground

```sh
cd web
npm install
npm run build
cd ..
cargo run --release --bin lilscript-playground
```

Open `http://127.0.0.1:4173`. The playground compiles LilScript on the Rust server,
shows generated JavaScript and source diagnostics, and executes output in a
sandboxed iframe. The same vanilla Vite project includes `/docs.html`,
`/benchmarks.html`, `/libraries.html`, `/explorer.html`, `/roadmap.html`, and
`/about.html`; it contains plain HTML, CSS, and JavaScript with no Astro files.
The explorer is generated from checked benchmark JSON and real repository
source files. It filters and sorts 32 projects / 147 artifact lanes, and each
project opens a separate source, method, package, and artifact detail page.

For Vite development with hot reload, run the compiler API and Vite in separate
terminals:

```sh
cargo run --release --bin lilscript-playground
cd web && npm run dev
```

Open `http://127.0.0.1:5173`. Vite proxies `/api/compile` to the Rust server on
port 4173.

## VS Code

The repository includes a native language server and installable VS Code
extension with `.lil` syntax highlighting, compiler and linter diagnostics,
completion, hover documentation, snippets, symbols, semantic tokens,
scope-aware rename/references, formatting, import organization, and quick fixes.

```sh
cargo build --release --bin lilscript-lsp
cd vscode-extension
npm install
npm run package
code --install-extension lilscript-vscode-0.1.0.vsix
```

The extension discovers the language server in this repository's release or
debug target directory, then falls back to `PATH`. Set `lilscript.server.path` in
VS Code settings when the executable is installed elsewhere.

The standalone Rust tools use the same project configuration:

```sh
target/release/lilscript-lint src --deny-warnings
target/release/lilscript-lint src --format sarif
target/release/lilscript-fmt src --check
```

Lint presets, per-rule severities, suppressions, formatter policy, and global
disable switches are configured in `lilscript.toml`. The library API also
accepts zero-copy Rust `LintRuleProvider` implementations over the checked
module and optimized IR. Provider namespaces can be enabled independently, so
project rules remain separate from the compiler and keep stable rule IDs for
JSON, SARIF, the LSP, and coding agents.

## Web platform

Browser objects use typed, zero-wrapper host declarations. Exact host names are
preserved even when property mangling is enabled:

```lilscript
extern class Document {
  Element createElement(string tag);
  Element? querySelector(string selector);
}
extern Document document;
```

`document.createElement(...)` emits as the same direct JavaScript operation.
`ArrayBuffer`, `SharedArrayBuffer`, `Uint8Array`, and `Float32Array` are core
types with direct JavaScript lowering and portable storage lowering for C. Browser host-object
access itself is rejected by native targets unless it is isolated behind an
ordinary user-defined `extern` function ABI. The exact scope and deployment
requirements are in [docs/web-platform.md](docs/web-platform.md).

## Compiler Pipeline

```text
LilScript source
  -> Logos lexer
  -> bumpalo arena recursive-descent parser
  -> static module graph resolution and private-name linking
  -> symbols, scopes, type checking, capture analysis
  -> typed control-flow IR
  -> mem2reg SSA and phi insertion
  -> devirtualization and inlining
  -> proof-driven private implementation sharing
  -> constant/branch folding, GVN, algebraic simplification
  -> pure-call, dead-store, unreachable and whole-program DCE
  -> escape analysis and scalar replacement
  -> liveness coalescing and frequency-ranked mangling
  -> optimized JavaScript and/or native C
```

The major implementation boundaries are in `src/lexer.rs`, `src/parser.rs`,
`src/module.rs`, `src/semantic.rs`, `src/lower.rs`, `src/ir.rs`, `src/optimizer.rs`,
`src/codegen_ir_js.rs`, and `src/codegen_native.rs`. The executable language
contract is [docs/language-v0.1.md](docs/language-v0.1.md).

## Verification

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
scripts/verify.sh
benchmarks/run.sh
node benchmarks/finite-values/run.mjs
node benchmarks/function-folding/run.mjs
node benchmarks/function-subsumption/run.mjs
node benchmarks/function-layout/run.mjs
node benchmarks/ir-variants/run.mjs
node benchmarks/profile-guided/run.mjs
npm --prefix benchmarks/apps ci
npm --prefix benchmarks/apps run benchmark
npm --prefix benchmarks/libraries ci
npm --prefix benchmarks/libraries run benchmark
npm --prefix benchmarks/popular ci
npm --prefix benchmarks/popular run benchmark
npm --prefix benchmarks/browser ci
npm --prefix benchmarks/browser run install-browser
npm --prefix benchmarks/browser run benchmark
npm --prefix benchmarks/scenarios ci
npm --prefix benchmarks/scenarios run benchmark
node benchmarks/paired/run.mjs
npm --prefix web run build
npm --prefix vscode-extension run package
```

`scripts/verify.sh` compares Node and native output for two conformance suites
and links a generated aggregate ABI against a C host. It also runs 69 programs
through JavaScript, emitted C, and native executables with maximum and disabled
optional optimization, for 138 matrix executions, plus a framed LSP session
through diagnostics, completion, hover, symbols, semantic tokens, references,
rename, formatting, quick fixes, and shutdown.
The verification script also generates 64 deterministic typed programs and
includes byte and Float32 binary-memory kernels, then requires an independent
checked-AST Rust evaluator,
optimized JavaScript, optimizer-disabled JavaScript, direct native output, and
independently compiled emitted C to agree exactly. The evaluator scope and seed
reproduction commands are in
[`docs/differential-testing.md`](docs/differential-testing.md).
`benchmarks/run.sh`
downloads the pinned Closure Compiler `v20260803`, runs `ADVANCED` compilation,
checks equivalent runtime output, and measures normalized raw, gzip-9, and
Brotli-11 bytes.
The isolated profile-guided higher-order-call fixture executes the same output
contract before measuring and changes from `111/107/80` to `107/104/77`
raw/gzip-9/Brotli-11 bytes. This demonstrates one bounded specialization win;
it is not a universal PGO size claim.

The finite-value ablation holds inlining and scalar replacement disabled in
both variants and toggles only interprocedural finite-value propagation. Both
artifacts execute the same contract; the pass reduces the checked workload from
`216/155/118` to `143/108/77` raw/gzip-9/Brotli-11 bytes.
The inlining-IR ablation holds every other optimizer and emitter decision
constant; exact codec selection improves `267/144/109` to `219/113/83`.
Late identical-private-function folding improves its isolated workload from
`177/139/111` to `123/129/105`. Compressor-selected declaration layout keeps
raw size at `1,133` while improving gzip/Brotli from `460/369` to `454/362`;
source order remains a candidate, so the heuristic is not forced.
Proof-driven private-function subsumption binds an existing broader
implementation's extra scalar or known-function parameter and improves its
isolated workload from `445/217/179` to `351/201/172`. Exported and
address-taken identities are excluded, and the untouched optimizer IR remains
codec-scored.

The source-neutral lane in `benchmarks/paired` mechanically generates readable
LilScript and JavaScript from one workload schema. Every case must agree through
Closure JavaScript, LilScript JavaScript, emitted C, and native execution, and
LilScript may not exceed Closure in any per-case Brotli cell under the project
default while raw and gzip tradeoffs remain published. The
separate Chromium gate uses alternating warmed samples and requires the 95%
bootstrap upper runtime ratio to remain at or below `1.03`. These are scoped
regression gates, not universal compiler-superiority claims.

On the repository's ten compiler workloads LilScript totals 1,457 raw / 1,280
gzip / 1,032 Brotli bytes versus Closure at 2,450 / 1,771 / 1,471. LilScript is
smaller in all 30 measured cells. The separate application lab
compares five readable JavaScript references with matching-scope LilScript,
feeds those exact references to Closure `ADVANCED`, and keeps hand-specialized
JavaScript as an oracle. Its checked-in run totals 1,794 raw / 1,283 gzip /
1,088 Brotli bytes for LilScript versus 2,231 / 1,512 / 1,303 for Closure; the
hand oracle remains smaller at 1,008 / 840 / 745. Real Alien Signals, mitt, and
Motion applications are built separately by Vite and excluded from compiler
totals. All 26 comparable/diagnostic JavaScript artifacts, three Vite package
builds, and six native executables pass checked-in output contracts. Matching
those contracts is regression evidence, not proof of complete library
compatibility. In the checked-in 25-sample module-evaluation run, LilScript is
1.038x Closure's runtime and hand-specialized JavaScript is 0.681x, so runtime
parity is not yet claimed. Compiler methodology and tables are in
[docs/benchmark-results.md](docs/benchmark-results.md); the pass-by-pass
responsibility mapping is in
[docs/optimization-coverage.md](docs/optimization-coverage.md), and the Vite
8/Oxc versus Closure `ADVANCED` architecture audit is in
[docs/vite-closure-minification-audit.md](docs/vite-closure-minification-audit.md).

The complete-library lab measures installed npm packages against LilScript
ports after translated upstream assertions, dense differential API checks, and
JavaScript/C/native app contracts. All seven currently selected complete
entrypoints pass the raw/Brotli, throughput, and retained-memory publication
gates against npm/Vite 8 and public-surface-preserving Closure ADVANCED. Their
LilScript Brotli sizes are 272 bytes for `@motionone/easing`, 131 for `clamp` +
`lerp`, 106 for `string-hash`, 408 for `js-levenshtein`, 234 for
`@emotion/hash`, 414 for `murmurhash-js`, and 6,192 for the complete
`robust-predicates` root. Those corpus results are published without a universal
superiority claim in
[benchmarks/libraries/RESULTS.md](benchmarks/libraries/RESULTS.md) and at
`/libraries.html` in the Vite site.

The separate populated-package corpus only publishes complete selected
entrypoints after algorithm, public-API, differential behavior, throughput,
retained-memory, and raw/selected-codec size gates. Its current eligible rows
beat or tie npm/Vite 8 and public-API-preserving Closure `ADVANCED` in Brotli:
Nano ID is `408 / 409 / 414`, mitt is `300 / 300 / 311`, clsx is
`481 / 493 / 499`, and gl-matrix is `14,056 / 14,330 / 14,328` bytes
(LilScript / Vite / Closure). The clsx port preserves its raw recursive
JavaScript-value algorithm without an allocation-changing conversion facade.
Incomplete ports remain excluded from that claim; see
[benchmarks/popular/RESULTS.md](benchmarks/popular/RESULTS.md) and
[benchmarks/popular/PERFORMANCE.md](benchmarks/popular/PERFORMANCE.md).

The [solidlil lab](labs/solid-client) is pinned under `labs/solid-client` as a
Git submodule (its independent repository remains
[lilscript-solid-lab](https://github.com/yeargun/lilscript-solid-lab)). It
compares todolist apps with no framework-identifying UI strings. The primary
lane is Solid JSX versus solidlil **LSX** (LilScript reactive + LilScript DOM):
Brotli-11 is 32.1% smaller for solidlil (3,722 vs 5,479). Its 15-sample jsdom
interaction median is 1.028× Solid and retained heap is 1.047×, within the lab's
5% regression gate. A secondary lane keeps identical `babel-preset-solid` DOM
compilation and swaps only the reactive core (Brotli −10.7%). Fairness gates and size tables live in the lab
`artifacts/size-report.md`. This is partial implementation evidence, not full
Solid compatibility.
The real-application matrix in `benchmarks/scenarios` adds login-risk,
animation-timeline, and geometry-hit-test workloads plus a property-boundary
stress case. Each application compares Vite 8 unminified, Vite/Oxc, Vite with
Terser private-property mangling, Closure ADVANCED, LilScript unmangled,
LilScript public-safe, LilScript closed-world, and LilScript followed by Vite.
All application lanes also match C/native output. The property stress case
keeps objects alive behind a key-insensitive host boundary: LilScript property
mangling changes `155/143/107` to `105/117/90` raw/gzip/Brotli bytes.
The maintained implementation and research backlog is in
[docs/roadmap.md](docs/roadmap.md).
Motion's audited compatibility gate is in
[docs/motion-compatibility.md](docs/motion-compatibility.md).

## v0.1 Scope

The implemented v0.1 language includes primitive and nominal types, arrays,
typed maps and sets, fixed-length array/shared buffers and unsigned byte views,
functions and closures, structs, classes and constructors, static modules,
checked purity contracts, control flow, compound assignment, templates,
inferred generic functions and classes, nullable `T?` values with direct
null-guard narrowing, first-class `A | B` unions with tagged native lowering,
portable `value is Type` union-member guards with branch narrowing,
explicit host `extern` declarations,
and the standard methods listed in the language contract. Package management,
lockfiles, typed lazy module loading, and runtime chunks are project-delivery
capabilities. Generic struct literals, exceptions, async functions, and a
direct machine-code backend are outside v0.1; native
executables currently use optimized C as the final lowering stage.
