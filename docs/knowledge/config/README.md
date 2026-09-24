# Config

Parent: [knowledge tree](../README.md). Behavior: [compilation](../compilation/README.md).

Compiler policy lives in `lilscript.toml`. The contract is
[configuration.md](../../configuration.md): what each key means, how per-library
configuration splits into contract, objective, effort and permission, and which
old keys are retired. The key-by-key reference with defaults is
[schema.md](schema.md), generated from the source by
`node finer/tools/config-schema.mjs` (`--check` fails on drift).

A configuration is read in two steps. First the retired-key table: a key of the
deleted compiler route warns "no effect in this compiler" and is removed, or
refuses the build. Then strict reading: an unknown key or value is an error.
`lilscript <input> --print-policy` prints the resolved policy.

## Pages

| Page | Keys |
|---|---|
| [schema.md](schema.md) | Every accepted key, its type and default; the retired-key table |
| [`[bundle]`](bundle.md) | Delivery modes, chunk limits and deploy-cost weights |
| [`[package]`, `[dependencies]` and lockfiles](package-dependencies.md) | Package identity, path dependencies, `lilscript.lock` |
| [`[lint]` and `[format]`](lint-format.md) | Author constraints; no effect on emitted code |

The pages that explained the old route's knobs (`[optimization]` passes,
`javascript.priority`, the compression and optimization lists as the old route
read them, `[mangle]`, `[profile]`, `[native]`, cost model, tradeoff matrices)
are in [history](../history/README.md#configuration).
