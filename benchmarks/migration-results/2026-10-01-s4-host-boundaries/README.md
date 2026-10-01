# S4 host callbacks and singleton retirement

S4 remains active. This batch reuses the concrete struct adapter for generic and
nullable `JS.assume` values and arrays. Host callable adapters now preserve
omission/defaults, real rest arrays, explicit receivers and public length.
Receiver wrappers inherit strictness from their factory even in scripts;
primitive receivers do not acquire a host box. The nullable return path uses
the common value codec instead of a second handwritten conversion.

R16 removes unsupported `object Name` singleton declarations and their AST,
checker merge/member tables, module-interface special cases and LSP completion
branch. Ordinary object literals remain available. No existing compiled
singleton program is being changed: the old source converter refused them.
Census and focused verification are recorded below when complete.

These changes have no optional optimization switch. ABI semantics are required
under every effort/permission setting. Algorithm 22 separates their artifacts;
search 31 and walk 10 remain unchanged. Wider host/generic contracts,
entry-boundary propagation, shapes/variants/enums and the rest of S4 remain open.


## Evidence

Pinned release: `/home/azureuser/lilscript-work/bin/s4-host-boundaries-1/lilscript`,
SHA-256 `0167a9299329bd439f1a32e39074bf05dd065a8de7b506eee635ffbbad8cc073`.
The CLI and LSP release build passes. [identity.json](identity.json) records the
source tree. Seventy distinct focused checks pass across the batch: 16 boundary
checks and 54 parser/ownership/module checks. The first boundary group exposed
an invalid reflected-length marker on a required-parameter wrapper; its focused
follow-up passes after correction. Initial negative evidence is retained.

The compiler-lexer census covers **27 maintained workloads and 2,707 source
files**, including the in-repository Vue checkout, and finds **zero singleton
declarations**. [object-census.json](object-census.json) contains per-source
hashes, workspace revisions and scanner identities. Comments and string contents
are skipped using compiler tokens. `object { key: value }` remains available.

[comparison.json](comparison.json) retains one matched existing callback under
each independently selected objective, against `s4-boundaries-1`: raw 531 → 549
(**+18**), gzip 256 → 255 (**−1**), Brotli 223 → 220 (**−3**). The common nullable
codec changes helper shape and costs raw bytes on this fixture; the regression
is retained, not presented as a universal compression win. The new typed
receiver/default/rest callback passes independent host observations in scripts
and modules under all three objectives. Every selected score equals the final
served artifact. There is no prior supported artifact for that new capability.
No full library/fleet rerun, runtime speed, Closure, or native-complete claim.
No port output or ratchet baseline changes.
