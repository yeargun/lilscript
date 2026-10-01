# S4 author controls

1. Implement checked `inline for` expansion in shared source conversion, with
   fresh cells and closure capture banks for every iteration. Preserve nesting,
   returns, exceptions, and exact scalar values under JavaScript and native.
   Admit expansion work/storage before allocation. A dedicated TOML permission
   controls authored unrolling independently from private function inlining.
2. Carry region policy into source and target choice ownership. Implement the
   existing `@pool` contract and explicit `@choose` alternatives with diagnosed
   conflicts, hard TOML vetoes, stable provenance and replay behavior.
3. Document each control's useful situations, default, cost/size/runtime effects
   and source-pin precedence. After the coherent batch, run focused scalar,
   capture, module, native, permission and replay checks and matched objective
   evidence. No full-library/fleet rerun.

Prediction: programs without author directives retain their current choices.
Unrolling trades compile time and text growth for exposed constants and removed
loop machinery; a pin need not improve compressed size. More effort never grants
new semantics or runtime permissions. Explicit conflicts fail with source spans.

## Inline-for batch

Shared source conversion now expands `inline for` in source order, with fresh
physical cells for each occurrence and nested closure parameter/local banks.
It accepts the declared scalar type as context, including empty and nullable
lists. Outer captures retain their cells; nested iterations retain separate
banks. `loop-unrolling` is independent of inlining, enabled for explicit pins
at every effort, and diagnoses hard-off conflicts. Its provenance survives
source edits, candidate replay, artifact qualification and native formation.
The checker's constant-list predicate no longer allocates a discarded vector.

The native array-push admission now uses the existing callable adapter when a
function with defaults is stored under a required-parameter callable view. The
emitter already implements this adaptation and owns its retained environment.

Ten focused checks pass, including all three objectives at efforts 0 and 13,
native runtime observations, empty/scalar/nullable lists, mutable nested
captures, function parameters, hard-off diagnostics, replay/qualification,
resource refusal cleanup and shared descriptor ownership. The schema check and
release CLI build pass. No full-library or fleet rerun was performed.

Four matched CLI programs pass independent oracles under every objective. The
ordinary-loop control is byte-identical. Selected totals change from
**552 to 674 raw (+122), 522 to 474 gzip (−48), and 418 to 369 Brotli (−49)**.
This is evidence for an explicit expansion pin, not a policy claim that unrolling
always improves size. Configuration deliberately disables function inlining,
folding and scalar replacement in these comparisons. Resource growth is charged;
no compilation-speed or global-optimum claim is made.

Pin `s4-inline-for-1`, SHA-256
`a883f9860bcc5389ab35350579e2f06854aa55d23050cefa6dd67153306b2a4d`.
Algorithm 38 / search 32 / walk 10 / local facts 14. Exact configurations,
artifacts, work counts and identities are in [comparison.json](comparison.json)
and [identity.json](identity.json). Steps 2–3's `@pool`/`@choose` implementation
remains S4 work; this record does not mark the whole author-control task complete.
