# Lilpack

Parent: [Delivery](README.md). Bin: `src/bin/lilpack.rs`. Bridge: `tooling/lilpack/vite-runtime.mjs`.

Lilpack is **not** a second optimizer. It launches Vite with a plugin that shells out to `lilscript`:

```
lilscript <file> --target js-module --delegate-bundling [--mode development|production]
```

`--delegate-bundling` forces `bundle.mode = single`. Vite then tree-shakes and chunks the mixed graph (TS, CSS, WASM, workers, assets) using Vite’s semantics. Production hashing and `lilpack.manifest.json` are Lilpack/Vite output, distinct from LilScript’s `<entry>.manifest.json` in `split` mode.

When `[javascript.source_map]` is enabled, the compiler returns a structured
code-and-map artifact to Lilpack instead of publishing a sidecar itself. The
Vite plugin passes that map through its transform hook, preserving authored
`.lil` locations across Vite's later transformations. Use `lilpack build
--sourcemap` to publish maps for the final production assets; the compiler's
`hidden` / `linked` / `inline` mode only controls direct `lilscript` output.

When `[javascript.analysis_map]` is `summary` or `full`, the delegated artifact
also carries the compiler analysis object. Production builds publish it below
`lilscript-analysis/<source-relative>.lilmap.json`. Its SHA-256 identifies the
LilScript-selected module code before Vite's later transforms; it does not
claim to describe final Vite chunk spellings.

## Dev vs production

| | Dev (`lilpack dev`) | Production (`lilpack build`) |
|---|---|---|
| Candidate search | off (`--mode development`) | project `lilscript.toml` |
| HMR | Vite + LilScript dependency JSON (`--print-dependencies`) | n/a |
| `hotAccept` / `hotDispose` | optional entry exports; no args, `void` | not a runtime |

Without `hotAccept`, Vite falls back to normal propagation / reload. JS/TS/asset HMR stays Vite’s.

## Why this split

Closed-world proofs need the whole `.lil` graph. Vite already understands npm and assets. Lilpack composes them without making TypeScript the application language. `import extern` is the typed seam.

Do not expect Lilpack to run LilScript’s `split` planner on `.lil` modules. If you need compiler-scored lazy chunks, compile with `lilscript --target js-module` and `bundle.mode = split` on a LilScript-only (or pre-bundled) graph.
