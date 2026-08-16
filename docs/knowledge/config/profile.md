# `[profile]`

Parent: [Config](README.md). Optimizer: [IR optimizer](../compilation/ir-optimizer.md).
Profile loading requires resolved `[optimization].profile_guided` (explicit `true`,
or the `maximum` preset default). JavaScript also requires
`profile-guided-optimization`, supplied by level ≥ 10 or an exact optimizations
allowlist. This gate is configured independently of `candidate_search`, so profile
guidance can affect the single configured pipeline even when multi-candidate search
is off.

Optional version-1 JSON plus inline tables. Inline counters override file counters.

```sh
lilscript src/main.lil --profile-template lilscript.profile.json
```

Keys (no source annotations): `$entry`, function name, `Class.method`, `Class.constructor`, span-keyed closure; loops append `#index` (`render#0`). Counters must be positive.

| Key | Default | Meaning |
|---|---|---|
| `path` | unset | JSON file relative to config dir |
| `specialization_min_count` | 100 | Minimum profile hits to clone a call |
| `max_specializations_per_function` | 8 | Clone cap |
| `max_clone_instructions` | 64 | Body size cap |

Hot or statically estimated-profitable direct-call groups may clone a callee for
constant / known-function arguments. Constant captures may clone a closure and drop
environment slots. These clones are transformations inside one optimizer pipeline;
they are **not** independently accepted or rejected by the codec. With candidate
search enabled, the compiler may also build a pipeline with call-site or capture
specialization disabled and compare the resulting complete artifacts. With search
off, only the configured specialized pipeline is emitted.

`[optimization]` switches remain authoritative: `call_site_specialization = false` cannot be undone by a hot profile.
