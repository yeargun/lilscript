# S4 erased variant contracts

1. Extend payload domains to applied generic classes/shapes when the closed
   union contains one instantiation of each nominal identity. A runtime test
   identifies the nominal, never a type argument; same-identity alternatives
   remain ambiguous and are refused.
2. Admit alpha-equivalent generic virtual overrides. Keep the receiver's
   checked instantiation on the one prepared call and adapt native callable
   slots only when their actual C representations differ.
3. Check generic base substitutions, method binders, public module routes,
   defaults and native boxing together after implementation. Preserve the
   existing non-generic cases without another complete library run.

Prediction: JavaScript keeps erased type arguments and one nominal identity,
without per-instantiation metadata. Native retains its existing tagged generic
ABI; a concrete overriding body may need an adapter. No compression win or
runtime parity is assumed from adding the capability.

4. Keep directly suspending payload arms in their original function. Expression
   bindings captured by closures use a fresh carrier at each initialization;
   a synchronous closure factory snapshots that carrier. Shared writes and
   transitive captures retain their one lexical binding without extra promise
   scheduling. This required lowering has no optional optimization switch.

## Result

Four focused `s4_erased_variants_` checks pass as one group (35.41 seconds):
JavaScript raw/gzip/Brotli, native generic dispatch/boxing, concrete overrides,
renamed method binders, generic class/tagged-shape payloads, ambiguous identity
and override diagnostics, ordinary await order, and captured await loops.
The independent async reference compares promise events, escaped readers and
writers, nested closures and loop conditions. The earlier payload-scope/native
identity test also passes (7.82 seconds). CLI release build passes.

Pin `/home/azureuser/lilscript-work/bin/s4-erased-variants-1/lilscript`, SHA-256
`bd6ca8311a8e0b358243710b402e3d95e01970bf2e4fbec722e38eb4c128c33d`. Algorithm 32 / search 32 / walk 10 / local facts 14.
[Compiler/source identities](identity.json), [comparison](comparison.json),
[runner](measure.mjs) and `artifacts/` retain the exact inputs and outputs.

The unchanged static-inheritance and captured-payload controls total **−20 raw,
+7 gzip, −1 Brotli** against the preceding variant pin, using each objective's
own output. Static inheritance stays identical; fresh-carrier closure formation
changes the captured-payload spelling. The gzip loss is retained as negative
evidence. The new generic-dispatch capability has no valid old-compiler
comparison. Every independent behavior oracle and delivered/score comparison
passes. No fleet, Closure, timing or native-completion claim; no ratchet or
port artifacts changed. S4 remains active for R17/R18 and the consumer audit.

Logs: `/tmp/lilscript-s4-erased-variants-tests4.log`,
`/tmp/lilscript-s4-erased-variants-scopes.log`,
`/tmp/lilscript-s4-erased-variants-release.log` and
`/tmp/lilscript-s4-erased-variants-measure.log`.
