# Browser and host cases

Parent: [verification](README.md). Migration:
[phase 04](../migration/README.md#phase-04). Web ABI:
[`docs/web-platform.md`](../../web-platform.md).

## Harness shape

Serve production artifacts from a loopback HTTP server. Launch a pinned Playwright
browser, navigate normally, and collect a structured result from page state rather
than scraping incidental console output. Preserve console/error/request traces as
diagnostics.

Each paired case supplies the same HTML/fixture data when possible and swaps only the
compiled application artifact. Cache is disabled for cold-transfer checks; warm-cache
behavior is a separate lane.

## Oracles

- canonical DOM tree (namespace, element/attribute/property/text state);
- ordered event trace with phase/currentTarget/default state;
- public global/ESM API descriptors and calls;
- request URLs/method/body/status/abort sequence against a deterministic local server;
- storage/history/worker messages;
- uncaught error, rejection, and console trace;
- initial and lazy artifact request set.

Normalize only known nondeterminism declared in metadata. Do not erase ordering,
descriptors, stack-independent error types/messages, or request boundaries just to
make results equal.

## Runtime measurements

Warm functions, alternate artifact order, use enough samples, and apply a documented
non-inferiority rule. The existing `benchmarks/browser/` lane provides a model: 400
samples after warmup and paired-bootstrap upper confidence bounds against a 1.03
ratio. Browser feature cases should first prove correctness and transfer; add timing
only to stable workloads.

## Extern discipline

Every host global, property, method, callback, and exception used by LilScript must be
declared through the typed boundary or an explicit compiler intrinsic. Cases should
include a name-collision/mangling check so compression cannot break the host ABI.

## Engine scope

Start with pinned Chromium because a flaky cross-browser lane teaches little. Add
Firefox/WebKit only after fixtures are deterministic. If web standards permit engine
differences, declare an accepted result set per engine rather than mutating one output
into another.
