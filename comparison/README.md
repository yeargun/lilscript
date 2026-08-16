# LilScript vs Closure comparison laboratory

Every directory under `apps/` is a self-contained program comparison. Each
program includes independent LilScript and JavaScript sources, expected stdout,
pinned compiler versions, and local build/test entry points.

Applications may contain multiple source files. `module-graph` gives both
compilers three modules and verifies transitive import resolution, cross-module
optimization, and unused-export elimination.

The JavaScript source is written for Closure `ADVANCED`: it is closed-world,
does not export internal names, avoids dynamic property access, and explicitly
normalizes integer arithmetic to LilScript's signed 32-bit semantics.

Run every program and regenerate the aggregate report:

```sh
comparison/run-all.sh
```

Paired LilScript vs Terser/Oxc/esbuild micro cases live under `cases/`. The
durable catalog is materialized into reviewable folder-per-case build artifacts,
then gated for stdout plus independent raw, gzip, and Brotli size:

```sh
node comparison/cases/run.mjs
```

Escalating multi-function algorithms with fixed runtime vectors, structural
metadata, module graphs, and Terser/Oxc/esbuild/Vite frontiers live under
`algorithms/`:

```sh
node comparison/algorithms/run.mjs
```

Ready-to-run generated JavaScript, emitted C, Closure ADVANCED output, and
macOS ARM64 native executables are checked in under `comparison/artifacts`.
Validate those snapshots without rebuilding either compiler with:

```sh
comparison/test-artifacts.sh
```

Run one program:

```sh
comparison/apps/numerical-kernel/build.sh
comparison/apps/numerical-kernel/test.sh
```

Each build produces optimized JavaScript, generated C, two native executables,
runtime output, `report.json`, and `report.md` under that application's ignored
`build/` directory. A result is accepted only when Closure JavaScript, LilScript
JavaScript, LilScript native output, and independently compiled LilScript C all match the
checked-in expected output.

The reports measure actual generated files without removing trailing bytes.
LilScript is compiled independently for raw, gzip, and Brotli; each objective
artifact is gated only on its matching metric against Closure. A Brotli-selected
artifact may be larger raw or gzip without failing the Brotli objective. These cases
test specific language and optimization categories; they are evidence over the
listed corpus, not a claim about every possible JavaScript program.
