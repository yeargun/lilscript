# D2 consumer delivery and module semantics

Base `13e9261c`; algorithm67/search33. `identity.json` records the source hashes;
`artifacts.json` pins the release compiler, canonical codec, Node and esbuild.
The production pin is `lilscript-work/bin/d2-consumers-1/lilscript`.

This batch closes D2's six gates: initialization/placement, lazy effects and
cycles, consumer controls, typed host lowering, public immutable product graphs,
and source maps. Initialization proofs now account for entry consumers and lazy
activation. Source cycles preserve instantiation, live bindings, order and TDZ.
Single-file lazy effects use cached activation records, including cached errors;
effect-free lazy modules avoid that runtime. CJS lazy activation retains failures
that Node's ordinary require cache would evict.

Host parsing/lowering uses typed Oxc trees. Embedded hosts own separate graph
nodes; host/source/foreign dependencies retain authored order and entry reach.
Internal host imports stay live and respect lexical shadowing. External imports
retain loading and named-export validation even if no local use survives. Foreign
requests that cannot preserve their lazy/effect ordering in a single file are
diagnosed with split/preserve or typed-embedding alternatives. Raw carried host
text stays limited to single-file delivery; unsupported host syntax and directly
imported mutable host exports retain their existing external/diagnostic boundary.

TOML exposes independent annotation, facade placement and source-map controls.
Generated adapters preserve reference aliases, value-product copies and deep
immutability while leaving private product storage positional. Source origins
survive cross-module inlining and copied class-field initializers. Maps use UTF-16
coordinates, final filenames and actual source contents. Their bytes, hashes,
closure membership and codec costs belong to the scored artifact and manifest.
They add no source-text/point storage when disabled. The public service's single
file accessor also works for planned output; external maps are separate files.

Focused evidence in `logs/`:

- `delivery-final-batch.log`: 14 passing consumer tests, covering effort 0/13,
  ESM/CJS, single/split/preserved delivery, runtime order/identity/failures and
  independent Node SourceMap decoding. The bundler oracle is ignored here and
  qualified separately below.
- `final-regressions.log`: application IIFE mapping plus both affected host
  regressions pass. `adapted-regressions.log` retains six other passing legacy
  checks and the two failures repaired by that final batch.
- `typed-host-build.log`: seven initialization, graph, annotation and API checks.
  `host-parser-linkage.log`: five typed host/compaction/live-import checks.
- `consumer-bundles-qualified.log`: independent esbuild 0.25.9 consumer builds
  pass for split/preserve × auto/facade × annotations off/all. Every build retains
  the observable effect. Annotation output lets the consumer remove an unused
  initializer: 111→71 raw, 119→80 gzip9, 94→71 Brotli11. These are consumer bytes,
  not a claim that adding comments shrinks the compiler's own artifact.

`qualify.py` measures six release builds, each independently optimized at effort
13 for raw, gzip or Brotli with maps off/external. It checks public graph aliases,
freezing, first-load-only lazy effects, original source locations and exact actual
file hashes/raw/codec lengths against manifest and search totals. Timing is one
observation per build, not a statistical compilation-speed comparison.

| Configuration | Raw objective | Gzip objective | Brotli objective |
|---|---:|---:|---:|
| Maps off | 1,608 | 716 | 620 |
| External maps, including map bytes | 2,013 | 1,002 | 875 |

These are the full runtime/adapter-inclusive costs of this fixture, not a fleet
comparison. All six actual artifact/manifest/runtime checks passed.

Intermediate failures and build diagnostics are retained in `logs/`. In
particular `hosts-focused.log` was an accidentally broad selection that was
stopped; it is not passing-suite evidence. No full library/fleet rerun or accepted
baseline update is part of D2. V1 owns broad final qualification. Independent
program-region admission and full-file parsing remain in force; generated
activation/linkage wrappers have runtime oracles, not an independent whole-wrapper
canonical proof. Native module task/public const ABI work remains N1/N2.
