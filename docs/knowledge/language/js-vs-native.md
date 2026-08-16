# JavaScript vs native

Parent: [Language](README.md). Related: [mission](../mission.md), [native config](../config/native.md).

## Shared frontend, two optimizer copies

`--target all` parses and type-checks once, then optimizes **separate** IR copies:

- JS uses `js_optimizer_options()` (priority inline budgets, compression-gated compress passes, JS effort features).
- Native uses `optimizer_options()` plus `[native]` storage placement. `javascript.priority` does **not** change C.

Native object code is produced by emitting C and invoking `${CC:-clang} -O3`. The C text is the portable artifact.

## Reject rather than approximate

| Feature | JS | Native |
|---|---|---|
| `extern class` / host objects | Direct property ops | Diagnostic |
| `JsValue`, `JSON.parse`, `Regex` | Implemented | Diagnostic |
| `Task`, `async`/`await`, generators | Native Promise / `function*` | Diagnostic |
| `import extern`, `import()` | ESM / typed task | Diagnostic |
| Unions / nullables | Erased / raw `null` | Tags at boundaries |
| Class inheritance | Flattened static dispatch | Rejected until pointer ABI |
| Generics | Erased | Box at polymorphic boundaries |

This is a **language** rule: a JS size trick that needs a different native meaning is illegal. Compression work belongs in codec-scored JS emission, not in forked semantics.

## Native-only knobs

`[native]`: partial escape analysis, stack allocation, region allocation, `stack_array_element_limit`. Eligible non-escaping arrays/classes/closure envs go on the frame; larger bounded arrays go in a per-function region released on every return. Values that return, go global, hit a phi, get captured unsafely, or pass an unknown call stay on the heap. These change storage, not source-visible ownership.

`profile_guided` on `[optimization]` can feed native PGO the same way; JS additionally requires the JS effort feature `profile-guided-optimization`.

## Current focus

JS transfer size (gzip/Brotli) is the active race. Native exists to keep the IR honest and to ship `exec` later. Do not add JS-only semantic shortcuts that would make the second backend a lie.
