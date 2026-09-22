# Bounded Evidence Commands

The [accepted receipt](run-2026-09-19T14-37-46.001Z/receipt.json) passes 63 focused
checks on Node 24.11.1 in about 10.8 seconds, with unchanged input digest
`b58c4221b959eb5bc21db384ad2cfed90cbd7c30c32a24a7035d99a83b8dfa25`.
Receipt SHA-256:
`1566cda71a5137c1408bf0526619f95be1b524a5f84bd01979f2c8acf34e9a3c`.
No compiler build or full library test suite ran.

- Build, Node/Vitest test, runtime-version and canonical-codec commands share one
  asynchronous POSIX process-group owner. Arguments remain literal, without a shell.
- Timeouts escalate TERM to KILL, and normal parent exit cleans remaining group
  members. Output overflow is a failure. Missing executables and invalid options
  cannot become successful empty results.
- Inherited pipes have a bounded final wait. Forced closure records a failure and
  explicit incomplete cleanup, even if the direct child exited successfully.
- Scoped exit/INT/TERM handlers clean active groups, preserve application handlers
  or ordinary signal termination, and are removed after the final owner settles.
- Public portgate fixtures preserve failed build/codec receipts with no artifacts
  or test credit. Node evidence now pins tool/runtime inputs before execution and
  rejects replacement during an otherwise passing test run. Async callers await
  completion; original required cases remain unchanged.
- Marked's unchanged 75-case inventory now records additional closed-profile,
  VM/browser-byte and executable package/deployment coverage as required gaps.
  These gaps can no longer be mistaken for a fully verified library gate.

## Limits

This is cooperative Node supervision, not an OS sandbox or hard RSS guarantee.
Escaped sessions are outside group killing; the escaped-pipe tests clean their own
deliberately escaped workers. Supervisor SIGKILL cannot run cleanup, and event-loop
or kernel stalls remain outside a hard wall-clock claim. Buffer limits count stream
bytes, not all allocator/process memory. Workspace copying, hashing and Git metadata
reads are not covered by per-command deadlines. This receipt does not qualify a
complete installed dependency closure, library output or release compiler speed.

`qualify.mjs` records source identities, command, runtime, logs and stop state for
the focused tool cohort. It is evidence collection, not another migration plan.
