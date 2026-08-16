# Phase 06 — browser and host boundaries

Parent: [migration](README.md). Host contract:
[browser cases](../verification/browser-host-cases.md).

## Objective

Move beyond Node stdout without confusing a DOM shim with browser semantics. Validate
real observable state in headless browsers and keep extern boundaries explicit.

## Required families

- DOM creation/query/mutation, attributes/properties, text/HTML, class/style, and
  document fragments;
- event registration/removal, bubbling/capture, delegation, default prevention,
  listener identity, and custom events;
- timers/microtasks/promises, fetch/XHR abort/error paths, URL/form encoding;
- storage, history/location, observers, workers, modules/dynamic imports, and
  progressive enhancement boot timing where supported;
- public script-tag globals, ESM exports, `noConflict`-style ownership, and host
  exceptions;
- browser engine matrix after Chromium is stable; engine-specific expectations must
  be explicit, never silently normalized.

## Measurement

Use a local HTTP server and exact production artifacts. Record DOM/API snapshots,
event traces, console/error traces, network requests, initial/lazy bytes, and runtime
samples separately. Runtime evidence cannot waive a size failure, and size cannot
waive behavior.

## Exit criteria

- Browser cases run deterministically in CI with pinned engine revisions.
- Every host call/property in a case is covered by a typed extern or documented
  intrinsic, and boundary names survive/mangle according to config.
- At least one real case exists for script-tag, ESM, lazy enhancement, and worker
  delivery.
- jQuery verification uses the same browser contract before its size row can become
  eligible.
