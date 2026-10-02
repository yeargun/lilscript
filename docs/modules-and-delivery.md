# Modules and Delivery

Reasoning: [knowledge/language/modules-lazy.md](knowledge/language/modules-lazy.md), [knowledge/delivery](knowledge/delivery/README.md). Keys: [configuration.md](configuration.md#delivery-delivery). The deleted route's chunk planner is in [history](knowledge/history/compilation/chunk-planning.md). This page is the delivery contract.

LilScript checks a closed, typed module graph as one program before any
optimization. Static imports remain the default because they permit cross-file
inlining, scalar replacement, and complete tree shaking without a runtime
loader.

## Foreign JavaScript and TypeScript

LilScript stays at the root of a mixed application. A foreign ESM binding is
declared separately from its type contract:

```lilscript
import extern { add as hostAdd } from "./host.ts";
extern int hostAdd(int left, int right);

print(hostAdd(20, 22));
```

`import extern` is resolved and owned by the LilScript module loader. Every
local import requires a matching `extern` function, global, or class in that
module. This keeps the program's host boundary explicit: foreign calls
remain effectful unless an allowed `pure extern` contract says otherwise, host
names stay exact, and native targets reject the JavaScript-only edge.
Side-effect-only modules use `import extern "./setup.ts";` and need no binding
contract.

Relative `.js`, `.mjs`, `.ts`, `.mts`, `.jsx`, and `.tsx` imports are supported.
By default (`delivery.host_modules = "external"`) the compiler validates local
sources and emits the original specifier as a native ESM edge. With `"auto"` or
`"embed"` the relative host modules travel with the output: the compiler parses
them with Oxc and lowers the supported subset into typed host units. Other
supported module bodies retain their parsed JavaScript with unknown effects. It does not
type-check TypeScript or pretend that type erasure is sufficient for the full
language. Bare specifiers remain package edges.

Lilpack owns the complete application graph. Its integrated Vite engine resolves
foreign static and dynamic imports, npm packages, TypeScript, JSX/TSX, CSS,
JSON, WebAssembly, workers, URLs, and assets using Vite's normal semantics. In a
production build Vite tree-shakes and chunks that graph after LilScript has
finished checking, whole-program optimization and codec-scored selection of the
`.lil` side. The final hashed assets and `lilpack.manifest.json` are Lilpack output.

Running `lilscript --target js-module` directly intentionally leaves foreign
ESM edges in its output. Use Lilpack when a deployable mixed-language graph is
required.

## Lilpack development server

The application development loop is exposed by Lilpack:

```sh
lilpack dev src/main.lil --root . --port 5173
```

Lilpack compiles the Lilscript entry as reusable ESM and registers every
compiler-discovered `.lil`, config and lock input with Vite's watcher.
Vite owns its transform cache, TypeScript and asset transforms, dependency
optimizer, WebSocket update channel, browser client, and compile-error overlay.
Production compressor candidate search is disabled in development.

Vite invalidates the compiled entry when any linked `.lil` input changes. If the
entry exports `hotAccept`, Lilpack installs a self-accepting boundary and invokes
that function on the new module; exported `hotDispose` runs on the old module
first. Both hooks currently take no parameters and return `void`. Without the
contract, Vite uses its normal propagation and page-reload fallback. JS/TS and
asset updates retain Vite's standard HMR behavior.

## Dynamic modules

`import()` accepts a compile-time string specifier and returns a typed
`Task<module>` value. It is not an untyped JavaScript `Promise` boundary. The
module namespace exposes the target module's runtime exports with their declared
LilScript signatures, and unused namespace properties are not retained as chunk
exports.

```lilscript
import("./feature")
  .then((auto feature) => print(feature.answer(40)))
  .catch((auto error) => print(`${error.specifier}: ${error.message}`));
```

`Task<T>` provides `then`, `catch`, and `finally`. A `then` callback receives
`T`; a `catch` callback receives a general `JsValue` because async throws and
`Task.reject` may reject with any non-void JavaScript value. The nullable
`specifier` and `message` fields are available without assuming they exist.
Failures synthesized by Lilpack's dynamic loader populate both fields. Promise
flattening is reflected in the result type.
`auto` is permitted for arrow parameters only when the callback has a contextual
type.

Dynamic specifiers must be string literals. This keeps the graph deterministic,
lets the type checker load the exact export interface, and prevents a runtime
filesystem search. Dynamic and static import cycles are legal. Static cycles
retain live bindings and once-only initialization; eager reads before a binding
is initialized remain errors.

Lazy modules may contain top-level executable statements and variables. Their
static dependencies initialize in source order on the first load; later loads
share live bindings, namespace identity and the cached initialization outcome.
`single` includes their code in the entry artifact, with suspended lexical
activations for initialization. `split` and `preserve-modules` use separate files.
Single-file entries retain independent instances; split entries share them.
A function in a static cycle is available during instantiation, while reading a
lexical value before its initializer still throws.

Dynamic module tasks are JavaScript-only. Native targets report a source
diagnostic because LilScript does not claim that a JavaScript chunk has a
portable C ABI.

## Delivery

One program has one or more **entries** (`[delivery.entries]`, `--entry
NAME=PATH`, or `INPUT`), each a root of one module graph and a public surface of
its own. The compiler checks and optimizes the whole program, every entry
together, then places its root statements in files once, before naming
(plan M3.3). Each root statement carries a row formation writes from the
program's effect facts: a *definition* only creates its bindings and may move
to where its readers need it; everything else is *anchored* to its module's
evaluation. Placement never recomputes these from JavaScript syntax.

### Modes

- **`single`** (the default): one file per entry. With one entry this is the
  one-file output; `import()` builds the namespace in place, a turn later. With
  several, each entry's file is its own instance of everything it runs.
- **`split`**: each entry's file is a facade that imports what the entry runs,
  in its own order, and re-exports its names; code several entries load lives
  in files they share, and a module that some entry loads only with `import()`
  gets a lazily loaded file. Merges are kept only when a simulation of every
  entry's ES module evaluation, and of every lazy load after it, reproduces the
  sources' order.
- **`preserve-modules`**: a file per source module, importing what its module
  imports. An entry module another module imports keeps an internal file (named
  like a chunk) behind its entry file.

A write to a binding another file declares goes through a setter that file
exports (`let s=v=>x=v`): ES imports are read-only.

### The contract

| Clause | Rule |
|---|---|
| DL1 Instances | In `split` and `preserve-modules`, every root binding has one instance per realm, whichever entries load and in whatever order. `single` with several entries gives each entry file its own instance |
| DL2 Order | For every load sequence, effects, throws and initializations run in the sources' ES module order, restricted to the modules not yet evaluated |
| DL3 Closure | Loading an entry runs exactly the effects of the modules it reaches statically that have not run yet |
| DL4 Initialization | A read before initialization throws where the source would. No file import cycle exists except the image of a source cycle |
| DL5 Lazy loads | `import()` loads its target's files and evaluates what the target reaches and nothing has evaluated yet, then resolves with its namespace |
| DL6 Public surface | Each entry exports exactly its declared names; one cell exported by several entries or under several names is one identity |
| DL7 Frames | The format is a container only; a module-frame program keeps strict semantics |
| DL8 preserve-modules | A statement never leaves its module's file; a write to another file's binding goes through a setter |
| DL9 Complete delivery | Scored bytes are delivered bytes: files, file names inside specifiers, the preload prelude |
| DL10 Determinism | Bytes, names and manifest depend only on sources, configuration and binary: never on `-j`, map order, absolute paths or the order of TOML tables or `--entry` flags |

The plan verifier (design §7.11, P1–P10) checks every plan before its files are
printed; a failure is an internal error that names the assertion.

### Target limits

- Effects in lazy modules on a target without dynamic-import syntax need
  `delivery.mode = "single"`; that mode uses the in-file module runtime.
- Typed embedded host modules participate in the source dependency graph,
  including separate entry reachability, mixed import order and lazy activation.
  Carried host text outside typed lowering has compiler-written ESM/CJS module
  files in split/preserved output. Static single-file output keeps those bodies
  at their graph positions, sharing each dependency's identity and live state.
  Each entry carries only its reachable host closure. Opaque lazy modules require
  split/preserved output; typed hosts also support single-file lazy activation.
- External module requests retain loading and named-export validation even when
  their local bindings are unused. Split/preserved delivery uses separate
  request files to preserve their place in the dependency order.
- A single-file static import necessarily loads before that file's body. If a
  foreign request belongs only to a lazy closure, or must follow a source
  initializer, single-file delivery diagnoses the mismatch. Use split/preserved
  delivery, or embed a supported static host module. These cases
  never silently turn lazy effects into eager effects.
- Native dynamic module tasks and multiple native entries have no supported
  portable module ABI yet; they receive target diagnostics.

### Files, names and the manifest

Entry files are named by `entry_names`, module files by `module_names`, every
other file by `chunk_names`. `-o FILE` writes a one-entry delivery with FILE as
the entry file and the other files beside it; `--out-dir DIR` writes any
delivery, and `lilscript.manifest.json` there (`<stem>.manifest.json` beside
`-o FILE`). A rebuild removes the regular files the previous manifest listed
and this build does not write, inside the output directory only.

Manifest version 3 reads the sizes the objective measured on the delivered
bytes and never encodes again. Per output it lists every entry (file, the files
it loads in evaluation order, its row: the objective's bytes of what it loads
plus the declared deployment costs) and every file (role, label as entry names,
source modules, whether loading it runs code, static and dynamic imports, bytes,
objective-codec bytes, SHA-256). `side_effects` lists the files whose loading
runs code, ready for `package.json`. The search ranks by the sum of rows, and
the terminal stage keeps a challenger only when that sum shrinks and no entry's
row grows.

Several `[objective] codecs` require `--out-dir DIR`, with no `-o FILE`.
Each objective's complete delivery lives under its `raw`, `gzip` or `brotli`
subdirectory. The compiler optimizes each independently; all JavaScript bytes
and relative imports remain exactly as judged. These are alternative JavaScript
trees, not precompressed files. A single-objective build keeps the layout and
version 3 manifest described above.

A combined manifest uses version 4. Its `codecs` list is in raw/gzip/Brotli
order, and each `outputs` item retains the version 3 fields plus `codec`,
`codec_settings` and `policy_fingerprint`. File, entry, closure, static/dynamic
import and side-effect paths are relative to the root output directory and
include the codec prefix; logical labels and source-module names are unchanged.
Each file hash and measured size still describes that file's exact bytes.
Rebuilding with a different objective set, including returning to one objective,
removes only obsolete regular files listed in the previous manifest. Unrelated
files remain in place. `--target all` additionally writes one C file and native
executable under `native`; those are not JavaScript manifest entries.

### Several formats from one checked input

Use additional outputs when a package needs ESM, CJS and a browser bundle. Each
output has its own delivery plan and independent codec search. Source discovery,
checking and conservative shared semantic rules run once:

```toml
[objective]
codecs = ["raw", "gzip", "brotli"]
[delivery]
mode = "split"
[delivery.entries]
full = "src/full.lil"
mini = "src/mini.lil"

[[delivery.also]]
name = "cjs"
format = "cjs"

[[delivery.also]]
name = "browser"
format = "iife"
mode = "single"
entries = ["mini"]
global = "Mini"
codecs = "brotli"
```

Build with `lilscript --target js-module --out-dir dist`. This example writes
`primary/{raw,gzip,brotli}`, `cjs/{raw,gzip,brotli}` and `browser`. A group with
one objective has no codec subdirectory. Names are unique plain path segments;
`primary` is reserved. At most eight groups, including the primary, are admitted.
The complete version 5 manifest includes output names, per-output policy/codec
identities, the checked source/input hashes, and every exact delivered file.
Cleanup follows that manifest across changes of formats, subsets and objectives.

`delivery.select` selects primary entry names; `delivery.also.entries` selects
additional ones. Empty lists select all configured roots, in name order. A
subset publishes only its chosen interfaces and executes only their reachable
initializers; lazy imports remain lazy. The complete configured source graph is
still checked, including unselected entries. The compiler retains canonical
module identities; selecting an output never rewrites source text.

Additional outputs inherit primary controls unless overridden. Allowed overrides
are `format`, `mode`, `entries`, `codecs`, `directory`, `entry_names`, `chunk_names`,
`module_names`, `global`, `global_binding`, `globals`, `external_specifiers`, `es_module_marker`,
`exports`, `default_interop`, `annotations`, `export_placement`,
`source_maps`, `sources_content`, `source_root` and `preload`. An annotation or
facade setting inherited from an ESM primary must be explicitly changed for a
format where it is unavailable. Empty `codecs` inherits the objective set; a
nonempty list has the same validation as `objective.codecs`. An explicit Rust
`ServiceOptions.objectives` override applies to every output.

`delivery.directory` optionally chooses a directory beneath `--out-dir`, and
additional outputs can override it. The default for several formats is each
output's name. Set it to `"."` to publish ESM, CJS and browser files together;
give them distinct file templates, including their shared chunks. Absolute paths
and parent traversal are rejected, and filename collisions fail before writing.
This routing changes package paths and the complete build identity, not the
relative imports or scored bytes inside an output. It adds no runtime work.

`delivery.external_specifiers` maps authored external module requests to the
requests that a consumer can resolve. For example, an ESM output can map
`provider = "./provider.mjs"`, while a CJS output maps the same source key to
`"./provider.cjs"`. Relative destinations are relative to that output's root;
the printer rebases them for files in subdirectories. Internal graph links and
embedded host code are unaffected. `delivery.globals` continues to use the
authored request as its key for IIFE/UMD global access.

Use these mappings when a package's formats have different host entry points.
The destination must provide the declared host API and effects. The default
empty map preserves authored requests. Mapping adds no runtime wrapper; its
literal bytes affect compression, policy fingerprints and cache identity.
It avoids editing emitted imports after scoring. Empty specifiers and NUL are
rejected. An additional output's map replaces the primary map when supplied.

Source assumptions, host providers, source contracts, effort, codec parameters
and hard tactic permissions belong to the shared build and cannot be weakened
by an output. All requested baselines coexist before optional search starts;
remaining work is divided across output/objective pairs under one memory limit
and deadline. More outputs save frontend work relative to separate builds but
cost their own formation/search/encoding and retain more mandatory artifacts.
No format or objective borrows another's winner. Explicit decision-lock files
apply to primary outputs; use an individual build to replay an additional output.

The Rust build API exposes `ServiceCompilation.outputs()` and
`javascript_output(name, codec)`. `javascript(codec)` remains the primary result.
Session clients call `compile_targets()` once to build the configured set.
`--print-policy` fingerprints the complete set as well as each output policy.
Whole-build caching checks and restores every requested group.

## Packages and lockfiles

Bare imports resolve through `[dependencies]`. The current package transport is
an explicit local path, which keeps resolution auditable and works for monorepos
without a registry protocol.

```toml
[dependencies]
mathkit = { path = "../mathkit", version = "^1.2", abi = 1 }
```

Each dependency has package metadata:

```toml
[package]
name = "mathkit"
version = "1.2.0"
abi = 1
entry = "src/lib.lil"
```

Generate or refresh the lockfile with:

```sh
lilscript src/main.lil --write-lock -o build/app.js
```

`lilscript.lock` is deterministic and portable. It pins the complete transitive
graph, semver versions, compiler ABI, relative source roots, entries, dependency
edges, and a SHA-256 hash over every `.lil` file plus the package manifest.
Normal compilation never rewrites it. A missing/stale lock, changed source,
version mismatch, ABI mismatch, path escape, symlink, or conflicting package
resolution is a hard error.

Dependency visibility is scoped to the importer. Root modules may import only
root dependencies, and package modules may import only dependencies declared by
that package. A transitive package therefore cannot become an undeclared,
accidental dependency.

Bare package subpaths such as `mathkit/vector` resolve inside the locked package
root. Absolute paths and package-root escapes are rejected.
