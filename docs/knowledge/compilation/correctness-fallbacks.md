# Correctness, fallback, and determinism

Parent: [compilation](README.md). Verification:
[verification](../verification/README.md). Source anchors:
`optimize_and_select_javascript`, `finalize_javascript_candidates`, diagnostic
rendering in `src/compiler.rs`, SSA validation in `src/optimizer.rs`, and the
independent evaluator in `src/interpreter.rs`.

## Fallback hierarchy

1. Frontend, linking, semantic, or configured lowering/optimizer failures are real
   errors with source diagnostics.
2. The configured optimizer/emission is retained as the baseline candidate.
3. An optional optimizer variant that fails to optimize/emit may be discarded without
   invalidating a program whose configured path succeeds.
4. Candidate admission/startup limits may reject an alternative; they do not erase
   the configured baseline.
5. A parsed peephole clone must parse and validate as a complete generated artifact.
6. If all optional alternatives lose, emit the configured baseline.

Search never repairs type errors or weakens host boundaries.
`[optimization].foo = false` remains a hard off. An exact compression allowlist
usually prevents omitted tactics, but current search-only overlays and the
unconditional `elide_length_tonumber` flip are documented exceptions.

Selection is deterministic for fixed compiler/source/config/profile/toolchain:
priority rank, raw length, top-level declaration preference, startup score,
lexical JS, and stable plan identity provide total tie-breaking. Terminal
topology-preserving search may first prefer more resolved one-byte bindings.
Bundle names use source identity and content hashes; manifests record exact output
sizes and dependencies.

## Semantic oracles

- Rust unit/conformance tests pin parser, semantic, IR, optimizer, codegen, config,
  package, and chunk behavior.
- `scripts/verify-matrix.sh` compares JS/C/native execution.
- `lilscript-differential` interprets checked AST without SSA, targeting shared
  transform bugs.
- Paired web cases compare independently authored JS and LilScript only after both
  pass the observable oracle.

A size win never overrides a semantic failure. A compiler crash, invalid emitted JS,
nondeterministic artifact, or baseline-only mismatch is a red result with preserved
artifacts for triage. Search ranking an unresolved binding is the same class of
failure ([ident-05](../migration/board/notes/ident-05.md)); it is not a smaller
program. [Current architecture](current-architecture.md) treats identity as blocking further search
widening.

SSA validation includes edge uses. In particular, aggregate scalar replacement may
not delete a struct definition merely because all instruction uses are field reads;
a phi incoming edge is also a live use. Until field-wise phi decomposition exists,
the pass retains such aggregates. The structural algorithm corpus owns a loop-carried
aggregate regression in addition to the optimizer-level dangling-definition test.
