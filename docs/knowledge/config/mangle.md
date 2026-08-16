# `[mangle]`

Parent: [Config](README.md). Language: [aggregates](../language/aggregates.md), [boundaries](../language/boundaries-escape.md). Emission: [JavaScript emission](../compilation/javascript-emission.md).

Highest precedence for four flags. Each is `Option<bool>`: unset → derived from `javascript.compression` / priority.

| Key | Unset means | Never mangles |
|---|---|---|
| `identifiers` | `identifier-mangling` | reserved externs, referenced globals |
| `properties` | `property-mangling` (size-first default on) | `extern class` members; public named aggregate fields unless exports mangled |
| `exports` | `export-mangling` (priority never enables) | — when true, public ESM names **and** public aggregate fields may shorten |
| `pool_strings` | `string-pooling` | — |

`mangle.exports = true` is for LilScript-only apps whose static imports are linked before codegen. Reusable packages (Solid open-world, jQuery `<script>` facade) keep `exports = false`.

jQuery dual configs: `lilscript.toml` / `lilscript.public.toml` keep export names; `lilscript.app.toml` mangles exports for a closed LilScript app. See [jQuery](../evidence/jquery.md).

Identifier **alphabet** and **layout** are not `[mangle]` keys; they are compression/optimization search (`entropy-aware-mangling`, `function-layout-variants`, `local_name_reserve`).
