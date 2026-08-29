# Baseline toolchains

Parent: [verification](README.md). Research comparison:
[Terser/Oxc/Vite](../research/terser-oxc-vite.md) and
[Closure ADVANCED](../research/closure-advanced.md).

## Principle

Compare against the strongest semantically eligible JavaScript artifact, not a single
favorite tool. Pin exact package/JAR versions and full options in the report. A
dependency lockfile is necessary but not sufficient; record the resolved version and
command/API options for each run.

## Lanes

| Lane | Proper use | Minimum contract |
|---|---|---|
| Terser | Single-file or already bundled JS compression/mangling | `toplevel`, module mode, ECMA target, passes, unsafe/property options recorded |
| Oxc minifier | Fast compressor/mangler with different rewrite set | compressor/mangler/codegen options and platform binding recorded |
| esbuild | Transform/minify and simple bundle baseline | target, format, platform, tree-shaking, legal comments recorded |
| Vite | Real application/module graph and asset build | Vite version, minifier choice, target, Rollup/Rolldown config recorded |
| Closure ADVANCED | Closed-world or precisely externed programs | pinned JAR/hash, ADVANCED, language modes, entry/chunk/extern/export flags recorded |

The micro runner, `comparison/cases/run.mjs`, gives every tool the ES2022 target. It uses
Terser (`passes=3`, top-level compression/mangling), Oxc through Rolldown's pinned
minifier utility, and both esbuild's script-preserving transform and closed-IIFE form
(the latter permits top-level mangling but pays for its wrapper). It executes every
result, excludes a semantically invalid candidate from size selection while failing
the case, and selects the smallest valid artifact independently for raw, gzip-9, and
Brotli-11. Schema 6 records exact options, resolved Oxc platform binding, runtime and
canonical scorer identity, durations, and artifact hashes. It does not yet run Vite or Closure
for those micro cases. The maintained Closure app lane pins a JAR version and SHA-256
separately.

The structural runner, `comparison/algorithms/run.mjs`, has a stronger required
frontier. Every case competes Terser, Oxc, Closure `ADVANCED`, and esbuild
script/IIFE candidates over the same executable boundary. Every module graph also
competes direct Closure `ADVANCED` dependency pruning, direct esbuild bundling,
Vite/Oxc, and Vite/Terser. A Closure build or oracle failure makes the case red; it
is not an optional result silently removed from the frontier.

Terser, Oxc, esbuild, and Vite target ES2022. The pinned Closure package uses its
newest supported named input/output mode, `ECMASCRIPT_2021`, which is a strict subset
of ES2022. Closure receives only the three `algorithm*` host functions as custom
externs; `env=BROWSER` supplies the `console` contract. Reports record the custom
extern digest, package version, selected native-or-Java runtime, and runtime binary
or JAR digest as well as the full flag set.

## Bundler vs minifier

Do not run a single-file minifier on unbundled module graphs and call it a bundle
comparison. Conversely, do not charge Vite’s HTML/assets/runtime to one side unless
the other side supplies the same application boundary. For graph cases, compare
production artifact sets and keep the chosen minifier visible.

## Closure eligibility

Closure ADVANCED assumes control over the compilation unit and can rename/remove code
not protected by extern/export contracts. Provide the same externally observable API
as the LilScript row. The algorithm corpus is intentionally a closed runtime script,
so both its prepared-executable Closure candidate and, for module cases, its direct
dependency-pruned graph candidate are required and eligible. A no-export closed-program
Closure result remains ineligible for a public jQuery facade. See
[Closure research](../research/closure-advanced.md).

## Unsafe modes

Terser/Oxc/Closure assumptions that change standard observable behavior are separate
lanes. They are eligible only when the case contract proves those assumptions (for
example unmodified built-ins and no observable comparison-order difference). Never
turn on `unsafe` globally merely to obtain a smaller baseline.

## Updates

A baseline version/options update refreshes all affected artifacts. Review semantic
drift first, then size drift. Keep the previous report long enough to attribute which
cases moved and why.
