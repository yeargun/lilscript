# Build profiles and reproducible recipes

Parent: [config](README.md). Tradeoffs: [tradeoff matrix](tradeoffs.md).
These are intent recipes, not new named presets in the schema.

## Development

Use CLI `--mode development`. It forces multi-IR/emission candidate expansion off even
when TOML says `always`; parsing, types, mandatory normalization, the configured
optimizer, and independently enabled finalization features such as parsed peephole
still run. Lilpack dev uses this path.

## Correctness/oracle

Set `strip_console=false`, name the target, and keep a checked config file. A fast
oracle may use search off, but it does not prove release compression. Portable cases
use `--target all` and compare JS/C/native results.

## Default transfer release

Use `priority=size-first`, the actually served `cost_model`, production search, and a
recorded byte/beam/count budget. Production caps the effective candidate count at 384
even if `candidate_limit` is larger. Gzip and Brotli releases should be separate rows
when both transports matter.

## Maximum compression experiment

Use `candidate_search=always`, raise byte/count/beam limits deliberately, and record
compile time. `always` removes the production 384 cap but still obeys
`candidate_limit`, byte budget, exact allowlists, hard-offs, and the CLI development
override.

## Reusable library vs closed app

Reusable: `mangle.exports=false`, named public ABI, and public function
constructibility matching the API. Closed app: export mangling/opaque positional
handles may be enabled only when every consumer is linked. Never compare these rows as
the same boundary.

## Profile-guided build

Generate stable keys with `--profile-template`; load version-1 JSON plus optional
inline counters. `[optimization].profile_guided` and the JS/native feature gate must
both allow it. Inline counters override file data. Record the profile hash because it
is an input to emitted bytes and runtime shape.
