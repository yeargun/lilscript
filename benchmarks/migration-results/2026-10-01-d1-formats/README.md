# D1 formats and manifest qualification

Base `a3b55dab`; algorithm66/search33. Source and production binary hashes are in
`identity.json`; compiler, canonical codec, Node and fixture hashes are repeated
in `artifacts.json`. The pin is `lilscript-work/bin/d1-formats-1/lilscript`.

The substantial batch implements CJS, library IIFE/UMD, explicit bare delivery,
interop/publication controls, live imports/exports, script/module frame rules,
injective module paths, URL spelling and manifest cleanup. Wrappers and linkage
are emitted by the compiler and included in every scored artifact. No external
minifier or hand-edited output is used.

`qualification.json`, `focused.log` and `cli.log` record **34 passing focused
checks**: nine format/runtime checks, target-copy/resource rollback, independent
structural admission (including a negative case), prior affected ESM contracts,
three CLI manifest/config checks. The final expanded namespace guard has its own
`namespace.log`: `[name]` cannot expand to a reserved wrapper binding such as
`exports`. That refusal-only fix postdates the release pin; production artifacts
below do not use those names and are unchanged by it.

`qualify.py` and `artifacts.json` record **21 passing production builds**, effort
13, independently optimized for raw/gzip/Brotli. The library runs as ESM, CJS,
IIFE and all three UMD branches. The application runs as IIFE, bare and CJS.
Independent Node oracles check mutable live exports, alias identity, argument
dependent behavior, immutable null-prototype records and application output.
Every actual file is remeasured by the canonical codec; hashes, raw length,
codec length and row totals match the compiler manifest/search explanation.

| Artifact | Raw objective | Gzip objective | Brotli objective |
|---|---:|---:|---:|
| Library ESM | 368 | 253 | 214 |
| Library CJS | 669 | 329 | 286 |
| Library IIFE | 693 | 343 | 296 |
| Library UMD | 914 | 422 | 357 |
| Application IIFE | 77 | 83 | 67 |
| Application bare | 66 | 73 | 59 |
| Application CJS | 116 | 118 | 85 |

These are format costs for these fixtures, not a fleet comparison or an argument
that one codec's winner serves the others. Live getters and explicit container
semantics add bytes. D2 owns consumer annotations and export granularity.

`cases.json`/`cases.log` record the small existing-case matrix through the new
CJS/bare runner lanes. Full language/library/fleet reruns remain deferred.

Initial failures are retained: build/fixture fixes in `initial-build.log`,
`initial-runtime.log`, `activation-fixture.log`, `proof-build.log`; the production
harness originally executed CJS application code as ESM. `initial-qualification.log`
retains that failure. The follow-up fixes the runner and preserves already passed
rows rather than repeating them. No compiler failure was hidden by a ledger.

Admission independently checks the emitted program-statement region against its
target partition and parses the full file. Generated wrapper/linkage semantics
are runtime-qualified; a full independent canonical proof of the wrappers is not
claimed. Lazy effects/cycles, consumer shaking, source maps and stored public
product graphs are D2; shared graph/port integration is D3.
