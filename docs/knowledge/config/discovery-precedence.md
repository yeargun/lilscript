# Discovery and precedence

Parent: [Config](README.md). Code: `load_project_config` in `src/config.rs`, CLI in `src/main.rs`.

## Discovery

The CLI walks from the input module toward filesystem root looking for `lilscript.toml`. `--config path` selects one file explicitly. Missing config → struct defaults (`JavaScriptPriority::SizeFirst`, `cost_model = brotli`, `optimization_level = 15`, `candidate_search = production`, …).

`config_dir` is the canonical parent of the TOML, used to resolve `[profile].path`.

## Precedence (high wins)

1. **CLI**
   - `-j N` / `--jobs N` → `compiler.resources.threads`
   - `--codec-jobs N` → `compiler.resources.codec_workers`
   - `--mode development` → `javascript.candidate_search = off` (multi-IR/emission
     expansion off; independently configured finalization features remain)
   - `--delegate-bundling` → `bundle.mode = single`
   - `--config` → which file, not which keys
2. **`[mangle]`** explicit `identifiers` / `properties` / `exports` / `pool_strings`
3. **Exact `javascript.compression` allowlist** if the key is present (including `[]`)
4. **`javascript.priority` defaults** for which compression decisions are on
5. **`javascript.optimizations` exact list** if present, else **`optimization_level`**
6. **`[optimization]` per-key overrides** over **`preset`**
7. **Struct defaults**

Hard offs:

- `[optimization] call_site_specialization = false` (etc.) cannot be revived by level 15.
- `[optimization] function_subsumption = false` suppresses JS subsumption candidates even if the JS feature is listed.
- `compression = []` turns off identifier mangling, pooling, grammar elision, … unless `[mangle]` turns a name/pool flag back on.

## JS vs native option objects

| API | Used for |
|---|---|
| `optimizer_options()` | Native IR; `[optimization]` only |
| `js_optimizer_options()` | JS IR; `[optimization]` AND JS effort/compression; inline limits from priority |
| `js_options()` | Canonical `IrJsOptions` emission |
| `compress_pass_options()` | AND of `[optimization]` compress keys with compression decisions; outlining default false |
| `native_options()` | `[native]` storage |

`--target all` shares parse/semantics, then runs JS and native optimization on **copies**.

## Validation (selected)

- `bundle.min_chunk_bytes` > 0, `max_chunks` > 0, `shared_min_imports` ≥ 2
- at least one `[bundle.cost]` byte weight
- `candidate_limit`, `candidate_byte_budget`, `candidate_beam_width` > 0
- `compiler.resources.threads` (when present) and `codec_workers` > 0
- `optimization_level` ≤ 15; `function_layout_exact_limit` ≤ 18; `local_name_reserve` ≤ 256
- `max_candidate_raw_growth_percent` ≤ 1000
- duplicate names in `compression` / `optimizations` / `lint.providers` are errors
- `package.abi` / dependency ABI must match compiler ABI

## Repo vs struct defaults

Checked-in root `lilscript.toml` is a **size-first release** profile (level 15, brotli, explicit compression list, `local_name_reserve = 48`). `JavaScriptConfig::default()` uses `local_name_reserve = 16`. Projects that omit TOML are not identical to this repository’s file.
