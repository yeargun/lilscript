# Control flow, nullish flow, and exceptions

Parent: [language](README.md). Contract: [statements and expressions](../../language-v0.1.md#statements-and-expressions)
and [async/exceptions](../../language-v0.1.md#async-tasks-and-exceptions). Compiler
anchors: checking in `src/check.rs`, elaboration into regions in
`src/program/from_source.rs`, and JavaScript formation in `src/program/javascript.rs`.

LilScript evaluates expressions left to right. `&&`, `||`, `??`, optional access,
conditional flow, and `match` evaluate only the selected arm. Assignment and update
are expressions; computed receivers/indexes are evaluated once. `break`, `continue`,
and `return` retain their structured targets through lowering.

`??` and `??=` test only `null`; `false`, `0`, and `""` remain present. Optional
member/index access skips the index on a null receiver. Optional method calls are not
accepted yet because receiver binding and portable call semantics are not defined.

Closed enum `match` is exhaustive unless the final arm is `_`. Scalar literal
`match` also supports enum, integer, string, and boolean patterns with duplicate
and exhaustiveness checks. The scrutinee runs once and only one arm executes.
`if` / `else` has both statement and value-producing expression forms; the
expression form requires `else` and lowers to a `Select` region. `?` remains the
nullable type marker, not ternary syntax. The JavaScript target may spell a
branch as a statement or a conditional expression when both preserve evaluation
order.

`throw` accepts any non-`void` value. `try` requires `catch`, `finally`, or both, and
native JavaScript completion order is preserved: `finally` runs for normal and abrupt
completion and may replace the earlier completion. Catch values are `JsValue`; no
error-record shape is assumed.

Owner decision [D3](../../future-architecture.md#d3-in-full) settles that
implementation-specific resource-exhaustion timing may differ after optimization,
while results, ordinary throws, argument errors, host effects and divergence remain
observable; each clause has an executable case. It is not permission to remove
ordinary exceptions or perform unbounded evaluation.

## Compiler boundary

The Program IR keeps control flow as nested regions (`If`, `Loop`, `Try`, `Block`,
`ShortCircuit`, `Select`, `ForIn`, `ForOf`). There is no CFG and there are no phis:
values merge through cells, and formation maps each region to the structured
JavaScript (or C) statement it came from. Exception regions stay `try` statements
with mutable bindings, so throw timing is preserved. An unused catch binding is
omitted only when it has no use.

Optional member and index access evaluates its receiver once and is formed today
as a conditional on that receiver (`a!=null?a.v:null`), not as a native `?.`
chain. The deleted route's optional-chain recovery is in
[history](../history/compilation/ir-optimizer.md#proof-scoped-nullable-simplification).

Tests must exercise effects in conditions/arms/indexes, loop-carried values, labeled
completion equivalents, updates on members, throws between mutable assignments, and
`finally` overriding return/throw/break/continue.
