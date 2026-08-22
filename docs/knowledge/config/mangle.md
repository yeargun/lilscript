# `[mangle]`

Parent: [Config](README.md). Language: [aggregates](../language/aggregates.md), [boundaries](../language/boundaries-escape.md). Emission: [JavaScript emission](../compilation/javascript-emission.md).

Highest precedence for these flags. Unset `identifiers` / `properties` / `exports` / `pool_strings` derive from `javascript.compression` / priority. `extern_fields` is independent: unset means **on**.

| Key | Unset means | Never mangles |
|---|---|---|
| `identifiers` | `identifier-mangling` | reserved externs, referenced globals |
| `properties` | `property-mangling` (size-first default on) | `extern class` members when `extern_fields` is on; public named aggregate fields unless exports mangled |
| `exports` | `export-mangling` (priority never enables) | — when true, public ESM names **and** public aggregate fields may shorten |
| `extern_fields` | **on** (library ABI) | `extern class` member spellings. `false` is closed-world only: those names mangle. Host members such as `string.length` stay exact. |
| `pool_strings` | `string-pooling` | — |

`mangle.exports = true` is for LilScript-only apps whose static imports are linked before codegen. Reusable packages (Solid open-world, jQuery `<script>` facade) keep `exports = false`.

`mangle.extern_fields = false` is the matching switch for `extern class` members that exist only because JavaScript callers read them (`gfm`, `parse`, …). A program written entirely in LilScript does not need those spellings and can turn the pin off.

jQuery dual configs: `lilscript.toml` / `lilscript.public.toml` keep export names; `lilscript.app.toml` mangles exports for a closed LilScript app. See [jQuery](../evidence/jquery.md).

Identifier **alphabet** and **layout** are not `[mangle]` keys; they are compression/optimization search (`entropy-aware-mangling`, `function-layout-variants`, `local_name_reserve`).
