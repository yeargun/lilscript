# Control flow, nullish flow, and exceptions

Parent: [language](README.md). Contract: [statements and expressions](../../language-v0.1.md#statements-and-expressions)
and [async/exceptions](../../language-v0.1.md#async-tasks-and-exceptions). Compiler
anchors: structured shapes and phis in `src/ir.rs`, lowering in `src/lower.rs`, and
CFG simplification in `src/optimizer.rs`.

LilScript evaluates expressions left to right. `&&`, `||`, `??`, optional access,
conditional flow, and `match` evaluate only the selected arm. Assignment and update
are expressions; computed receivers/indexes are evaluated once. `break`, `continue`,
and `return` retain their structured targets through lowering.

`??` and `??=` test only `null`; `false`, `0`, and `""` remain present. Optional
member/index access skips the index on a null receiver. Optional method calls are not
accepted yet because receiver binding and portable call semantics are not defined.

Closed enum `match` is exhaustive unless the final arm is `_`. The scrutinee runs
once, duplicate/unknown variants are errors, and only one arm executes. `match` is
**enum-only**. `if` / `else` is statement-only. `?` is nullable, not a ternary.
Value-producing `if` lowers to a phi; the emitter may recover `?:` via
`local_phi_expression_regions` (default off under Brotli). That recovery is not a
source form. Language RFC: [compressor surface](compressor-surface.md).

`throw` accepts any non-`void` value. `try` requires `catch`, `finally`, or both, and
native JavaScript completion order is preserved: `finally` runs for normal and abrupt
completion and may replace the earlier completion. Catch values are `JsValue`; no
error-record shape is assumed.

## Compiler boundary

Ordinary reducible flow enters SSA with explicit phis and structure metadata. The JS
emitter may compare conditional/comma/state-machine/loop spellings, but exception
regions retain native structured bindings and are excluded from CFG rewrites that
cannot preserve throw timing. Unused catch binding elision requires a checked zero-use
binding. The parsed peephole validates generated syntax before any final contraction.

Optional member/index phis retain their receiver provenance. JavaScript emission may
recover a native optional chain only after proving receiver identity, the matching
null branch, lazy arm order, and a safe structured merge; otherwise the explicit CFG
is the correctness fallback. A separately proven non-null receiver can erase the
guard before CFG simplification. See the
[IR optimizer](../compilation/ir-optimizer.md#proof-scoped-nullable-simplification).

Tests must exercise effects in conditions/arms/indexes, loop-carried phis, labeled
completion equivalents, updates on members, throws between mutable assignments, and
`finally` overriding return/throw/break/continue.
