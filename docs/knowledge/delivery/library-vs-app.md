# Reusable library vs closed application

Parent: [delivery](README.md). ABI controls:
[JavaScript shape and ABI](../config/javascript-shape-abi.md). Language boundary:
[packages and exports](../language/packages-exports-abi.md).

These are different compilation claims, even when they share source.

| Boundary | Retention/public contract | Safe compression latitude |
|---|---|---|
| closed executable | entry behavior only; source exports are accessibility | all linked private/export names may internalize when no host observes them |
| closed LilScript app as JS | all consuming LilScript linked before emission | export/property mangling and opaque positional handles may be valid |
| reusable ESM | root runtime exports, names, arity, construction, public fields | internals mangle; public contract remains stable |
| script-tag/global facade | declared globals and plugin-facing keys | thin stable facade over typed internals |
| mixed Lilpack app | `.lil` closed subgraph plus foreign Vite graph | LilScript must leave typed foreign ESM seams for Vite |

A closed-app result cannot serve as a baseline for npm jQuery or another public
library. Conversely, charging a reusable library for names that the closed app does
not expose is not a fair app comparison.

For every build, record target (`js` vs `js-module`), public aggregate ABI, mangle
exports/properties, function spelling, and artifact set. API eligibility includes
export names, global names, property descriptors, arity, constructibility, identity,
throw behavior, and plugin/dynamic access—not only output text.

jQuery deliberately keeps separate public and closed-app TOMLs. Its latest checked-in
pre-canonical public row is larger than npm under all recorded codecs and is
ineligible; the app config does not erase that result. Canonical scorer refresh is
still required for current byte claims.
