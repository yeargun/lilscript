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

The optimized JavaScript for this program is 112 bytes. The class is
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
compiler such as Clang. The benchmark suite requires Java, Node.js, and curl.

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

# Parse and optimize once, then produce app.js, app.c, and app
target/release/lilscript examples/full_conformance.lil --target all -o build/app
```

The native target invokes `${CC:-clang}` with C11 and `-O3`.

Compiler policy is configured in an auto-discovered `lilscript.toml`, or with
`--config path/to/lilscript.toml`. Presets and per-pass overrides control
folding, CSE, global optimization, inlining, scalar replacement, DCE, identifier
and boundary-property mangling, public-export mangling, and string pooling.
Bundle policy selects a single artifact, source-module-preserving static ESM
chunks, or size/import-limited shared chunks. See
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

When `bundle.mode` is `split` or `preserve-modules`, JavaScript output becomes a
static ESM entry plus sibling chunks and a deterministic JSON manifest. The
whole program is still optimized before partitioning. C and native output stay
single artifacts, and `--target all` can emit both forms in one invocation.

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
sandboxed iframe. The same vanilla Vite project includes `/docs.html` and
`/about.html`; it contains plain HTML, CSS, and JavaScript with no Astro files.

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
extension with `.lil` syntax highlighting, compiler diagnostics, completion,
hover documentation, snippets, bracket/comment support, and document symbols.

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
npm --prefix web run build
npm --prefix vscode-extension run package
```

`scripts/verify.sh` compares Node and native output for two conformance suites
and links a generated aggregate ABI against a C host. It also runs 50 programs
through JavaScript, emitted C, and native executables with maximum and disabled
optional optimization, for 100 matrix executions, plus a framed LSP session
through diagnostics, completion, hover, symbols, and shutdown.
`benchmarks/run.sh`
downloads the pinned Closure Compiler `v20260803`, runs `ADVANCED` compilation,
checks equivalent runtime output, and measures normalized raw, gzip-9, and
Brotli-11 bytes.

On the repository's nine current workloads LilScript is smaller in every measured
raw, gzip-9, and Brotli-11 cell. Across the corpus it totals 1,508 raw / 1,300
gzip / 1,080 Brotli bytes versus Closure at 2,357 / 1,687 / 1,382. The module
workload uses three real source modules for each compiler and measures
116 / 130 / 104 bytes for LilScript versus 122 / 134 / 108 for Closure. These are
workload-specific results, not a claim that one compiler wins for every
possible program. Full methodology and tables are in
[docs/benchmark-results.md](docs/benchmark-results.md); the pass-by-pass
responsibility mapping is in
[docs/optimization-coverage.md](docs/optimization-coverage.md).

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
lazy module loading, runtime chunks, generic struct literals, exceptions, async
execution, and a direct machine-code backend are outside v0.1; native
executables currently use optimized C as the final lowering stage.
