# JavaScript Analysis Maps

LilScript analysis maps explain how the selected JavaScript was compiled. They
are compiler-specific JSON sidecars, separate from standard Source Map v3:

- a source map answers **where did this generated token come from?**;
- an analysis map answers **why was this retained name preserved, shortened,
  shared, or exposed as an export?**

Use both when you need authored source in a debugger and compiler decisions in
an audit:

```toml
[javascript.source_map]
enabled = true
mode = "hidden"

[javascript.analysis_map]
level = "full" # off | summary | full
```

`off` is the default. Direct CLI output writes `<output>.lilmap.json` for
`summary` or `full`. The selected JavaScript contains no reference to that file
and receives no analysis bytes, parse work, download cost, or runtime work.

## Detail levels

| Level | Captured data | Intended use |
|---|---|---|
| `off` | Nothing | Production builds that do not retain compiler diagnostics |
| `summary` | Final name, original name/location, outcome, occurrence count, primary semantic rule, short explanation, and bounded selected-search facts | Routine artifact auditing |
| `full` | Everything in summary plus ordered rule-by-rule evaluations and naming evidence | Compiler debugging and heuristic analysis |

The setting does not change optimization, candidate admission, codec scoring,
name allocation, ABI policy, or emitted JavaScript. It controls only work after
a winner already exists.

## Performance model

Analysis capture deliberately does not log candidate search or instrument the
hot mangler:

1. Candidate generation and scoring run through the normal untraced emitter.
2. After selection, the compiler re-emits only the winning IR/emission plan.
3. That replay records compact semantic naming facts.
4. Final JavaScript token provenance is composed once. Source Map v3 and the
   analysis map share this result when both are enabled.
5. JSON strings are constructed only while serializing the sidecar.

Consequently, builds with both maps disabled have no replay, hashing, token
composition, record allocation, or JSON serialization. With a source map
enabled and the analysis map off, only the existing source-map provenance path
runs; analysis adds no semantic decision capture. Enabled analysis work is
proportional to the selected artifact rather than to the number of rejected
candidates. `full` writes more JSON than `summary`, but neither level retains or
serializes every candidate spelling considered during global search.

## Mangling rules represented

Identifier decisions distinguish:

- external and public-identity ABI constraints;
- readable hygienic allocation when identifier mangling is disabled;
- stable cross-scope local preferences;
- weighted frequency ordering;
- deterministic compact sequential allocation;
- noninterfering SSA values sharing a JavaScript binding;
- reserved-word, occupied-name, and lexical-capture exclusion.

Property decisions distinguish:

- property mangling disabled;
- prototype-sensitive names;
- extern/host ABI names;
- dynamic or untyped boundaries;
- public named aggregate ABI fields;
- host members and methods;
- unowned keys without a proven closed namespace;
- shared versus owner-scoped namespaces and loop-weighted frequency ordering.

Export decisions state whether an ESM boundary retained the declared public
name or surfaced the selected internal binding. Dynamic-import module exports
remain boundary-stable and are reported accordingly.

Rule IDs such as `identifier.frequency-ranked` and
`property.public-aggregate-abi` are versioned semantic schema values. They are
not Rust function names, source-line numbers, or raw `if` logs, so tools do not
break when compiler control flow is refactored without changing behavior.

## Artifact identity and determinism

Every sidecar contains:

- schema kind and version;
- analysis level and coordinate convention;
- exact selected-JavaScript byte length and SHA-256;
- compiler and decision-registry versions;
- a fingerprint of the selected mangling policy;
- source paths, byte lengths, and SHA-256 values;
- bounded-search evidence for single-artifact builds, or the selected chunk
  strategy for compiler-planned bundles;
- deterministic decision IDs, summaries, and records.

The artifact hash covers the JavaScript selected by the compiler, before a
`linked` or `inline` Source Map publication comment is appended. Hidden source
maps and analysis maps preserve those exact bytes, so their output file hashes
match directly.

