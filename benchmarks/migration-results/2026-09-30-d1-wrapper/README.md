# D1: private classic-script delivery

Source checkpoint `dd935b80`, compiler `~/lilscript-work/bin/d1-wrapper-1/lilscript` SHA-256 `9204a5f36b8213f9dfd32555a7495ccf437a22680d1d6e74abaf50e3680d7260`; codec SHA-256 `ee8dff12ffc3e095d6d654fe6e9de80c80021a59a3e7f5f4ff6284243b66c664`.

Application scripts now default to a private arrow IIFE. Explicit `bare` remains available to an embedding that owns the scope; module libraries still default to ESM. Policy, prepared output, printing, independent structural admission, measured scores and delivery metadata all describe the same container. The wrapper preserves classic strictness and lexical `this`. Incompatible static imports/exports are refused. This does not implement CJS, UMD or a global library export.

The complete library checkpoint passes 1,681 tests with nine existing ignores, excluding the previously qualified expensive data-table case; all six CLI tests pass. This includes repeated loads and colliding root names, host callbacks, exact scores under every objective, and cross-container retained-artifact refusal. Two existing tests were updated for the new frame and output text; both focused reruns pass.

The [explicit bare ratchet](bare-ratchet.md), with [full receipt](bare-ratchet.json), passes 642 inputs × three objectives. Every lane's state, size and selected-output hash matches the accepted S1 baseline.

The [private IIFE comparison](iife-ratchet.md), with [full receipt](iife-ratchet.json), has the same 36 ledgered failures and no new behavior failures. Compared with the matched bare artifacts:

| Corpus | Raw delta | Gzip delta | Brotli delta |
|---|---:|---:|---:|
| Cases | +6,732 | +5,577 | +4,293 |
| Applications | +77 | +58 | +37 |
| Algorithms | +121 | +32 | +11 |
| Total | **+6,930** | **+5,667** | **+4,341** |

This is the approved container's overhead, including exactly 11 raw bytes for each passing artifact. Its verdict against the bare size baseline is **fail**, intentionally retained. The runner records the container and refuses to replace a baseline from a different delivery contract, even with growth acceptance. It defaults explicitly to the historical `bare` contract; `--format iife` measures the new application default separately. The 16 runner tests pass. No size improvement or matched competitor win is claimed for adding the wrapper.

The [reference port suites](ports.json) pass markedlil 29/29, zodlil 1,353/1,353 and posthoglil 21/21 across nine artifacts. All nine artifact hashes and measured sizes are identical to S1's port run. These suites establish functional coverage, not runtime-speed parity. Wrapper elimination where the scope is unobservable is a possible later optimization, and requires its own proof and measurement.
