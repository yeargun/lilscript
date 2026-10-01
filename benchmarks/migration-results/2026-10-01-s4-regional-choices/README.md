# S4 regional representation pins

1. Implement `@choose(family = alternative)` on functions and methods for the
   registered region-owned JavaScript spelling families. Alternative 0 retains
   canonical spelling; 1 selects the proved alternative. Pins constrain legal
   sites, never establish their proof. Reject unknown families, conflicting
   duplicates and nonregional families with a source diagnostic.
2. Carry compact annotations on semantic operations/regions and target sites
   through inlining, copies and arena renumbering. Search must keep pinned sites
   fixed, including zero/canonical pins, with independent TOML permissions and
   retained artifact checks. Native ignores JavaScript syntax preferences.
3. Document source scope, supported alternatives, gates and cost. Test complete
   public entry points, imported/inlined bodies, conflicting neighboring regions,
   replay, syntax editions and admission cleanup after this coherent batch.
   Compare affected raw/gzip/Brotli artifacts; no full-library/fleet rerun.

Prediction: no annotations preserves bytes and search opportunities. Explicit
pins can grow files and reduce explored alternatives. New provenance is bounded
by semantic/target node counts and admitted through the existing owners.

Scope decision: a regional annotation cannot coherently fix whole-program
naming, property allocation, declaration ordering or a cross-region storage/
helper equivalence class. Those keep their existing TOML and family controls.
The public author registry exposes only region-owned spelling alternatives;
unsupported families are diagnosed, never silently treated as regional pins.

## Result

Implemented all twelve documented spelling families. Pins travel with shared
operations/regions, constant/string-family provenance and target sites through
inlining, specialization, helper sharing, pooling and arena renumbering. Search
excludes pinned alternatives; source, direct formation and retained-artifact
admission enforce the resolved tactic permissions. Pooling and shared helpers
keep conflicting quote classes separate. Quote printing obeys a selected
delimiter even on a size tie. Unannotated output keeps the existing heuristic.

Pin `/home/azureuser/lilscript-work/bin/s4-choices-2/lilscript`, SHA-256
`c6b3a26f3bdfbdaf8e735b4e5665b3b0cb7e4aa6c9ba00ba29502424e2b0fe11`.
Algorithm 40 / search 32 / walk 10 / local facts 14; string family 2.
[identity.json](identity.json) records the exact source and compiler identities.

The final affected batch passes 52 checks: 51 on the first run and the one
native string check after supplying the existing Clang path. Earlier focused
runs found and fixed missing rule-free inspection support. The first CLI
comparison exposed a quote-printer tie that ignored an otherwise retained pin;
the final runner asserts the actual delimiter, branch and modern spellings as
well as behavior. It supersedes the first pin. The release build, schema check,
24 independent Node artifact oracles and scored/delivered-byte checks pass.
No full-library or fleet rerun was performed.

| Matched program | Raw before → after | Gzip before → after | Brotli before → after |
|---|---:|---:|---:|
| Unannotated control | 81 → 81 | 90 → 90 | 73 → 73 |
| Pinned return branches | 110 → 120 | 94 → 106 | 71 → 82 |
| Imported/inlined quoted helper | 70 → 70 | 81 → 81 | 57 → 57 |
| Modern spellings | 101 → 97 | 93 → 93 | 78 → 78 |
| **Total** | **362 → 368 (+6)** | **358 → 370 (+12)** | **279 → 290 (+11)** |

[comparison.json](comparison.json) retains artifacts, hashes and work counts.
Baseline sources remove only unsupported `@choose` attributes. The control's
bytes and judgments are identical. Modern-site judgments fall 25 → 13 raw,
3 → 1 gzip and 46 → 26 Brotli; author pins reduce choices but can cost bytes.
This is an explicit author tradeoff, not an automatic compression improvement
or a timing/fleet claim. No ratchet baseline or port artifact changed. R15 is
complete within its documented regional scope; S4 public erased ABI remains
open.
