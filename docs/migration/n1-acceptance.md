# N1 native integration

N1 follows D3 (`dbcb72f6`). N2 owns the remaining
language implementations; unsupported native features must be reported honestly
and with their source locations throughout N1.

1. Keep the existing shared semantic pipeline as native's optimized input.
   Qualify folding, dead storage, calls, scalar replacement and final-use
   transfers through public source and module builds, including `all` requests
   and tactic vetoes. Do not add a second optimizer or an emitted-C rewrite pass.
   Native global guards consume the common initialization/effects proof;
   possible early callback/reentry accesses retain the guard. The legacy
   exception-status and ABI-dependent specialization consumers remain N2,
   whose runtime/ABI protocols and performance gate they depend on.
2. Move C toolchain invocation out of the CLI into one library owner. Resolve
   `[target.native]` controls, compiler paths, strict numeric flags and host
   sources consistently. Record compiler, arguments and input/output identities;
   report subprocess errors without pipe deadlocks or replacing a good output
   with a failed build. Document each control's use, defaults and tradeoffs.
   Ordinary case/Rust execution uses this owner; independent numeric/profile
   oracles explicitly preserve their different compiler flags.
3. Supply target-aware checking through the same native admission used by
   compilation. Keep backend validation for edited programs, but report source
   refusals with their original module/span, including representation/ABI
   constraints. Checking must not execute a C toolchain or optional JS search.
4. Publish a native capability/ABI matrix distinguishing implemented features,
   N2 work, and declared JavaScript-only facilities. A successful source check
   for JavaScript alone makes no native support claim.
5. Batch focused configuration, process-failure, target-diagnostic and shared
   semantics checks. Record exact verification limits and identities, update
   index/history/coverage, commit and push before beginning N2.

Expected effect: no JavaScript byte changes. Native retains the existing shared
optimization wins. Target checking pays native admission once when requested;
ordinary builds must not add a duplicate preflight. Native compiler optimization
selection is a toolchain prerequisite implemented here; N2 owns performance and
cross-target qualification. No executable-size or speed win is claimed from a
compiler flag alone.

## Result

Complete under this contract. [Evidence](../../benchmarks/migration-results/2026-10-02-n1-native/README.md)
records the focused semantic/driver/CLI gates, all 403 target checks, capability
mask inventory and exact compiler identities. Source diagnostics are qualified;
four obsolete TOML fixture controls remain explicitly V1. No full library or
native-performance qualification is claimed. N2 is next.
