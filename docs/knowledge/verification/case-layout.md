# Case layout

Parent: [verification](README.md). Contract:
[paired cases](paired-case-contract.md).

## Canonical folder

```text
comparison/cases/<case-id>/
├── README.md           # intent, observable contract, expected optimization
├── case.toml           # stable metadata and gates
├── lilscript/
│   └── main.lil        # may include additional .lil modules
├── javascript/
│   └── main.js         # may include additional JS modules
├── host/               # optional shared HTML/extern/fixture data
├── expected.json       # canonical value/effect/API/DOM oracle when applicable
└── snapshots/          # optional reviewed non-generated expectations
```

Build outputs and result JSON belong outside the source folder (for example
`comparison/cases/build/<case-id>/<lane>/`). Generated cases may be materialized in a
separate directory, but their generator, seed, template ID, and values must appear in
metadata.

This is the target durable layout. Hand-authored cases live under
`comparison/cases/canonical/<family>/<id>/`. The generated catalog still
materializes ignored `generated/<name>/` folders from `catalog.mjs`. Both go through
the same minifier + codec gate.

## `case.toml` fields

At minimum:

```toml
schema = 1
id = "struct/local-scalar-replacement"
family = "aggregates"
contract = "stdout"       # stdout | value | trace | api | browser | artifacts
boundary = "closed-app"   # closed-app | esm-library | script-tag | split-app
target = "es2022"
expect = "strict"         # parity | strict
metrics = ["raw", "gzip9", "brotli11"]
portable_native = true
tags = ["struct", "escape-local", "scalar-replacement"]
```

Optional fields select baseline lanes, browser engines, configs, expected failures,
fixture files, timeout, generator provenance, and issue/quarantine metadata.

## Naming

Use a stable semantic path, not an output number: `loops/continue-phi-two-values` is
better than `test-417`. Parameter variants append readable values or a stable seed.
Renaming a case breaks historical trends and needs an alias entry.

## README questions

Every hand-authored case answers:

1. What observable behavior is equal?
2. What proof should LilScript possess?
3. What compiler/bundler decision is under pressure?
4. Is parity or a strict win expected, and why?
5. Which boundaries/configs/tool lanes are eligible?

One case should isolate one primary reason. Medium/app cases may combine features but
must list their coverage tags so failures can be routed back to a micro reproducer.
