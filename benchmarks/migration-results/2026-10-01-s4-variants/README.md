# S4 payload matches and sealed dispatch

1. Express payload sums with the existing nominal union types. Add
   `match(value) { Text(text) => text.content, Break(br) => br.width }` for
   closed class/tagged-shape unions. Check identity, disjoint tag domains,
   scoped payload bindings and exhaustive coverage once. Reuse the existing
   type tests, checked views and value-region lowering on both targets.
2. Add sealed class declarations and virtual/override method contracts.
   Unsealed methods retain static dispatch. Closed virtual calls lower through
   the known implementation set, preserving receiver/argument/default order,
   refs, effects and shared optimization. Exact receiver sites can remain
   static. Runtime identity and public constructor observation remain distinct.
3. Keep one checked contract across build/check/lint/LSP. Unsupported erased
   runtime distinctions receive source diagnostics; wider generic transport
   stays within S4's R18 work, native shapes remain N2. No effort selector may
   change dispatch or match meaning.
4. After the coherent implementation, check payload scopes/exhaustiveness,
   effects, inheritance, public entry points and shared/native behavior. Measure
   unchanged controls separately from new source capabilities under each
   objective. No whole-library/fleet rerun.

Prediction: typed payloads remove handwritten tag casts and dispatch scaffolding.
Closed dispatch can expose ordinary calls to the shared optimizer. Polymorphic
sites may need type tests and repeated calls, increasing code; preserve exact
behavior and let the existing objective choices judge resulting code. Generic
or private-layout alternatives must never invent a runtime type argument.

Implementation includes a private closed-identity operation whose JavaScript
witness is initialized before user module evaluation and registered when the
class definition executes. This avoids reading a later subclass's source
binding through its TDZ. Native uses the existing nominal identity table.
Selected method values share one argument schedule; callable view conversion
preserves the native object-pointer ABI.

Focused checks: six `s4_variants_` cases pass for payload scopes (including
escaping per-iteration closures), tagged shapes, default/rest contracts, ref
argument evaluation, negative contracts and an independent ABI enum model.
The public constructor/prototype and module-graph case passes separately after
correcting its import/re-export fixture. JavaScript checks cover raw/gzip/Brotli
and production/development; native applications cover efforts 0 and 13.
The initial source-binding omission, expression-region cycle and native-library
fixture mistakes were corrected. No full library suite was run.

Directly suspending payload arms and erased generic virtual dispatch are still
source-refused. This is an implementation batch within S4, not completion of S4
or the migration. Broad size/runtime qualification remains V1.

Release pin: `/home/azureuser/lilscript-work/bin/s4-variants-1/lilscript`, SHA-256
`f248fc320df0af85d647aa749cca66bf429f0b4f60bb92dd22a6ff462683e7fe`;
algorithm 31 / search 31 / walk 10 / local facts 14. [Exact measurements](comparison.json)
retain each objective's artifact and search counts. The unchanged static
inheritance control stays **53 raw / 72 gzip / 56 Brotli**, with unchanged
judgments. The new payload/dispatch source measures 595 raw / 321 gzip / 283
Brotli under its independently optimized objectives, and passes its oracle.
It was not accepted by the baseline compiler; these are capability sizes, not
compression wins. No timing, fleet or Closure claim.
