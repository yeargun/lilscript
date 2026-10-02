# D3 integration evidence

Base: D2 `9a3cca29`. Acceptance: [D3](../../../docs/migration/d3-acceptance.md).
The work began October 1 and completed October 2. D3 is complete within its
acceptance contract; native integration (N1) follows.

## Implementation

1. Build, lint and editor consumers share configured graph discovery, canonical
   module identities and checked interfaces. All open editor buffers override
   dependencies on disk. A retained session owns parsed input and bounded
   elaboration checkpoints; disabling reuse takes the cold path. Dependency
   contracts, graph order, configuration, schema and source offsets guard reuse.
   Changed-length edits conservatively invalidate checkpoints. No arbitrary
   relocation of checked fragments is claimed.
2. TOML `delivery.also` builds format/entry subsets from one checked graph, with
   independent objectives, resolved policies, mandatory baselines and optional
   work budgets. API/CLI manifests include source/configuration/artifact digests,
   output groups, final paths and exact byte receipts.
3. Port sources own their contracts, public API adapters, host providers, flags
   and package entrypoints. The compiler emits ESM/CJS/browser files; installers
   validate manifests and copy exact bytes. Historical compiler-side source
   patches are archived and no longer applied. Upstream test oracles and explicit
   third-party providers retain their own ownership and hashes.
4. Integration fixes include opaque host dependency ownership and order,
   package-private aliases, portable package exports, foreign export cells,
   keyword exports, target module URLs, namespace behavior, constructor ABI,
   finally cleanup and uninitialized record-field seeds. Static single-file
   opaque hosts share live internal dependency state and preserve mixed source
   initialization order. Lazy opaque hosts still require split/preserved output.
   Single-file source functions retain independently allocated bindings when
   their observable function names collide.

Expected effects: reuse avoids repeated frontend work; format/objective search
and compression retain their own costs. Removing post-minifiers does not by
itself establish a size win. Port effort/assumption decisions remain explicit
in their TOML profiles. Development-mode integration builds do not establish
production compression performance.

## Verification scope

The logs retain initial failures and their focused corrections. In particular,
`graph-resources.log` includes an editor alias failure corrected in
`editor-foreign.log`; `outputs-qualified.log` includes an execution fixture
failure corrected in `outputs-runtime.log`. They are not all-green summaries.
The format CLI, directory naming, host class and keyword-export checks have
separate qualified logs.

Subsequent checks cover cache/graph/format behavior and representative delivery
consumers: Motion's shared ten-entry graph; MobX cleanup and descriptors;
KaTeX's independent metrics oracle; Monaco worker routing; Solid identity and
shared signals; Zod schemas/locales; rehype-katex structural parity; PlayCanvas
public descriptors; and Vue's source-owned package formats. Individual compiler
pins are recorded in `ports-*-identity.json`. Later focused repairs do not imply
every earlier consumer was rerun at the final pin.

V1 owns the full maintained/held-out runtime and independent raw/gzip/Brotli
qualification, including the remaining Motion consumer size gap. V2 owns clean
release rebuilds, current site reports, staged-default qualification and
release receipts. Generated working artifacts are kept separate from D3's
source integration commits. Existing release/site reports remain historical.

## Source ownership

Port commits and push destinations are recorded separately from compiler pins.
Several upstream port `main` branches advanced with release and dependency work
after the checked local revisions. Their D3 commits use
`codex/d3-compiler-delivery`, retaining the audited source/dependency versions;
they do not claim to incorporate or qualify those later releases. The local
probelil repository has no Git remote; its source commit and dual-lane result
are recorded as local evidence.

## Final integration result

Algorithm72, final compiler pin `ports-13-identity.json`:
`0adb33483e58a2010d892e9ffcacd689d8ce3cbf780b2a51466cf74a5f77a3a6`.
The final release build passed. The focused consumer group passed 25 checks,
with one external-tool test not selected and one new function-name fixture
corrected to publish both observed functions; its six ESM/CJS/IIFE × effort
0/13 cells then passed. The earlier D3 integration group passed 31 checks.
No full library suite was rerun.

Vue's 32 configured profiles produced 3,543 artifact rows and 19 declarations
in development compilation mode. The first focused batch passed 46/47 checks.
Two internal anonymous function descriptors and one CJS default-provider
setting were repaired in their source/TOML owners. Rebuilding the affected
profiles and running the 17 affected checks passed. This includes separate
ESM/CJS/browser imports, cross-package shared identity, all five public global
containers, template compilation and a selected reactivity entry. Unaffected
profiles retain their earlier input snapshot; V1/V2 must requalify production
search and complete release artifacts. `vue-delivery-receipts.json` and
`vue-input-receipts.json` retain the exact per-profile identities.

PlayCanvas passed four public delivery checks. Its old source differential
harnesses now use explicit compiler-written development and external-host
profiles: 14 GLSL and 13 shader-definition fixtures pass. The external-host
profile intentionally shares upstream constructors for the existing instanceof
checks; the independent bundled public oracle remains separate. Initial
nominal-identity and external-path failures are retained in the logs.
React Markdown's two entrypoint checks pass with its package script's declared
`development` condition. The initial invocation omitted that condition and
incorrectly expected a development assertion from the production entry.

`ports-source-revisions.json` records 26 pushed port repositories plus the local
probe. Vue sources and profiles live in this compiler repository. Existing
working release artifacts and unrelated site changes were not included in the
source integration commits. Site receipt qualification is explicitly V2,
because it consumes the release outputs that milestone qualifies. This resolves
its old D3 mapping without claiming any site has been rebuilt or published.
