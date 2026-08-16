# Popular library compression lab

Fair size comparison of version-pinned npm packages against LilScript ports.
Exact selected entrypoints are separated from behavior- or feature-incomplete
research rows. The goal is to measure LilScript's compression effectiveness,
not to claim a smaller replacement from a hand-selected subset.

## Columns

| Column | Meaning |
| --- | --- |
| Raw JS | Unminified published / bundled source bytes used by the app |
| Terser | esbuild-bundled package, then Terser minify (raw / gzip-9 / brotli-11) |
| Closure | Same bundle under Closure, with the actual `ADVANCED` or `SIMPLE` level printed per row |
| LilScript | LilScript port under size-first / Brotli cost model; raw and gzip are diagnostics |
| Brotli | Brotli-11 of the selected production artifact (also reported per toolchain) |

Every eligible row must pass the selected published entrypoint's behavior
contract with the same algorithmic work, dense differential vectors, matching
npm vs LilScript app stdout, and representative throughput and retained-memory
gates. Adapter bytes are always counted. The configured transport codec must be
no larger than either npm/Vite 8 or public-API-preserving Closure ADVANCED in
that matching metric. The current objective is Brotli; raw and gzip are reported
as diagnostics and may lose because choosing one exact objective can trade bytes
in the others. C/native execution is extra
portability evidence where that API can be represented; browser-only host APIs
are judged at their JavaScript boundary.

## Published exact-entrypoint corpus

| Project | Status |
| --- | --- |
| Nano ID | Exact `index.browser.js` entrypoint; pooled Node entrypoint excluded |
| mitt | Exact root entrypoint |
| clsx | Exact root entrypoint; raw recursive JavaScript-value walk and default/named identity |
| gl-matrix | Exact ESM root entrypoint: common live bindings and every module export/alias |

Candidate selected-surface rows (not eligibility wins):

| Project | Status |
| --- | --- |
| motion | Selected `mix`/`wrap`/`stagger`/`spring` surface; openable DOM fixtures on `/libraries.html#motion-lab-examples` |

The clsx lane uses LilScript's JavaScript-only `JsValue` boundary, direct dynamic
indexing, truthiness tests, and direct `for-in` lowering. It therefore performs
the upstream recursive walk without an intermediate conversion tree; its 10,000
input differential suite includes inherited enumerable keys and ignored bigint,
Symbol, and function values.

The Solid/solidlil LSX todolist remains a separately labeled application benchmark.
Acorn, Preact, Redux Toolkit, Immer, and Zod ports are retained as implementation
backlog, but no library-size claim is published for them until their selected public
entrypoints are complete and pass the same behavior, size, performance, and memory gates.
`audit-surfaces.mjs` pins that backlog to installed runtime inventories: Acorn's
root has 22 exports; Preact publishes 14 runtime entrypoint patterns and its
audited root/hooks entrypoints each have 12 exports; Redux Toolkit's root has 59
exports across a four-entrypoint package; Immer's root has 19; and Zod's root has
109 across eight runtime entrypoint patterns. Sorted names and SHA-256 digests
are checked in and re-imported by the audit test.

Closure ADVANCED receives generated extern declarations for every observable
published property in the mitt and gl-matrix lanes. This permits closed-world
optimization while preventing Closure from winning by renaming an API that both
the npm package and LilScript must retain.

## Run

```sh
cd benchmarks/popular
npm install
npm run benchmark
```

The benchmark command regenerates size, performance, retained-memory, Markdown,
JSON, and website artifacts. Node implementations run in alternating isolated
processes with identical workloads and checksums. LilScript runtime uses the
same explicitly configured Brotli-objective modules as the publication size
lane; raw and gzip diagnostics never choose a runtime artifact.
