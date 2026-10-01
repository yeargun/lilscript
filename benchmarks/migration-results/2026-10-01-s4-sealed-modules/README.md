# S4 sealed module boundaries

1. Internal constructor exports are visibility, not an ABI declaration. Seed
   publication only from declared delivery roots and actual runtime constructor
   observations; preserve canonical identities through aliases and re-exports.
2. Do not materialize a private constructor's unused runtime interface when all
   of its uses are typed construction. Keep the type interface and dependent
   initialization graph. Preserve dynamic namespace and host crossings.
3. Qualify aliases, public re-exports, multiple delivery roots and actual
   first-class/dynamic observations through shared facts and public build paths.
   Observed generic class transport remains the following S4 boundary batch.

Prediction: unused internal publication no longer pins fields/prototype methods,
allowing existing structural and naming rules to work. Public identity and ABI
are unchanged. Check a coherent batch through module facts and public build
routes; compare affected artifacts under independent raw/gzip/Brotli objectives.
No full library or fleet rerun.

## Result

Internal constructor visibility no longer seeds the public/observed class sets.
A checked canonical-binding scan retains actual first-class/dynamic constructor
observations. Conversion omits only dissolved constructor values from matching
internal import/export interfaces; type interfaces and initialization remain.
The existing structural optimizer can remove the unused class/prototype.

Twelve distinct focused checks pass (three new tests, two existing dual-binding
checks, five module-contract checks and two additional nominal-identity checks).
The two new identity tests were also included in their existing group. Release
build passes. No full suite was rerun.

Pin `s4-sealed-modules-1`, SHA-256 `7511059d61700fa3d8ab8ec1fe89ac0e33e8af6e03b75c45deae38e230653c56`; algorithm 25 / search 31 / walk 10 /
local facts 10. [Identity](identity.json) records source hashes;
[comparison](comparison.json) retains four module graphs, their config/artifacts,
search work and independent host observations. Totals against pin 24 are
1006 → 837 raw (**−169**), 703 → 613 gzip (**−90**), and 570 → 499 Brotli
(**−71**). The win is entirely internal visibility; the public barrel,
constructor-value and dynamic-namespace cells are unchanged. Constructor and
prototype identity, methods, keys, arity and typed results agree in every lane.
Scores equal canonical delivered bytes. No fleet, timing or Closure claim.
S4 remains active; observed generics and the other language/catalog work remain.
