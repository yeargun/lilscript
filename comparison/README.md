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
Raw UTF-8, gzip level 9, and Brotli quality 11 sizes are recorded. These cases
test specific language and optimization categories; they are evidence over the
listed corpus, not a claim about every possible JavaScript program.
