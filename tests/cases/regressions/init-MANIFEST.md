# Initialization order (M6.5)

Reproductions for the initialization owner (`src/program/initialization.rs`):
the root statement that settles each module-level cell, the first root
statement each body may run at, and the per-access answer effects, liveness
and the unit-local facts read. Each `.out` is written from the program's
meaning (D3.7: a read before initialization throws where the source would),
never from a compiler. The facts themselves and the removed calls are asserted
by `src/program/initialization_tests.rs`; these cases check that every lane
still prints what the source means.

| Case | What it pins |
|---|---|
| `init-a_read_before_initialization_still_throws` | D3.7: a named function exists from instantiation and the first statement calls it before the cell it reads is set; the read throws, then the same call returns the value |
| `init-an_exported_function_the_host_calls_early_still_throws` | An exported function handed to the host by a statement before its cell settles: the host calls it at once and the read throws (`.host.js`) |
| `init-an_import_cycle_calling_back_during_initialization_still_throws` | A static cycle: the imported module initializes first and its root statement calls back into the entry before the entry's cell is set |
| `init-root_constants_read_by_functions_created_earlier` | zodlil's kind constants: read by a named function created before them and called only by later statements. The facts prove the reads initialized; the tree does not substitute them yet (M5.2 must carry each binding's settling statement and each function's first point to `root_constants.rs`) |
| `init-warning_through_an_imported_binding_after_initialization` | motionlil's `warning`/`invariant` across modules: exported bindings of empty bodies, called by functions that run only after both modules initialized. The calls leave the module lane's production output; `noted` still runs once, in order |

Not a case: a body escaped into a host object before a statement that may
throw. If that statement throws, initialization stops and the host may call
the body later with the cell never set; observing it needs an uncaught
top-level exception, which the runner counts as a crash. It is the unit test
`a_body_escaped_before_a_throwing_statement_may_run_after_initialization_stops`.
