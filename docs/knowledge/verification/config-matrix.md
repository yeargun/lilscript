# Configuration matrix

Parent: [verification](README.md). Keys: [configuration.md](../../configuration.md)
and the generated [schema](../config/schema.md). Runners: [testing.md](../../testing.md).

Which configuration axes a test suite must own, on the one compiler. The old
route's matrix (priorities, parsed-peephole finalization, the production cap of
384, the `optimizations` search families) is in
[history](../history/config/behavior-matrix.md); those keys are retired.

## Axes

| Axis | Values and boundaries to own |
|---|---|
| Target (contract) | `--target js` (closed script), `js-module` (library: exports are the API), `c`, `native`, `all`; the frame each implies (D3.9) |
| Syntax floor | `javascript.ecmascript` `es2015` … `es2022`, `esnext`; `browsers` intersected with it; a construct with no spelling at the floor fails the build |
| Observation | `strip_console` true and false (`print` is the tests' observation channel, so every oracle lane sets `false`) |
| Names | `keep_published_function_names`, `keep_function_names` |
| Contract assumptions | each `assume_*` on and off, and the `length-to-number-elision` compression entry |
| Objective | `cost_model` `raw`, `gzip`, `brotli` |
| Effort | `optimization_level` 0, 13 (the default), 16 and each tier boundary; `candidate_search` `off`, `production`, `always`; CLI `--mode development` |
| Budgets | `candidate_limit`, `candidate_byte_budget`, `candidate_beam_width`, `candidate_proposal_limit`, `terminal_codec_probe_limit`: 1, below, at and above the level's tier |
| Search cadence | `[policy.search]` `codec_schedule` `staged`/`immediate`, `render_batch`, `diversity_interval` |
| Resources | `[policy.resources]` ceilings: a ceiling below the mandatory artifact fails the build; exhausting one during search keeps the best artifact |
| Permission | every tactic `auto`, `on`, `off` in `[policy.tactics]`; the older keys that set a tactic, and a contradiction between the two (an error); `optimization.preset = "none"`; the exact `compression` and `optimizations` allowlists |
| Delivery | `bundle.mode` `single`, `split`, `preserve-modules`; `preload`; `host_modules` `external`, `auto`, `embed`; the cost weights |
| Retired keys | one no-effect key of each kind (warned, removed, listed under `"warnings"` by `--print-policy`); each refused key; retired entries of the two allowlists |
| Author tools | `[lint]` presets and rule levels; `[format]` |

## Facts to pin

- The effective candidate count, candidate bytes and beam width are each the
  lower of the level's tier and the configured ceiling. Level 0 turns proposals
  and terminal probes off whatever the keys say.
- `candidate_search = "off"` and `--mode development` keep only the mandatory
  artifact.
- `off` is a hard veto in direct and searched use; `on` permits and never
  forces. The formation-only lane (`tests/config/no-optimization.toml`) sets
  every tactic `--print-policy` lists to `off`.
- `helper-sharing` and `property-mangling` have no producer yet: their
  permission must change no byte.
- `-j` and `--codec-jobs` are accepted and warn that they have no effect; a
  thread count must never change output bytes (plan M3.6).
- `[compiler] backend` is refused, and so are `priority` other than
  `size-first`, a `[policy.constraints]` table, `public_aggregate_abi =
  "positional"` and a nonzero `for_of_specialize_family`.
- `preserve-modules` chunks and lazy `import()` chunks are broken today (plan
  M3.3); their fixtures are expected failures until then.

## Where it is tested

- `src/config.rs` and `src/compilation_policy.rs` unit tests: parsing, the
  retired-key table, tactic resolution and budget derivation.
- The case runner's lanes: `{formation-only, production} × {brotli, gzip, raw} ×
  {script, module, c}` ([testing.md](../../testing.md)). Plan task M2.2 adds a
  family-veto lane per optional family once the family registry exists (M3.2):
  behavior must match with that family vetoed.
- `scripts/verify-matrix.sh`: every `tests/cases` program with `--target all`,
  under the default policy and the formation-only configuration; JavaScript,
  the native executable and independently compiled C must print the expected
  output.
- `scripts/verify-bundles.mjs`: the delivery modes (plan M3.3 splits it into
  contract and plan assertions).

## Test form

Each configuration test reports:

1. the resolved policy and its fingerprint (`--print-policy`);
2. the tactics each lane leaves enabled;
3. the output hash and its raw, gzip and Brotli sizes;
4. the program's result against its expected output.

Configuration failures are positive tests: unknown keys, duplicates, zero or
over-maximum budgets, retired keys that refuse, and contradictory permissions
must fail with a stable, actionable diagnostic.
