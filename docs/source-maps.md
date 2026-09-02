# JavaScript Source Maps

LilScript can emit a standard Source Map v3 for the JavaScript artifact that
actually won whole-program optimization and codec scoring. With the default
`sourcesContent` policy, browser developer tools and error services can show the
exact authored `.lil` files even when the shipped JavaScript has been heavily
rewritten and mangled.

Source maps explain generated-to-authored locations and original names. To
audit *why* the compiler preserved, shortened, coalesced, or exported a name,
enable the separate [analysis map](analysis-maps.md). The two artifacts share
the winner replay and final provenance composition when both are enabled.

Source maps are opt-in:

```toml
[javascript.source_map]
enabled = true
mode = "hidden" # hidden | linked | inline
include_sources_content = true
```

`enabled = false` is the default. In that state the compiler skips provenance
capture, token composition, and map serialization entirely; the emitter's
recording hooks are one `Option` check each, and the selected JavaScript is
byte-for-byte what the same compiler produces with the map on. Enabled, the
map costs one traced emission of the winner (none when the candidate search
is off, where the single emission is traced as it happens), one token
alignment of that emission against the final JavaScript, and the
serialization: markedlil at level 15 takes 37.5 s with a hidden map against
36.2 s without, on a 16-core worker; acorn with the search off, 300 ms
against 273 ms. See [analysis maps](analysis-maps.md#performance-model) for
the timing buckets.

## Publication modes

| Mode | JavaScript bytes | Map publication | Intended use |
|---|---|---|---|
| `hidden` | **Exactly identical** to the map-disabled selected artifact | `<output>.map`, with no URL in JavaScript | production error reporting with zero served-code overhead |
| `linked` | Adds one `//# sourceMappingURL=...` line | `<output>.map` | browser debugging with an ordinary sidecar |
| `inline` | Adds a base64 data URL containing the complete map | no sidecar | local or self-contained debugging |

For example, `-o dist/app.js` produces `dist/app.js.map` in hidden or linked
mode. Hidden and linked modes require an output path because stdout has nowhere
to put a sidecar. Inline mode also works on stdout. Split and preserve-modules
builds produce one map per JavaScript chunk.

Lilpack receives the compiler map directly through its Vite transform hook, so
Vite can compose LilScript locations through later bundling and minification.
For a production Lilpack build, enable this compiler option to retain authored
LilScript locations and pass `--sourcemap` to `lilpack build` when the final
Vite bundle should publish maps. Lilpack owns the final map filename and
publication style; the compiler's `mode` applies to direct `lilscript` output.

Inline maps substantially enlarge JavaScript and are not a zero-overhead
production mode. Hidden maps add build-time work and a separate file only; they
do not add runtime, parse, download, or JavaScript bytes unless a deployment
chooses to serve the `.map` itself.

## What the map means after optimization

LilScript does not merely reprint source. It can inline functions, specialize
calls, scalar-replace objects, turn fields into numeric slots, pool strings,
coalesce SSA values into one JavaScript binding, and delete dead code. The map
therefore follows these rules:

- Each surviving generated token maps to the closest precise LilScript span
  retained through lowering and final JavaScript rewrites.
- Inlined code maps back to the original function/body that supplied it.
- A construct removed by optimization has no generated position. Its original
  text is still available through `sourcesContent` when that option is enabled.
- If several non-overlapping SSA values share one generated binding, locations
  remain useful but the JavaScript binding has one primary original-name entry.
- Generated glue with no independent source construct inherits the nearest
  meaningful source location.

Mappings use UTF-16 columns as required by Source Map v3. Source paths are
relative to the project/config root when possible.

## Mangling and original names

“Mangling” covers several distinct transformations in LilScript:

- **Identifier mangling** shortens private functions, globals, parameters, and
  locals while preserving lexical binding and capture behavior.
- **Property mangling** shortens eligible LilScript-owned fields. Host members,
  public named fields, and other ABI-pinned keys remain stable unless the
  project explicitly selects a closed contract.
- **Export mangling** is a separate opt-in that may shorten public ESM names.
- **Representation elimination is not mangling**: a class or property removed
  by scalar replacement has no corresponding JavaScript name to recover.

The standard `names` table carries retained original identifiers and property
names, so ordinary source-map consumers can display them. LilScript also writes
an additive `x_lilscript` object. Its `mangledNames` records make the relationship
auditable without decoding VLQ mappings: each record contains the generated and
original spelling, source location, category (`function`, `global`, `parameter`,
`local`, `temporary`, or `property`), and occurrence count. Consumers that do
not know this extension safely ignore it.

This metadata describes only the selected artifact. Source-map generation never
changes candidate ranking, ABI admission, mangling decisions, or optimizer
behavior.

LilScript bundle-manifest sizes, hashes, and objective scores likewise describe
that selected JavaScript. A linked or inline publication comment is debugging
metadata added afterward and is not fed back into optimizer scoring; hidden mode
adds no such bytes.

## Rust API

Artifact-returning compiler APIs expose `Option<JavaScriptSourceMap>` alongside
the selected JavaScript: `compile_path_explained_configured`,
`compile_path_to_js_module_explained_configured`, `compile_path_all_configured`,
and the bundle APIs. `JavaScriptSourceMap::as_str()` returns the Source Map v3
JSON; summary accessors report source, mapping, and original-name counts. The
older string-only compilation functions continue to return only JavaScript and
therefore skip map capture even if passed an enabled map configuration.

## Source privacy

`include_sources_content = true` is the most useful default: the map contains
the exact LilScript files and remains usable when source files are unavailable
on the deployed machine. A public `.map` then also publishes that source.
Set it to `false` when maps go to a private error service that uploads sources
separately, or when source disclosure is unacceptable. Paths and mappings remain
in the map, but a consumer must obtain the matching source tree to display code.
