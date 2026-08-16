# Modules, lazy loading, progressive enhancement

Parent: [Language](README.md). Related: [closed world](closed-world.md), [chunk planning](../compilation/chunk-planning.md), [delivery](../delivery/README.md). Contract: [`docs/modules-and-delivery.md`](../../modules-and-delivery.md).

## Static imports are the default because they shrink code

```lilscript
import { square } from "./math";
export pure int area(int w, int h) { return w * h; }
```

Static imports are **compiler inputs**. After linking, there is one SSA module. Cross-file inlining can turn `print(square(5))` into `console.log(25)`. Emitting JS `import` wrappers by default would block that.

`bundle.mode = "single"` (default) keeps even `import("./feature")`’s **type** but lowers it to `Promise.resolve(namespace)` inside one artifact.

## Dynamic import is typed

`import("./feature")` requires a **string literal** specifier and returns `Task<module>`. The namespace has the target module’s declared export types. Unused namespace properties are not retained as chunk exports.

Lazy-only modules (reached only dynamically) must be **initialization-free**: functions/structs/classes only — no top-level statements or variables. The compiler rejects hidden eager work in a supposedly lazy file. Put startup in an exported function.

Dynamic modules are JavaScript-only. Native reports a diagnostic rather than inventing a C chunk ABI.

Lazy exports are public even before their chunk is requested. All legality analyses
treat their parameters, returns, globals, aggregate owners, and function identities
like eager ESM exports; internal callers cannot supply a narrower domain and thereby
specialize away behavior that an eventual JavaScript importer can observe.

## Bundle modes (language-visible delivery)

Whole-program optimize **first**, then partition. Config: [`[bundle]`](../config/bundle.md).

| Mode | What the language/runtime sees |
|---|---|
| `single` | One artifact. Dynamic import is still typed, but not a network boundary. |
| `preserve-modules` | One static ESM chunk per source module for movable functions. Roots and global-writers stay in the entry. Size limits do not override source identity. |
| `split` | Mandatory lazy chunks + optional shared chunks scored by deploy cost (bytes + requests + depth + preload + cache reuse). |

`preload = none | entry | all` emits deterministic `modulepreload` (inert outside browsers). Manifest v2 records hashes, transport sizes, edges, deploy cost.

## Progressive enhancement is a boundary, not a runtime

LilScript does not emit a PE framework. The language gives you:

- **typed lazy chunks** so enhancement code is not in the first byte;
- **zero-wrapper host access** so the first paint path can be tiny;
- lint `web/eager-host-access` for top-level host work that runs before a PE boundary (`[lint].providers` includes `web`);
- `hotAccept` / `hotDispose` as explicit HMR contracts (dev), not as a shipped runtime.

Manual bundling is first-class: pick `preserve-modules` or `split`, set
`min_chunk_bytes` / `max_chunks` / `shared_min_imports`, and weight `[bundle.cost]`
toward gzip/brotli. Authored lazy roots remain mandatory even below
`min_chunk_bytes`; optional eager chunks obey the size/import thresholds, and split
fails if mandatory lazy chunks alone exceed `max_chunks`.

## Foreign JS stays outside the world

```lilscript
import extern { add as hostAdd } from "./host.ts";
extern int hostAdd(int left, int right);
```

LilScript emits the specifier as native ESM and type-checks the `extern`. It does not parse TypeScript. Lilpack + Vite resolve the foreign graph. Running `lilscript --target js-module` leaves those edges in the output on purpose.

## Config

`[bundle]`, `[bundle.cost]`, JavaScript candidate search (split skips the full
single-file two-level optimizer/emission beam, but its narrower joint chunk/symbol
search can score layout/name-reserve options and preserves the winner), and
`--delegate-bundling` (Lilpack forces `bundle.mode = single` so Vite owns chunking of
the mixed graph).
