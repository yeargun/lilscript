# Integrated labs

`labs/solid-client` is an ordinary, root-owned workspace in the LilScript
monorepo. The former gitlink and nested repository boundary were removed so the
compiler, runtime, LSX frontend, compatibility ledgers, applications, reports,
and documentation evolve in one change set.

The integrated browser-runtime gate currently verifies 135/135 public
Core/Web/Store exports and passes 469/469 unchanged pinned upstream tests. LSX
is reported separately: its strict client-rendering gate passes 21/21 in-scope
families, including Suspense and ErrorBoundary. Hydration and SSR are explicit
server-coupled exclusions rather than client-parity gaps.

The main benchmark site does not require the lab dependencies or upstream fixture. An archived,
portable LSX size snapshot lives at
`benchmarks/popular/apps/solid/size-report.json`; the popular-library runner
uses that stable snapshot when publishing the historical application row. The
LSX parser, lowerer, Vite transform, and strict feature ledger are integrated,
along with the differential fixture and resource gates. The old todolist stays
archived; the current 21/21 fixture is the reproducible client benchmark.

Generated dependencies, `dist`, the reproducible upstream Solid fixture, and
temporary compiler output remain ignored by the lab rather than being checked
in or duplicated. Reproducible JSON/Markdown/HTML reports remain under
`labs/solid-client/artifacts`, and `npm run publish:web` updates the main site's
filterable/selectable benchmark data from those reports.
