# Compiler analyses and proof invalidation

Parent: [compilation](README.md). Optimizer: [IR optimizer](ir-optimizer.md).
Source anchors: `src/semantic.rs`, `src/optimizer.rs`, and `src/value_analysis.rs`.

| Analysis | Fact | Enables | Invalidated by |
|---|---|---|---|
| types/narrowing | exact nominal/scalar/union member | legal ops, unboxed/tag-free paths | assignment, ambiguous runtime category |
| interprocedural effects | global/argument mutation, host/throw/call effects, dynamic coercion/proxy evaluation | DCE, motion, pure-call removal | unknown/impure call, observable `JsValue` operation, trusted boundary |
| escape graph | local, typed escape, untyped escape | scalar replacement, layout/mangle freedom | export/extern/`JsValue`/print/unknown call |
| integer ranges | conservative i32 intervals | coercion elision, safe update spelling, bounded lookup proofs | overflow/unknown call/alias/unsafe field owner; every eager or lazy export parameter starts at the full typed domain |
| finite values | complete set of up to four constants | branch fold, specialization, field propagation | widening, eager/lazy export, extern/indirect/closure/untyped owner |
| alias/mutation roots | which local aggregate state is observable | dead store/instruction removal, sinking | retained aliases and effectful calls |
| call/reference graph | direct, closure, method, address-taken targets | devirtualization, inlining, dead functions | unresolved indirect target |
| array length/use | fixed/stable lengths and callback observation | builder/pipeline fusion, loop contraction | resize, escape, aliasing callback |

Analyses are conservative and monotone where they feed legality. A missing fact costs
an optimization; it does not authorize a speculative rewrite. Codec scores choose
among legal representations only and cannot compensate for an unsound proof.

Typed non-mutating `Math`, string, and array intrinsics contribute no host effect;
their semantics come from the LilScript operation. Explicit dynamic evaluation is
classified separately. Potential coercion hooks, proxy traps, and boundary throws
invalidate purity and block DCE, value-number merging, motion, helper substitution,
and repeated-region outlining where those transforms require an unobservable
operation.

Eager root exports and every lazy-module export are the same public boundary for
range/finite summaries, constant-parameter specialization, global propagation and
internalization, escape/aggregate ownership, and dead-function reachability. A
narrow internal call cannot specialize or narrow a function that JavaScript may call
after loading a lazy chunk.

The performance model is a separate deterministic shape analysis over typed IR. It
counts deoptimization-prone shapes, allocations, indirect calls, and weighted hot
code. It is a ranking proxy, never a browser benchmark and never a semantic gate.

## Review rule

Every new optimization documents which fact proves it, every operation that kills the
fact, and a negative test for each boundary. Tests should include unknown calls,
exports, host getters, captured mutation, aggregate aliasing, exception edges, and
indirect callable values—not only the profitable local example.
