# Effect summaries and discarded calls (M6.2, M6.3, M7.2)

Reproductions for the effects owner (`src/program/effects.rs`) and the call
graph (`src/program/call_graph.rs`). Each `.out` is written from the program's
meaning, never from a compiler. The removal itself (a call no longer in the
output) and the `pure` contract diagnostic are asserted by the unit tests in
`src/program/effects_tests.rs`; these cases check that every lane still prints
what the source means.

| Case | What it pins |
|---|---|
| `effects-discarded_pure_calls_leave_output_unchanged` | `pure int twice(int x)`, a counted `for` loop and a string-length `while` loop, called with proven arguments and discarded: removed, output unchanged |
| `effects-empty_warning_and_invariant_bodies` | motionlil's development-only `warning`/`invariant`: empty bodies reached through exported function-typed bindings. A direct call of the empty body goes. Since M6.5 a call through the binding goes too in the module lane: `clamp` runs only after the bindings settle, so their reads cannot throw. A classic script's root bindings are not sealed, so there the call stays. An argument's own effect (`noted`) runs either way |
| `effects-discarded_call_arguments_still_run_in_order` | A removed call never removes its arguments: they run once, in order |
| `effects-discarded_call_that_may_throw_still_throws` | A discarded call whose body may throw stays |
| `effects-pure_extern_result_unused_still_runs` | D3.6 as settled: a `pure extern` has no observable effect (a `pure` function may call it), but no termination proof, so a discarded call stays. The pending amendment would change this case |
| `effects-raw_argument_conversion_in_a_discarded_call_still_runs` | D2: an `int` parameter may hold a raw host value; the body converts it, so the call stays and `valueOf` runs |
| `effects-jsvalue_getter_and_valueof_in_discarded_calls_still_run` | A getter or `valueOf` reached through a `JsValue` is user code: the calls stay |
| `effects-exported_function_survives_its_discarded_calls` | Module lane: an internal discarded call of an exported function goes; the export stays callable |

Divergence (`while (i != 0) i += 2`), unbounded recursion and the `pure`
violation diagnostic cannot be observed by a terminating, compiling case; they
are unit tests (`discarded_calls_with_effects_or_without_proofs_stay`,
`loops_without_a_counted_bound_and_recursion_may_diverge`,
`the_pure_contract_reports_observable_effects_with_a_span`), and D3.6's
diverging-call case stays in `d3_clause_tests`.
