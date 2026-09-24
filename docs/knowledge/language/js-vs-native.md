# JavaScript vs native

Parent: [Language](README.md). Related: [mission](../mission.md), [current architecture](../compilation/current-architecture.md), [future architecture §11](../../future-architecture.md#11-native-and-cross-target).

## One checked program, two targets

`--target all` parses and checks once. The same Program IR then feeds two
targets: JavaScript formation, target rewrites and codec-scored search, and the
native plan with its C11 writer. JavaScript choices never change the C output.
Today native gets none of the JavaScript side's optimizations; the program
rules of plan phases M6–M7 are target-neutral so both targets get them, and
M11.5 wires native to the shared facts.

Native object code is produced by emitting C11 and invoking `${CC:-cc} -std=c11
-O3 -fno-fast-math -ffp-contract=off`. The C text is the portable artifact.

## Reject rather than approximate

| Feature | JS | Native |
|---|---|---|
| `extern class` / host objects | Direct property ops | Diagnostic |
| `JsValue`, `JSON.parse`, `Regex` | Implemented | Diagnostic (regex: M11.6) |
| `Task`, `async`/`await`, generators, exceptions | Native Promise / `function*` / `try` | Diagnostic (M11.6) |
| `import extern`, `import()` | ESM / typed task | Diagnostic |
| `extern` functions (user C ABI) | Host call | Diagnostic until M11.3 |
| `Record<T>`, `Object.*`, JSON | Null-prototype object | Diagnostic until M11.4 |
| Strings | JavaScript strings | UTF-16 code units, same semantics |
| Unions / nullables | Erased / raw `null` | Tags at boundaries |
| Class inheritance | Static dispatch | Static dispatch through pointer records |
| Generics | Erased | Box at polymorphic boundaries |

This is a **language** rule: a JS size trick that needs a different native meaning is illegal. Compression work belongs in codec-scored JS emission, not in forked semantics.

## Native knobs

None. The old route's `[native]` storage switches and profile-guided
optimization are retired with it (a `[native]` table warns "no effect in this
compiler"). Stack and region storage return as a consequence of the escape fact
(plan M11.5), not as switches.

## Current focus

JS transfer size (gzip/Brotli) is the active race. Native exists to keep the IR honest and to ship `exec` later. Do not add JS-only semantic shortcuts that would make the second target a lie.
