# Monaco Editor → LilScript port status

Pinned to **monaco-editor 0.56.0** / **monaco-editor-core 0.56.0**.
VS Code source commit: `f487add297079a02eb836810185b165e50cadabc`.

## Port philosophy

Behavior and public `monaco.d.ts` API parity. Not VS Code object-shape or DI fidelity.

Keep the algorithms: piece tree + red-black metadata, prefix-sum line starts, decoration interval tree, Monarch lexer, Myers diff.

Rewrite representation as LilScript `struct` / `class` / enum. `JsValue` only at the host/DOM facade and `monaco.editor.create(options)` bag.

TypeScript compiler / `ts.worker` is out of scope.

Read/implement from `benchmarks/popular/vendor/` (gitignored). Refresh with `node monaco-layers/clone-vendor.mjs`. Measure/oracle against npm `monaco-editor-core@0.56.0` and `monaco-editor@0.56.0`.

Findings page with `lilscript-codec` tables: [`apps/monaco/findings.html`](../../apps/monaco/findings.html).
Paired demos: [`apps/monaco/lil`](../../apps/monaco/lil) and [`apps/monaco/js`](../../apps/monaco/js) (`node monaco-layers/build-apps.mjs`).

## Dual delivery

| Surface | Config | Contract |
| --- | --- | --- |
| Public ESM | `lilscript.toml` (`exports=false`, `properties=true`) | Stable exported function names; methods mangle; Position/Range fields kept |
| Closed app | `lilscript.app.toml` (`exports=true`) | Whole-program mangle for the integrated size row. The openable demo keeps public export names so `browser.js` can call `runDemo`. |

## Layers

Measured with `lilscript-codec` (stock zlib 1.3.1 / official Brotli C 1.1.0). Gate is Brotli-11 vs the best eligible JS minifier (esbuild / Terser / Oxc) of a matching monaco-editor-core extract.

| Layer | Verify | Brotli lil | Brotli JS | Gate |
| --- | ---: | ---: | ---: | --- |
| base-lifecycle | ok | 807 | 17,736 | PASS |
| core-types | ok | 748 | 1,587 | PASS |
| piece-tree | ok (400 random edits) | 4,047 | 29,445 | PASS |
| text-model | ok | 5,964 | 31,374 | PASS |
| view-render | ok (jsdom) | 7,934 | 212,866 | PASS |
| input-commands | ok | 11,340 | 213,030 | PASS |
| standalone-api | ok | 13,224 | 526,056 | PASS |
| monarch-popular | ok | 2,906 | 529,604 | PASS |
| popular-contrib | ok | 14,402 | 525,999 | PASS |
| remaining-contrib | ok | 6,976 | 526,079 | PASS |
| remaining-monarch | ok (75 langs) | 12,387 | 597,336 | PASS |
| json-css-html-ls | ok (no tsc) | 1,748 | 525,981 | PASS |

## Extract fairness

- **base-lifecycle, core-types, piece-tree, text-model** compare against targeted monaco-editor-core ESM (URI/lifecycle, Position/Range/Selection, `PieceTreeTextBuffer`, interval tree + search). These are the honest algorithm rows.
- **view-render / input-commands** extract monaco `editor/browser/view.js`, which pulls a large view graph. LilScript is a viewport + textarea + minimap canvas, not GPU/view-zones/white-space rendering.
- **standalone-api and later contrib/LS rows** extract `editor.api.js` (or language modules that import it for `IndentAction`). That kitchen-sink surface is what npm ships for `monaco.editor.create`; the LilScript side is the functioning subset (create, type, undo, theme, layout, find, fold, brackets, hover, suggest, snippets, comments, goto, Myers diff, Monarch, JSON/CSS/HTML adapters). Size deltas there overstate a full-product replacement.
- CSS and TTF are not counted. Editor workers are not ported; tokenize on the main thread.

## Compiler notes

- `createElement` known-host lowering emitted an unbound call in large compiles; the port uses `domCreateElement`.
- `candidate_search = "off"` for compile time.
- View/input oracles are jsdom snapshots (line count, scrollHeight, visible range, theme, typed value). Playwright is not a lab dependency.
- SSA coalescing in large compiles aliases `this`/params with constructed objects and drops `let` for inlined locals. Piece-tree `rbDelete`, `nodeAt`/`nodeAt2`, `getOffsetAt`/`getPositionAt`, decoration id lists, and Myers line diff run in `js-host.ts` so those paths do not depend on the broken lowering. String accumulators use `emptyBuf()` / `concat2()` instead of `""` + `+=`.
- Extra-cursor typing is a separate `replaceExtraCursors` pass, not folded into `replaceSelection`.

## Verified

- `node monaco-layers/verify-lil.mjs` — all 12 layers, including piece-tree vs monaco-editor-core (seed 42 × 400 edits).
- `node monaco-layers/build-apps.mjs` — demo log `value=70 matches=2 folds=1 highlights=2 hover=msg diffs=1`.
- `bindMonaco(entry.lil)` — `editor.create`, type, undo, comment, find, decorations, suggest provider, extra-cursor type, `createDiffEditor` line changes, 83 language ids.

## Non-goals

- Microsoft TypeScript compiler / `ts.worker`
- VS Code workbench
- TextMate / Oniguruma grammars
- Full language-service feature parity for JSON/CSS/HTML (adapters only)
