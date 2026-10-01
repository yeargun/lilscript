# S4 field-initialization contract

1. Replace the lint-only syntactic assignment guess with one checker-owned,
   metered constructor flow fact. Follow branch joins, early completion,
   exceptional/finally paths, loops and reads/captures of `this` before the whole
   instance is initialized. The migration fix and strict checking consume it.
2. Add `[language] field_initialization = "explicit" | "legacy"`. Legacy is the
   compatibility default during port migration; explicit requires the R3
   contract on every public frontend and target independently of effort. Record
   it in resolved policy and fingerprint. Unsupported/unknown controls fail.
3. Keep the staged port/default transition with V2: making S4 wait for the V2
   releases that depend on S4 creates a cycle. S4 owns implementation, diagnostic
   and fix-it qualification; V2 owns qualified port adoption and the default
   transition. No requirement is deleted and no old program silently changes.

Prediction: source diagnosis becomes sound for early returns and observations,
without a second proof in lint. This is a semantic contract, not a compression
heuristic; no byte improvement is assumed. Focused checker/lint/configuration and
JavaScript/native public-route checks follow the coherent implementation.

## Result

Implemented with shared checked field identities; the old lint-only assignment
heuristic is deleted. Config parsing, public check/compile paths, imported-module
diagnostics, resolved policy/fingerprints and the documentation schema share the
new contract. The staged default transition remains D3/V2 work.

Six focused field checks pass, covering thirteen constructor-flow cases,
target/effort independence, imported sources, configuration, and both new and
existing migration fixes. The linked index batch contributes six further
focused checks. Initial build/test-fixture errors and their corrections are
retained in logs; no full suite was rerun. CLI/lint/LSP release builds pass.

Pin `s4-field-contracts-1`, SHA-256
`27534025c90d306509f8a9491e85a9ab47980552d2ec2f41d1a159773d6e7302`, algorithm 24,
search 31, walk 10, local facts 10. [Identity](identity.json) records compiler and
source hashes. [Comparison](comparison.json) retains configs, artifacts, search
work and independent host observations: the matched class is unchanged at
243 raw / 173 gzip / 139 Brotli under both legacy and explicit contracts. The
new checked-indexing oracle passes each objective. Every selected score equals
the delivered artifact's canonical metric. No fleet, timing or Closure claim.
S4 remains active.
