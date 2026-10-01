# S4 declarative host catalog

1. Preserve generic extern class schemas, inherited substitutions and host
   constructor contracts through source conversion. Host methods and fields
   retain their declared names and exact receiver evaluation.
2. Ship compiler-owned ECMAScript and DOM declaration modules through ordinary
   checked module interfaces. Explicit imports add typed host bindings without
   executable initialization; native capability diagnostics retain source spans.
3. Move the host property inventory under the operation catalog and use that
   single owner for host-name reservations. Remove its stale duplicate owner.
4. Move the existing unsupported direct-eval contract to checking, before source
   conversion, with the declaration's original span.

Prediction: importing declarations alone produces no runtime code. Generic
extern types erase without additional metadata; existing explicit declarations
keep their output. Catalog support is a maintained declared surface, not a claim
of complete Web IDL generation. Focused module, receiver, generic-constructor and
negative capability checks follow the coherent implementation batch.
5. Make selected host bindings explicit in TOML: JavaScript paths for declared
   extern globals/functions/classes and native callback-provider symbols for
   declared extern functions. Resolve and fingerprint these semantic bindings
   before optional search; preserve the existing declared type/effect contract.
   Expose the native host header through the build API and C delivery, with
   configured C provider sources for executable builds. Invalid or missing
   native providers fail while source declarations are available.


## Result

Six `s4_host_catalog_` checks pass as one group (3.72 seconds after compilation):
generic extern inheritance/subclassing, shared catalog modules, unused-import
output, host receivers, alias identity/call ordering at effort 0/13 under all
three objectives, policy fingerprints, invalid/conflicting mappings and native
provider delivery. The extended native-provider test passes separately (0.49
seconds), including generic int/string/managed-array transport across a separate
C translation unit. That extension found a missing public `ls_value` definition;
the header and generated C now share its layout and retain/release implementation
in `native_value_interface.h`, with opaque object/array declarations before use.
The CLI release build passes. Schema generation/check and `git diff --check` pass.

Pin `/home/azureuser/lilscript-work/bin/s4-host-catalog-1/lilscript`, SHA-256
`a20feebf53589c898ddf56e3a6ce22e9a0252a0f164ef9b80733233857b02640`. Algorithm 33 / search 32 / walk 10 / local facts 14.
[Compiler/source identities](identity.json), [objective comparison](comparison.json),
[runner](measure.mjs), [CLI provider receipt](native-cli.json) and
[CLI runner](native-cli.mjs) retain reproducible inputs, outputs and oracles.
Source identity includes `.lil` and `.h` catalog/runtime assets as well as Rust.

The ordinary-host control stays **56 raw / 74 gzip / 57 Brotli** using each
objective's independently optimized output. New catalog/configured-generic
capabilities pass their independent behavior oracles; their sizes are not old-
compiler wins. Every scored/delivered comparison agrees. Native CLI C/header
and executable delivery link the TOML-relative provider and print the expected
result. No full-library/fleet rerun, timing comparison or native-complete claim.

The source checker owns selected extern identities; optional transforms cannot
veto or replace host bindings. No per-instantiation JavaScript metadata is added.
The host property inventory now has one catalog owner, with the existing public
Rust module retained as a compatibility re-export. Direct eval's existing
refusal moves from conversion to its source declaration. The catalog is a
maintained surface, not complete generated Web IDL. Wider product/union ABI and
the fact-consumer audit remain S4; delivery/conditions remain D1–D3, additional
native capabilities N1/N2. No ratchet or port artifact changed.

Logs: `/tmp/lilscript-s4-host-catalog-tests4.log`,
`/tmp/lilscript-s4-host-catalog-native2.log`,
`/tmp/lilscript-s4-host-catalog-release.log`,
`/tmp/lilscript-s4-host-catalog-measure.log`,
`/tmp/lilscript-s4-host-catalog-cli.log`.
