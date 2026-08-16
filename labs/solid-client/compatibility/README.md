# Solid compatibility gates

SolidLil is pinned to `solid-js@1.9.13`. Runtime compatibility, LSX syntax
coverage, transport size, and performance are independent gates: passing one
never turns another into a win.

## Current result

| Gate                                       |                    Result | Status     |
| ------------------------------------------ | ------------------------: | ---------- |
| Unchanged upstream candidate tests         | 469 / 469 across 26 files | Pass       |
| Browser Core/Web/Store exports             |                 135 / 135 | Pass       |
| Browser export types and function arities  |                 135 / 135 | Pass       |
| Curated LilScript behavior cases           |                 112 / 112 | Pass       |
| Curated optimizer executions               |                 224 / 224 | Pass       |
| Deterministic lifecycle/disposal workloads |          Solid-equivalent | Pass       |
| LSX client lowering families               |          21 / 21 verified | Pass       |
| LSX client runtime families                |          21 / 21 verified | Pass       |
| Server-coupled LSX families                | 2 explicitly out of scope | Excluded   |
| Exhaustive Solid type-contract groups      |       Not yet inventoried | Incomplete |

`npm run test:upstream:candidate` runs the pinned upstream files unchanged while
resolving public `solid-js`, `solid-js/web`, and `solid-js/store` imports to
SolidLil. The reference run remains a separate `npm run test:upstream` gate.

The 112-case corpus is not presented as a fraction of 469. It is an additional
LilScript-native suite run with maximum optimization and with optional
optimizations disabled on the JavaScript target, for 224 executions. C/native
are excluded from this Solid-runtime count because those targets do not yet
implement the JavaScript exception and Promise semantics required by Solid.

## Runtime and teardown

`npm run test:lifecycle` compares Solid and SolidLil under:

- root → signal → memo → effect → cleanup churn;
- idempotent disposal and stale disposers after internal slot reuse;
- keyed `mapArray` and positional `indexArray` creation/removal cleanup;
- resource promises resolved after their owner has been disposed; and
- stable owner/effect pool high-water with every slot returned and no queued
  effects.

The full `npm run benchmark` adds alternating interaction samples, isolated
live/post-unmount retained-heap samples, and repeated lifecycle-memory samples.
The application worker retains stale controls deliberately and proves they can
no longer update disposed computations or document effects.

`npm run test:lilx` also builds one feature-rich fixture through official Solid
JSX and SolidLil LSX, then compares normalized mount, update, keyed/positional
reconciliation, native/delegated event, ref/directive, namespace, fallback,
error recovery, Suspense reveal, pending-subtree disposal, and unmount digests.
The same differential runs against minified production bundles. Only families
exercised by that fixture are marked runtime-verified in the LSX ledger.

## Build boundaries

| Mode         | Public contract                                                  | Config                     |
| ------------ | ---------------------------------------------------------------- | -------------------------- |
| Open world   | Reusable ESM names remain stable for external consumers          | `config/open-world.toml`   |
| Closed world | The complete consumer graph permits property and export mangling | `config/closed-world.toml` |

`npm run verify:modes` checks all 54 Core exports and behavior in the reusable
open-world bundle, then proves that all 54 generated runtime bindings are
renamed in the closed-world module. Brotli-11 is the primary size gate. Web and
Store have separate exact-surface reports so a Web size loss cannot be hidden
inside the Core result.

## Commands

```sh
npm run test:upstream:candidate # 469 unchanged tests against SolidLil
npm run test:compat             # 112 cases × two optimizer modes
npm run test:lifecycle          # teardown, stale-slot, collection, async disposal
npm run audit:api               # exports + browser types/arities + evidence ledger
npm run audit:lsx               # independent LSX feature ledger
npm run verify:modes            # open-world behavior/size + closed-world mangling
npm run verify:web              # exact Web surface and Brotli report
npm run verify:store            # exact Store surface and Brotli report
npm run benchmark               # interaction and repeated retained-memory gates
```

`npm run parity:api` and `npm run parity:lsx` are expected to pass. The LSX
strict gate covers the complete client-rendering contract; hydration and SSR
remain visible in the inventory as explicitly excluded server subsystems.
