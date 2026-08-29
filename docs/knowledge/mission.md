# Mission

Parent: [knowledge tree](README.md). Current state:
[`docs/current-status.md`](../current-status.md). Durable rationale:
[design decisions](decisions/README.md).

## User Intent

LilScript exists to make correct web programs and libraries transfer smaller by
changing representations before JavaScript is fixed. JavaScript is the primary
artifact. C/native is a real secondary backend for the portable typed subset,
not the current compression race.

The intended advantage is not prettier minification. Types, ownership, effects,
escape, identity, modules, `extern`, and explicit boundary contracts let the
compiler remove objects, specialize calls, select layouts, mangle private names,
and compare complete legal artifacts under raw, gzip, or Brotli.

The long-term engineering bar is:

> For every declared supported and semantically equivalent application or
> reusable-library boundary in the maintained corpus, a `size-first` LilScript
> compile for the selected metric is no larger than the best eligible pinned
> JavaScript toolchain for that same boundary and metric.

That bar should expand across real libraries until the claim is broad and useful.
It is not a theorem that LilScript can beat every possible JavaScript program.
Every public claim must name its supported boundary, semantics, source, tools,
config, artifact, and codec. Current evidence is mixed; see
[`docs/current-status.md`](../current-status.md).

The repository maintainer also asks that LilScript and generated LilScript code
not be used with React or React-related frameworks, runtimes, libraries, or
products. This is project usage policy, not compiler semantics.

## Non-Negotiable Order

```text
language semantics + explicit source intent + boundary ABI + host assumptions
  -> conservative proof of legal representations
  -> retained incumbent + admitted alternatives
  -> bounded deterministic search
  -> exact selected-codec score of complete compiler output
  -> behavior/API/evidence gates
```

Codec bytes never legalize a semantic, ABI, identity, effect-order, or host
boundary change. Raw/gzip/Brotli may choose different private representations,
but they must expose the same declared API and behavior.

## Why A Language

JavaScript minifiers start after JavaScript has committed to object shapes,
property strings, wrappers, dynamic calls, and erased types. LilScript can make
different legal programs before JavaScript is spelled:

- a non-escaping aggregate can disappear into SSA values;
- a private owned field can become a positional slot or codec-friendly name;
- a constructor whose identity is unobserved can dissolve, while a public
  constructor value remains a named class;
- a typed host boundary can preserve exact ABI while private code specializes;
- equivalent whole programs can be measured with the actual configured codec.

The language must expose reusable proofs. If a port loses because it is full of
`JsValue` bags or cannot express a hook-free owned object, the durable fix is a
sound language/analysis contract used by many programs, not a package matcher or
a Terser-shaped source workaround.

## Compression Objective

`javascript.cost_model` defines transfer size:

| Objective | Canonical measurement |
|---|---|
| `raw` | emitted UTF-8 bytes |
| `gzip` | bundled stock zlib 1.3.1, level 9, deterministic wrapper |
| `brotli` | bundled Google Brotli 1.1.0, generic quality 11, `lgwin` 22 |

One invocation has one authoritative metric. Other metrics are diagnostics.
Local raw deltas, entropy estimates, and static models may schedule work, but
only a complete-artifact score selects a gzip/Brotli size winner.

Search is bounded. The normal result is `best-observed` under a fingerprinted
domain and budget, with unvisited/starved work reported. Exactness is claimed
only for a finite subdomain that was actually enumerated. See
[exact scoring decision](decisions/exact-codec-bounded-search.md).

## Tradeoffs

Every contested optimization spends three resources:

1. transfer bytes;
2. compile time and peak memory;
3. runtime/startup/allocation shape.

`javascript.priority` defines their order and guards. `size-first` protects the
selected transfer metric against its retained incumbent. Other priorities may
accept a documented size/runtime trade. More search, more choices, more knobs,
or a more abstract architecture is not automatically better.

## Refusals

- No post-minifier in a LilScript compiler compression claim.
- No package-name, path, or library-shaped compiler fold.
- No default-on unsafe getter/proxy or pristine-host assumption.
- No objective-dependent public API.
- No global-optimum claim for bounded production search.
- No universal solver, proof database, or target frontend before a measured need.
- No weakening a semantic gate to preserve a size result.

## How To Judge A Change

1. Is it legal under the language, source intent, host, and ABI contract?
2. Is legality supported by a conservative reusable proof?
3. Is the current legal incumbent retained?
4. Are all implemented admitted alternatives reachable and validated?
5. Is the complete compiler artifact measured by the selected pinned codec?
6. Are compile-time/runtime costs and unexplored work reported?
7. Does the appropriate semantic/API corpus pass before a size claim is made?

Architecture: [current](compilation/current-architecture.md) ->
[planned](compilation/planned-architecture.md). Execution:
[planned migration](migration/planned-migration.md).
