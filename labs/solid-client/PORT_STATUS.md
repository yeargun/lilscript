# SolidLil status

Baseline: `solid-js@1.9.13`, pinned at
`3be495cec52bf78d7cc61f054af00320ecf4058c`.

## Browser runtime parity

| Surface         | Public exports | Behavior gate | Solid Brotli-11 | SolidLil Brotli-11 | Size status   |
| --------------- | -------------: | ------------- | --------------: | -----------------: | ------------- |
| Core            |        54 / 54 | Exact         |         8,551 B |            8,548 B | Pass, −0.04%  |
| Store           |          8 / 8 | Exact         |         4,286 B |            4,195 B | Pass, −2.1%   |
| Web             |        73 / 73 | Exact         |        11,655 B |           12,151 B | Open, +4.3%   |
| Total inventory |      135 / 135 | Exact         |               — |                  — | API gate pass |

The API audit also compares browser value types and observable function arities;
there are zero mismatches. The candidate gate runs 469/469 unchanged upstream
tests across 26 files with Core, Web, and Store imports resolved to SolidLil.

“Exact” here means the pinned browser ESM contract and executable evidence in
this lab. It is not a claim of exhaustive TypeScript inference compatibility,
server-target SSR parity, or compatibility with every package in the Solid
ecosystem.

## Ownership, errors, async work, and teardown

The verified runtime includes signals, memos, effects, roots, contexts,
selectors, resources, transitions, Suspense/SuspenseList, error boundaries,
scheduling, external sources, keyed/positional collections, immutable and
mutable stores, DOM insertion, events, control flow, portals, and browser
hydration fallbacks.

Lifecycle gates specifically cover:

- reverse-order cleanup and idempotent root disposal;
- owner/effect handle invalidation and stale disposer safety after slot reuse;
- mapped and indexed row cleanup;
- pending resource resolution after unmount; and
- stable pools (currently eight owner slots and sixteen effect slots for the
  combined workload), all free with zero pending effects after churn.

## Open world and closed world

The reusable open-world Core build keeps the public ABI stable and is the size
comparison shown above. The closed-world diagnostic recompiles the same runtime
with export mangling enabled: all 54 source exports are present under 54 renamed
bindings, with no original public name left in the emitted module.

The equivalent counter application is a separate closed-world benchmark. Solid
JSX and LilScript DOM lanes must match count, memo, parity, batch, reset, effect,
idempotent unmount, and stale-handler behavior before their Vite and Closure
artifacts are measured.

## Complete client-rendering LSX contract

Runtime parity and LSX parity remain separate gates. Both client gates pass.

| LSX gate                    | Passed | In-scope |
| --------------------------- | -----: | -------: |
| Lowering verified           |     21 |       21 |
| Integrated runtime evidence |     21 |       21 |
| Server-coupled exclusions   |      2 |        2 |

The strict client LSX gate passes. The integrated differential covers user
components, ordered live spreads, nested Show/Switch composition, component
rows in For/Index, Dynamic string/component/null and SVG selection, Portal
head/SVG/shadow-root variants, xlink/xml attributes, immediate branch cleanup,
and idempotent unmount. Show and Match now distinguish keyed raw-value callbacks
from identity-preserving live accessors and short-circuit later matches.
`For` now accepts full LilScript callback types and differentially proves
Solid-compatible prefix/suffix duplicate identity, fallback ownership, removal
cleanup, and unmount. ErrorBoundary and Suspense prove construction and delayed
errors, reset/remount identity, two-resource reveal, pending-content ownership,
pending unmount, and cleanup. Hydration and SSR are explicit server-coupled
exclusions. The current exact fixture includes the SolidLil host ABI:

| Integrated LSX fixture  | Brotli-11 |   Gzip-9 |      Raw |
| ----------------------- | --------: | -------: | -------: |
| Official Solid JSX      |  12,560 B | 13,905 B | 38,629 B |
| SolidLil LSX + host ABI |  10,780 B | 12,069 B | 34,412 B |

The complete client fixture is 14.2% smaller under Brotli-11. Its isolated
resource ratios are 1.021× median interaction time, 0.975× live retained heap,
and 0.980× post-unmount heap; all pass the 1.05× regression ceiling.

## Reproduce

```sh
npm run test:upstream:candidate
npm run test:compat
npm run test:lifecycle
npm run audit:api
npm run audit:lsx
npm run verify:modes
npm run verify:store
npm run verify:web
npm run benchmark
npm run publish:web
```

Generated JSON, Markdown, and standalone HTML evidence lives in `artifacts/`.