Line and column values are zero-based; columns count UTF-16 code units, matching
Source Map v3. An analysis map references source by path, location, and hash but
does not embed source text. Enable `source_map.include_sources_content` when a
self-contained copy of the authored LilScript is also required.

An abbreviated `full` decision looks like this:

```json
{
  "id": "name-287a2c6001ed1481",
  "kind": "identifier",
  "category": "function",
  "source": { "name": "buildDebugOptions", "path": "main.lil", "line": 4, "column": 7 },
  "generated": { "name": "q", "firstLine": 0, "firstColumn": 9, "occurrences": 2 },
  "outcome": "mangled",
  "primaryRule": "identifier.frequency-ranked",
  "explanation": "Weighted use-frequency ordering assigned compact spelling `q`.",
  "rules": [
    { "rule": "identifier.external-abi", "result": "not-matched", "detail": "..." },
    { "rule": "identifier.frequency-ranked", "result": "applied", "detail": "..." },
    { "rule": "identifier.selected-spelling", "result": "applied", "detail": "..." }
  ],
  "evidence": { "selection": "frequency-ranked" }
}
```

`result` is one of `matched`, `not-matched`, `applied`, or `skipped`. Summary
maps retain the decision through `explanation` and `evidence` but omit `rules`.

## Inspecting and verifying

The CLI validates the schema, decision count, unique decision IDs, and optional
artifact hash:

```sh
lilscript dist/app.js.lilmap.json --inspect-analysis
lilscript dist/app.js.lilmap.json --inspect-analysis \
  --verify-artifact dist/app.js
```

It prints summary counts followed by original/generated names, primary rules,
and source locations. Hash verification fails nonzero when the sidecar and
JavaScript do not belong together. For `linked` and `inline` source maps, the
inspector recognizes the terminal publication comment and verifies the selected
JavaScript bytes beneath it.

## API and bundles

Artifact-returning Rust APIs expose `Option<JavaScriptAnalysisMap>` beside
`Option<JavaScriptSourceMap>`. `as_str()` returns deterministic JSON;
`artifact_sha256()` and `matches_javascript()` verify identity; count accessors
support build reporting. String-only APIs continue to return JavaScript only
and therefore do not request analysis artifacts.

Split and preserve-modules builds attach one analysis map to each
`JavaScriptBundleFile`, with the chunk filename recorded in `artifact.file`.
The direct CLI writes one `.lilmap.json` beside every emitted JavaScript file and
removes stale sidecars when the setting is turned off.

Lilpack receives the analysis object in the delegated compiler artifact. A
production build emits deterministic assets below
`lilscript-analysis/<source-relative>.lilmap.json`. Their hashes describe the
LilScript compiler's selected module JavaScript before Vite performs later
tree-shaking, bundling, or minification; they are compiler-analysis artifacts,
not maps of the final Vite chunk.

## Rollout and compatibility

The feature is additive and defaults off, so existing projects require no
migration. A staged rollout can enable `summary` in private CI artifacts first,
teach analysis consumers to require `kind` plus `version`, and enable `full`
only for compiler investigations. Consumers must ignore unknown object fields
within a supported version and reject an unsupported top-level version. A
future semantic or coordinate incompatibility increments `version`; merely
adding a rule ID or evidence field does not change compilation behavior.

For performance-sensitive adoption, record an off baseline first, confirm that
selected JavaScript hashes stay equal, measure summary/full winner-postprocessing
separately from candidate-search time, and retain the sidecar outside the
runtime asset graph. This preserves the intended migration boundary: analysis
observes compilation, but never becomes a compilation input.

## Limitations

The map explains retained selected names, not eliminated constructs. Inlining,
scalar replacement, dead-code elimination, and representation changes can
remove the original function, object, or property entirely; such a construct
has no generated name decision. Analysis also reports the winning heuristic
path, not an unbounded transcript of every rejected candidate. The bounded
search evidence says how the winner was selected without turning compilation
into candidate-count-sized logging.
