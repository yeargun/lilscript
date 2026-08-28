# ident-03 — catch identity bugs without a library

Parent: [ledger](../LEDGER.md). Status: landed. Depends on [ident-02](ident-02.md).

## Question

Can the differential oracle produce receiver-rebinding shapes on its own, so this class
is caught by the test suite instead of by a 660-case parser port?

## Current hypothesis

Yes. The kernel is `differentialIdentity` in `lilscript-differential`: field write,
rebind, computed access, invoked captured rebind, and the ident-01 `prev=cur` loop.
The remaining production hole was not IR: a `use_count == 1` nullish phi stayed in
the expression cache across `CallValue`. Function parameters flushed because the
cached `b.href??0` matched a local name; a top-level `Record` is a global, so the
name check skipped the snapshot and rematerialized after the IIFE (`94` not `89`).
Callee-code flush now binds every non-reusable cached expression, not only names
in `value_names` / `local_names`. Empty formals and `strip_console` on the no-opt
oracle configs were emit/oracle holes, not identity.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-19 | Oracle binary present | `ls target/debug/lilscript-differential` | present | diag |
| 2026-08-28 | Captured rebind still rematerializes | `compile_source` of snapshot-then-`rebind()` | node printed `94` (new+new) not `89` (old+new) | diag |
| 2026-08-28 | Seeded identity kernel | `target/debug/lilscript-differential --cases 8 --compiler target/debug/lilscript` | optimized JS compared (oracles matched through that lane); failed on optimizer-disabled JS `function closure(,$2,$3)` SyntaxError — empty formal, not the identity prints | diag |
| 2026-08-28 | Function-parameter captured rebind | `cargo test --lib snapshot_of_a_record_field_survives_a_captured_rebind` | passed (`89`) | gate |
| 2026-08-28 | Top-level global captured rebind | `cargo test --lib snapshot_of_a_top_level_record_field_survives_a_captured_rebind` | passed (`89`) | gate |
| 2026-08-28 | Differential all JS lanes including captured rebind | `target/debug/lilscript-differential --cases 8 --compiler target/debug/lilscript` | 8 programs matched evaluator across optimized, optimizer-disabled, peephole on/off, C, and native | gate |

## Log

- 2026-08-19 — Opened. Motivation: marked is currently the only thing that finds these,
  which makes the feedback loop a whole port long. — **OPEN**
- 2026-08-28 — Seeded `differentialIdentity` (write, rebind, computed, ident-01 loop).
  Production optimized JS matched the evaluator on `--cases 8`; optimizer-disabled
  JS is invalid `closure(,$2,$3)` (empty first formal). Invoked captured rebind
  is still not in the kernel. — **OPEN**
- 2026-08-28 — Empty formals are emit (`unique_name("")` → `_`; unmangled `v{id}`).
  No-opt oracle configs now keep `print` (`strip_console = false`). Captured rebind
  is in the kernel. Callee flush snapshots non-reusable cache entries so a global
  `Record` rebind cannot replay `l.href??0` after the IIFE. Interpreter mutable
  captures share `Rc<RefCell<Value>>` with the frame. — **LANDED**

## Next step

None for this note. Continue as [ident-04](ident-04.md) (canonical `identity/` folders)
then [arch-02](arch-02.md).
