# Q2 normalization reuse and admitted expression ordering

1. Replace expression-arena cloning during target renumbering with an admitted
   postorder/permutation plan and in-place moves. Admit the complete plan and
   work before mutation; keep provenance, authored choices, observations and
   journal identities aligned. Remove discarded public-to-the-module remap
   results; release the plan on success, refusal and unwinding.
2. Track local source normalization dependencies by immutable unit revisions.
   Revisit edited/newly-created units in the same structural order, while global
   consumers remain conservatively invalidated. Avoid effect analysis for rules
   that use only local control flow. Retain physical visit/reuse telemetry.
3. Classify target rule dependencies and reuse stable answers only while every
   declared input is unchanged. Keep full invalidation for cross-function and
   interface-sensitive rules. Dense scheduling remains an audit mode.
4. Follow the coherent implementation with focused dense/reused semantic,
   strictness, provenance, journal and admission/refusal checks. Rebuild and
   measure affected artifacts under raw/gzip/Brotli, not the full library suite.

Prediction: unchanged unlimited-resource output, fewer local visits and no
expression payload duplication while reordering. Newly counted scratch work
and storage can expose previously unaccounted resource refusals. Q2's remaining
formation, build/elaboration cache, decision lock and other admission owners
remain open; this batch is not a milestone-completion claim.

## Result

Implemented and qualified as a Q2 batch. Pin
`/home/azureuser/lilscript-work/bin/q2-normalization-1/lilscript`, SHA-256
`c514eca8a6cb6392a54512d489a20d571f031e2d60dd764a4b1bfa2ee11b3e15`;
algorithm 46 / search 32 / walk 10 / local facts 15.
[Source identities](identity.json), [comparison](comparison.json) and
[reproduction script](measure.mjs) retain inputs and final artifacts.

The old renumber implementation and unused escaping remap results are deleted.
A fully admitted postorder/permutation plan precedes mutation. In-place swaps
retain the original expression backing and nested argument/array allocations;
a 100-reordering fixture verifies their pointers and complete plan release.
Refusals preserve the tree, and journal/provenance/site identities survive moves.

Local source returns and unreachable cleanup use immutable body revisions,
including created-body status for reachability. They no longer construct effect
graphs they do not read. Target literal folding and undefined/unreachable
cleanup retain stable proofs with complete explicit dependency masks. Arena
renumbering invalidates every target domain. Replayed work uses the same charge
calls as a cold stable pass, including partial work refusals; journal admission
and cache bookkeeping stay identical in both modes. Strict TOML
`cache.normalization_reuse` controls these local proofs and stays outside the
semantic fingerprint; `--cache off` also disables them. Global/SCC consumers
remain conservatively invalidated and are unfinished Q2 work.

35 distinct focused tests pass, covering independent source/Node oracles,
strictness, capture/activation and native lifetime, dense/reused decisions,
all-objective search parity, work/memory refusals, key invalidation, site/journal
remapping, array packing and mention-cache lifetimes. The initial 35-test run
passed 34 and failed one because the command omitted the installed Clang path;
only that native test was rerun with Clang 18.1.3 and passed. No full-suite run.
The debug build took 58.70 seconds; the final release took 59.62 seconds.

All 36 release artifact/runtime oracles pass (four controls × three objectives
× previous/reused/disabled). Every file is byte-identical to the prior pin;
selected totals remain **888 raw / 691 gzip / 574 Brotli**. Current reuse-on and
reuse-off builds have identical policy fingerprints, logical work, selected
files and search counts. Source unit selections fall 330 → 180 across the
12 current builds; target physical passes avoid 77 executions.

| Case | Source selections: disabled → reused (each objective) | Target passes reused: raw / gzip / Brotli |
|---|---:|---:|
| Spellings | 36 → 20 | 1 / 1 / 1 |
| Local returns | 12 → 8 | 0 / 0 / 0 |
| Captured activations | 46 → 22 | 6 / 10 / 58 |
| Delivered setter | 16 → 10 | 0 / 0 / 0 |

Negative evidence: target reuse is modest and absent in two controls. One coarse
timing triplet per case/objective shows no clear CPU gain; some readings grow
by .01 seconds. Reported peak retained bytes remain dominated by existing
owners. Neither a general speed/memory claim nor a compression win follows
from these samples. No ratchet baseline or port artifact changed.

Q2 remains active: SCC/global dependency invalidation, shared/incremental
formation, build/elaboration caching, decision replay and remaining storage
admission still need implementation.
