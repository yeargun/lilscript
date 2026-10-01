# Migration handoff — S4 active

The owner requests the entire remaining plan, one milestone at a time. Commit
and push are authorized. Implement substantial coherent batches, then run
focused checks. Do not resume the deferred S1 full-library/CLI or CPU study.
One heavy job at a time.

Checkout `/home/azureuser/lilscript`, branch `finer/059-idiom-directed-naming`.
C1/C2/C3/S1/S2/S3/Q1/G1/G2/G3 are implemented. **S4 remains the sole active
milestone**, under its [acceptance contract](s4-acceptance.md).

The latest [S4 field batch](../../benchmarks/migration-results/2026-10-01-s4-field-contracts/README.md)
and [index batch](../../benchmarks/migration-results/2026-10-01-s4-index-contracts/README.md)
pin `/home/azureuser/lilscript-work/bin/s4-field-contracts-1/lilscript`, SHA-256
`27534025c90d306509f8a9491e85a9ab47980552d2ec2f41d1a159773d6e7302`.
Algorithm 24 / search 31 / walk 10 / local facts 10. One checker-owned constructor
flow replaces the lint heuristic and supports `[language] field_initialization`
`explicit`/`legacy` on every public frontend, independently of effort. Legacy
remains the migration default; D3/V2 own the qualified port/default transition.
Native string/code-unit/all binary reads trap; binary `.get` returns nullable;
required development/native traps survive unused results and helper inlining.
Twelve distinct focused checks pass across the batch, including seven native
compiler/sanitizer profiles for positive indexing. CLI/lint/LSP release builds
pass. One matched class boundary is unchanged at 243 raw / 173 gzip / 139 Brotli;
new bounds oracles pass all objectives and scored bytes match delivered bytes.
Next: remaining S4 module boundaries, language/catalog and shared-consumer work.

The preceding [S4 host batch](../../benchmarks/migration-results/2026-10-01-s4-host-boundaries/README.md)
pins `/home/azureuser/lilscript-work/bin/s4-host-boundaries-1/lilscript`, SHA-256
`0167a9299329bd439f1a32e39074bf05dd065a8de7b506eee635ffbbad8cc073`.
Algorithm 22 / search 31 / walk 10; shared local facts version 9.
Callable defaults/rest/receivers and generic/nullable `JS.assume` now use the
common struct codec. Zero-use `object` singleton declarations and their
AST/checker/module/LSP special cases are removed. The lexer census covers 27
maintained workloads and 2,707 sources. Seventy distinct focused checks pass
across the batch, including one failure-driven metadata correction. One matched
callback costs +18 raw, −1 gzip and −3 Brotli; the new callback host oracle passes
script/module under each objective. CLI/LSP release builds pass.

The preceding [S4 boundaries batch](../../benchmarks/migration-results/2026-10-01-s4-boundaries/README.md)
pins `/home/azureuser/lilscript-work/bin/s4-boundaries-1/lilscript`, SHA-256
`b80138abcdf0504f274219880b631387230e97e4b50c63d032cc35b26fb68b26`.
It implements concrete generic struct schemas, nested type closings,
nullable/default/rest public exports, constructor/prototype adapters, and dual
type/constructor imports/re-exports. Next: the remaining S4 language,
boundary/catalog and shared-consumer work; keep staged breaking migrations
explicit rather than silently flipping port semantics.

The prior [S4 contracts batch](../../benchmarks/migration-results/2026-10-01-s4-contracts/README.md)
pins `/home/azureuser/lilscript-work/bin/s4-contracts-2/lilscript`, SHA-256
`36458f95ec83d67f85c2cfdca1703dc716fa6af07b8067acfd6dd77f7edc64e2`.
Algorithm 20 / search 31 / walk 10; shared local facts version 9.

Implemented: statement and binder identities, canonical checked expression
types, callee-owned default expressions, shared primitive/target binding facts,
check-time native capability diagnostics, checked array reads and native array
traps, `with`, generic methods, typed receiver/rest declarations and calls,
default/rest composition, public arity, and evaluation-ordered array spread.
The frozen `records-128` refinement bug is fixed. Thirty-three focused S4 checks
passed through the batch, plus existing contract/admission/default groups.
Five matched G3 workloads have unchanged sizes for each independent objective;
the frozen record oracle passes all three objectives. These size/oracle results
belong to pin 1. Pin 2 fixes a forwarded extern-alias verifier refusal and passes
its focused regression and the eight Object.hasOwn script/module lanes. The
initial negative result is retained and the stale expected-failure entry removed.

1. Continue **S4**, closing generic/host transport, shapes/spread, ABI enums,
   variants/sealed virtuals, boundary normalization, definite-assignment and
   indexing migration, module/catalog gaps and the shared-consumer audit. Do not
   mark a language feature complete after adding only syntax or unused facts.
2. Keep the current `ref`, exported constructibility and `pure` termination
   contracts: Y2/Y3/Y4 have no approved semantic amendment. Breaking migration
   rules require their port patches and qualified transition, not a silent flip.
3. After S4 closes, continue **Q2 → Q3 → Q4 → D1 → D2 → D3 → N1 → N2 → V1 → V2**.
   Commit and push each completed milestone. D1 inspects
   `/home/azureuser/lilscript-work/portwork/m3.3b-wip.patch`.

Raw/gzip/Brotli remain independent. Default effort 13 favors size; costly
marginal exploration may default to 14/15 with explicit overrides. Every flag
must document purpose, gates, vetoes and compile/runtime/size tradeoffs.
Q2/Q3 retain markedlil's candidate growth and G3's 48 → 292 spelling judgments
for six Brotli bytes as negative cost evidence. S3's captured-activation +2
Brotli remains negative size evidence. No fleet, Closure or native-complete
claim. No ratchet baseline or port artifact changed in this batch.
