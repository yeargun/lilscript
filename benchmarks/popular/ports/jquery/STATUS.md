# jQuery → LilScript port status

Compiler/language perspective for this port:
[docs/knowledge/evidence/jquery.md](../../../../docs/knowledge/evidence/jquery.md).
Live work: [jquery-01](../../../../docs/knowledge/migration/board/notes/jquery-01.md).

## Port philosophy (LilScript-native)

Behavior parity with jQuery 3.7.1 matters. **JS object-shape fidelity does not.**

Prefer LilScript forms that codegen can actually optimize:

- **structs** (default positional layout → compact arrays) for tokens, tween
  state, event handles, caches — not `createEmptyObject()` bags with string keys
- **typed locals / scalar fields** over property maps when the shape is fixed
- **mangled exports/properties** for LilScript-app builds where the whole program
  is linked before codegen

Changing an internal API to the LilScript way is encouraged when it improves
size, mangling, or runtime. Do not cargo-cult Sizzle/jQuery record layouts.

### Dual delivery surfaces

| Surface | Config | Contract |
| --- | --- | --- |
| LilScript app | `lilscript.app.toml` (`exports=true`, `properties=true`) | Internals fully LilScript-native; whole-program mangle OK — even the jQuery API surface may mangle when the app is all LilScript |
| Classic `<script src>` + inline JS | `lilscript.toml` (`exports=false`, `properties=true`) | Thin **facade** keeps `jQuery` / `$` / documented public names; eligible LilScript-owned internal fields may still mangle |

Public names are a **config choice** for compatibility, not an internal shape requirement. `exports=false` protects the public export/aggregate surface; it does not disable internal property mangling. Prefer zero `JsValue` bags on hot paths; keep string-key facades only at the JS boundary.

Build: `node benchmarks/popular/build-jquery.mjs [public|app|all]`

## Coverage

LilScript ports of jQuery **3.7.1** modules under `ports/jquery/`, wired like
upstream `src/jquery.js` via `entry.lil`.

## Size reality check

Full-library compression claims require matching public behavior and boundary,
then an independently compiled LilScript artifact for every claimed raw, gzip-9,
or Brotli-11 objective, compared only on its matching metric against the best
eligible baseline. The current generated public-library report
(`benchmarks/popular/build/jquery-results.json`) starts from one Brotli-oriented
LilScript compiler output. Only that compiler output's Brotli measurement is
objective-aligned; raw/gzip and every downstream transformation are diagnostics:

| Artifact | Raw | Gzip-9 | Brotli-11 |
| --- | ---: | ---: | ---: |
| npm `jquery.min.js` | 87533 | 30342 | 27445 |
| LilScript compiler output (single selected artifact) | 162971 | 44309 | 38560 |
| Downstream esbuild diagnostic (same LilScript output) | 99709 | 36653 | 32786 |
| Downstream Terser diagnostic (same LilScript output, `passes=3`) | 98176 | 35487 | 31645 |

Those two downstream rows are attribution probes, not alternative LilScript
candidates. A LilScript compilation publishes one artifact selected internally for
its configured objective; it does not ask Terser or Oxc to choose or finish that
artifact. Any useful downstream rewrite must be generalized in the compiler and
compete there in the same invocation.

The report marks `eligible=false`, `sizeGate=false`, and `exactSurface=false`.
Closure ADVANCED and representative performance/memory gates remain open.

The table below is a **historical port-shaping log** under app/full-mangle experiments;
it is not the current public result:

| Stage | esbuild minify | terser |
| --- | ---: | ---: |
| Initial JsValue bags | ~102602 | ~98299 |
| After struct/class reshape + densify | ~100633 | ~96007 |
| typed css/each/fn* + AnimOpts | ~99450 | **~95038** |
| this/arguments methods (many fn.*) | ~1026xx | ~979xx |
| host-literal lowering (`{}` / null-this `call*`) | ~1020xx | ~981xx |
| bindMethod restore (wrap/showHide/manipulation) | ~1017xx | **~97836** |

The historical best densify checkpoint was **~95038** Terser raw. A subsequent
this-method conversion regressed that experiment; shared `bindMethod`+arrow variants
were used to investigate the walk-back. Rerun attribution scripts before treating any
historical stage as the current compiler/port baseline.

Compiler known-host lowering now covers (see
`lower_known_js_host_calls` / `known-js-host-literal-lowering`):

- plain/null-prototype object and array construction;
- JS type/string/nullish/undefined/property operations;
- `callN`/`apply`, array/string methods, selected DOM fields/methods, regex, and
  throw helpers;
- direct builtin spelling for eligible low-use wrappers (limit 4), otherwise a
  shared alias when call/value use makes that preferable.

The historical downstream Terser diagnostic is still above npm in every measured
size column.
Remaining `bindMethod*` / `call*` / `stringify` bridges and dual class+facade layers on
Deferred/Callbacks/jqXHR are investigation targets, not a complete attribution.
Prefer bindMethod+typed helpers over this-methods unless a complete-artifact ablation
shows the alternative wins.

`record{}` is **null-proto** and breaks jQuery — use `createEmptyObject()` for
plain `{}` (compiler lowers it).

## Selector engine

LilScript-native engine in `selector.lil`:

- **structs** `Pseudo` / `CompoundParts` (positional layout) instead of string-key bags
- QSA / `matches` fast path for plain CSS
- custom pseudo chains as post-filters
- combinators *after* custom pseudos recurse from matched seeds via scoped QSA
  (same idea as Sizzle `setMatcher`, LilScript shape)

Public facade still exposes `jQuery.find` / `jQuery.expr` for plugins and for
`css/hiddenVisibleSelectors.lil` / `effects/animatedSelector.lil`.

`sizzle-host.ts` remains only as an optional upstream reference; `entry.lil`
does not install it. `selector-native.lil` is the optional native-only stub.

## Verification

- `verify-jquery.mjs` — core / deferred / data / queue / attributes / events
- `verify-jquery-selector.mjs` — differential vs npm jquery
- `verify-jquery-upstream-selector.mjs` — harder upstream-style cases
- `verify-jquery-traversing.mjs`
- `verify-jquery-ajax.mjs`
- `verify-jquery-effects.mjs`
- `verify-jquery-global.mjs` — script-tag `window.jQuery` / `$` / `noConflict`
