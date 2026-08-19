# marked-01 — the marked port, from wherever it actually is to green

Parent: [ledger](../LEDGER.md). Status: landed. Covers marked-01..04.

## Question

Where does the port live in this checkout, what does it score, and what is the
parse-only hero it has to beat?

## Where the port is

`/Users/yeargun/markedlil` — the `@itslil/marked` package, a separate git repository
that compiles `src/*.lil` with this checkout's compiler (`LILSCRIPT_ROOT` defaults to
`../lilscript`). It is not a subtree of this repo, which is why a repo-wide grep here
finds nothing. In this tree only these exist:

- `benchmarks/popular/ports/monaco/vs/base/common/marked/marked.lil` — 87 bytes, a
  path-string stub for the vs-tree generator, not the port.
- `benchmarks/popular/vendor/vscode/src/vs/base/common/marked/marked.js` — 91,675 B,
  the vendored upstream source.
- Minified JS lanes already built under
  `benchmarks/popular/build/monaco-layers/catalog/js-lanes/`.

A `grep -ril marked` over the repo (excluding `target/`, `node_modules/`, `build/`,
`vendor/`) returns only references, no `.lil` implementation. So the first action is
not "continue the port" — it is to establish the port's home in this checkout.

## The heroes to beat

Full-library marked, measured with the authority codec:

| lane | raw | gzip-9 | Brotli-11 |
|---|---:|---:|---:|
| oxc | 35,031 | 10,694 | **9,889** |
| terser | 35,605 | 10,780 | 9,933 |
| esbuild | 35,553 | 11,028 | 10,174 |

Hero per metric = the minimum of the column, so Brotli-11 must come in **under 9,889 B**.

The **parse-only** hero referenced in earlier work is not defined in-tree: no
parse-only subset of the vendored source exists as an artifact. Before it can gate
anything, the subset has to be defined as a real file and measured the same way.
Until then, quoting a parse-only number is a `diag` claim at best.

## Constraints specific to this task

- No host file. Every DOM/JS touch goes through `extern class` / `JS.*`, so the
  compiler can enforce behavior and still emit the obvious JS.
- Semantics first: 660/660 before any size claim. A smaller artifact that fails a spec
  test is not a result.
- The port is not on trial for being "written like TypeScript" — if it loses on size,
  ask first whether the `.lil` was written as glue, then fix the compiler
  ([working rules](../../README.md#working-rules), rule 3).

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-19 | JS hero sizes | `./target/debug/lilscript-codec --json benchmarks/popular/build/monaco-layers/catalog/js-lanes/base__common__marked__marked.js.{oxc,terser,esbuild}.js` | table above | gate |
| 2026-08-19 | Upstream source size | `ls -l benchmarks/popular/vendor/.../marked/marked.js` | 91,675 B | diag |
| 2026-08-19 | Port present in tree? | `grep -ril marked --exclude-dir={target,node_modules,build,vendor} .` | references only, no port | diag |
| 2026-08-19 | CLI brotli vs authority | `brotli -q 11 -c …esbuild.js \| wc -c` vs `lilscript-codec` | 10,173 vs 10,174 — 1 byte apart | diag |

## Result

| lane | raw | gzip-9 | Brotli-11 |
|---|---:|---:|---:|
| Parse-only official | 67,247 | 14,064 | 12,684 |
| Parse-only · Oxc mangle on (hero) | 37,022 | 10,930 | 10,092 |
| Parse-only · Terser mangle on | 37,725 | 11,045 | 10,138 |
| **`@itslil/marked`** | **34,015** | **10,347** | **9,318** |
| npm `marked.esm.js` (full) | 43,018 | 12,992 | 11,907 |

−8.1% raw, −5.3% gzip-9, −7.7% Brotli-11 against the smallest minified official lane,
with the license banner included in our artifact and stripped from theirs. Playwright:
45.7 ms vs 52.5 ms on the document suite (13% faster) and 62.1 ms vs 67.4 ms on the
660-case loop (8% faster) — the fastest lane measured, including the published npm
build. 660/660 spec.

## What moved the numbers

1. **The compiler fix** ([ident-01](ident-01.md)) took the port from 659/660 to 660/660.
2. **Deleting the bundler wrapper.** `dist/marked.esm.js` was esbuild bundling the
   compiled core with a hand-written JS API shim. esbuild re-printed the compiler's
   chosen declaration layout as one `var` per binding and could not improve names it
   had to preserve: 9,942 → 10,624 Brotli, a 682-byte tax for 606 bytes of shim. The
   whole public surface — `marked()` as a callable with `.parse`/`.parseInline`/
   `.setOptions`/`.options`/`.getDefaults`/`.defaults`, plus `export default` — is now
   written in `entry.lil` with `JS.method1` / `JS.method2` and an export literally
   named `default`. The compiler emits the shipped ESM.
3. **Turning candidate search on**: 10,135 → 9,318 Brotli. This is the product working
   as designed — whole artifacts scored under the configured codec — and it is also
   where [ident-05](ident-05.md) lives.

## Log

- 2026-08-19 — Port located at `/Users/yeargun/markedlil`; earlier work had already
  dropped the host file. — **LANDED**
- 2026-08-19 — `gfm.0.29.json#625` (autolink backpedal) traced to the compiler, not the
  port; fixed in [ident-01](ident-01.md). — **LANDED**
- 2026-08-19 — API surface moved from the esbuild shim into `entry.lil`; ESM is now the
  compiler's own artifact. — **LANDED**
- 2026-08-19 — `local_name_reserve` pinned at 4 because the 660-case gate proves it
  correct; 6/8/12 are smaller and broken. That is [ident-05](ident-05.md), not a size
  preference. — **LANDED**

## Next step

Keep the port green while [ident-05](ident-05.md) is fixed, then re-measure: the
reserve-8 artifact is 143 Brotli bytes smaller and should become reachable honestly.
