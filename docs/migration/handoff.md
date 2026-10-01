# Migration handoff — G3 complete, S4 next

The owner requests the full remaining plan, completing one milestone before the
next. Commit/push authorization persists. Use coherent implementation batches
and focused checks; do not resume S1's deferred full-library/CLI or CPU study.

Checkout `/home/azureuser/lilscript`, branch `finer/059-idiom-directed-naming`.
C1/C2/C3/S1/S2/S3/Q1/G1/G2/G3 are implemented. The [G3 contract](g3-acceptance.md)
and [evidence](../../benchmarks/migration-results/2026-10-01-g3-complete/README.md)
pin `/home/azureuser/lilscript-work/bin/g3-complete-2/lilscript`, SHA-256
`8550769ffd9b5d4eb4b45b77e6a64309d1bb8c4c52bba49b19577f200b608f35`,
algorithm 19 / search 31 / walk 10.

G3 supplies stable per-site spellings, modern syntax with edition guards,
prepared printer forms, safe ordering and receiver aliases. Twenty-five focused
checks plus 30 objective cells pass. Small new cases save 48 raw / 7 gzip /
7 Brotli; the deliberate effort-14 locality case saves another 28,371 gzip.
Six G2 boundaries are unchanged. Runtime oracles pass, but the controlled timing
verdict is inconclusive. The spelling case's 48 → 292 judgments for six Brotli
bytes is explicit cost evidence for Q2/Q3, alongside markedlil's original growth.

1. Complete **S4 only**. Record its finite contract, then fix the frozen C3
   `benchmarks/calibration/corpus/records-128.lil` refusal:
   `semantic operation type mismatch: IntBinary(Add)`. Q1/G1 reproduce it even
   with inlining/scalar replacement off; 64 helpers pass and 128 fail. G1 retains
   the source/diagnostic. `LILSCRIPT_DEBUG_VERIFY=1` prints the failing types.
2. Close S4's identity/default/operation/fact-consumer work and supported language
   crossings. Keep existing `ref`, constructibility and `pure` termination
   contracts unless an explicit language amendment authorizes a change. Delete
   replaced analyses only together with coverage-preserving consumers.
3. Continue **Q2 → Q3 → Q4 → D1 → D2 → D3 → N1 → N2 → V1 → V2**. One active
   milestone; commit and push before advancing. D1 inspects
   `/home/azureuser/lilscript-work/portwork/m3.3b-wip.patch`.

Raw/gzip/Brotli remain independent. Default effort 13 favors size; measured
costly marginal exploration may default to 14/15 with explicit controls. Every
flag documents purpose, gates, vetoes and size/compile/runtime costs. Q3 owns
more complete joint exploration when one spelling exposes another.

No fleet or native-complete claim. S3's captured-activation +2 Brotli remains
negative evidence; no ratchet baseline or port artifacts changed. One heavy job
at a time. No build/test/measurement job is left running at this checkpoint.
