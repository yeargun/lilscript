# S4: diagnose detached primitive methods

Source `5dd3b4bd`; compiler `~/lilscript-work/bin/s4-primitive-1/lilscript` SHA-256 `5f29c9cbb89de554b38eb8e2539e311b23217e4fa4793a61c27ceb1fbf0daac9`, codec SHA-256 `814bc420c91b7849ff148b7095b5377e3ed1ae7e058d8d331007c7bfe0b9f42e`.

Checking a detached primitive method now emits the receiver-call diagnostic, including optional reads. The public check service and both compilation targets agree before formation. Direct calls and explicit receiver closures remain supported; ordinary callable fields and dynamic host members retain their contracts.

All 1,697 library tests pass, with four existing ignores and the separately qualified expensive table case excluded. Three focused tests cover the new check and valid alternatives. The [generic gate](ratchet.md), with [complete receipt](ratchet.json), passes 642 inputs × three objectives and the same 33 ledgered failing lanes. Its complete result table equals the stable-rule scheduler's table.

The [reference port suites](ports.json) pass markedlil 29/29, zodlil 1,353/1,353 and posthoglil 21/21. All 20 reported files are byte-identical to the preceding scheduler qualification. This batch corrects diagnostics; it makes no size or runtime-speed claim.
