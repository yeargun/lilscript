# G2 private property naming completion

Compiler pin `/home/azureuser/lilscript-work/bin/g2-complete-1/lilscript`, SHA-256
`9be0c7c1d4d7795904840877c82885604ddde95ca977cdeb4e9d63ef5a3b0910`;
policy algorithm 18 / search 31 / walk 10. [Identity](identity.json) records the
complete Rust/Cargo source manifest. This build includes G1's class-owner fix.

Private generic storage is eligible under the existing reflected-set closure.
Flattened field slots express actual interference: inherited fields agree,
fields on an instance stay distinct, and sibling-only/unrelated fields reuse
names. The shared lexical/property encoder uses digits only in continuations.
Kept class identities qualify only under explicit pristine builtins; a declared
`__proto__` still blocks the inheritance family. Extern, reflected and published
families remain protected. Reserved names and direct/cached/search vetoes hold.
`PropertyNames` is registered in the common representation contract. No field
initialization order changes or extra runtime operations are introduced.

[Focused checks](batch.log): **19 passed**, covering reflection, private/public
generics, inheritance, prototype setters, name reservations, name encoding,
independent lexical/property controls and search replay. The two adapter-name
fixtures now request `keep_function_names = true` and independently expect the
anonymous wrapper prescribed by the existing contract; identity/arity/call
checks remain. Legacy fixture results were not rerun.

[Comparison](comparison.json), reproduced by [measure.mjs](measure.mjs), covers
three affected development workloads at matched open/closed boundaries. Effort
13 is used separately for each objective; inlining/scalar replacement are off
to retain the affected layouts. All **144 numeric observations** and 18 public
shape/enumeration/computed-access/serialization executions pass in independent
Node processes. Delivered files match the selected codec scores.

| Boundary | Raw delta | Gzip delta | Brotli delta |
|---|---:|---:|---:|
| Closed applications | −154 | −96 | −57 |
| Open libraries | 0 | 0 | 0 |

Every individual cell is non-growing. Closed sibling slots account for −17 /
−67 / −54; private generics for −60 / −13 / 0; observed private classes for
−77 / −16 / −3. Exact terminal judgments increase 243 → 276 across the 18 cells.
These are logical counts, not a timing claim. The pristine contract is matched
between both compilers; no relaxed boundary is counted as a naming win.

No full library/fleet, native or held-out/competitor qualification was repeated.
No ratchet baseline or port artifact was updated. V1 retains broad qualification;
S4 retains the separately recorded `records-128` source-rule refusal. Generated
configuration documentation and `git diff --check` pass.
