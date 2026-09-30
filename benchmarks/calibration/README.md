# Calibration and protected evaluation

`generate.py` creates eight medium training modules in four families and three
separate algorithm libraries. Each exports input-dependent operations; expected
results come from independent Python formulas, a full dynamic-programming
matrix, a linear interval reference, or Python's native CRC implementation.
Sources range from 10 to 300 KB in training. Reports also show emitted sizes:
compiler simplification is part of the measurement, not padding to meet a size.

Generate once into a new directory and commit the frozen manifest and sources:

```sh
python3 benchmarks/calibration/generate.py --output /absolute/new/corpus
python3 scripts/calibrate.py --manifest /absolute/new/corpus/manifest.json \
  --compiler /absolute/pin/lilscript --codec /absolute/pin/lilscript-codec \
  --work /absolute/new/training --json /absolute/new/training.json
python3 scripts/calibrate.py --manifest /absolute/new/corpus/manifest.json \
  --compiler /absolute/pin/lilscript --codec /absolute/pin/lilscript-codec \
  --split evaluation --training /absolute/new/training.json \
  --work /absolute/new/evaluation --json /absolute/new/evaluation.json
```

Run sequentially. The runner refuses changed sources, tools, policy or corpus
between training and evaluation. It removes hidden environment policy adapters,
checks every artifact's oracle, remeasures with the canonical codec, retains
explanations and requires audit/on artifact identity. Each on-lane explanation
warms the inputs before three ordinary compilation samples establish current
CPU cost. Audit/off diagnostic timings are not a paired speed comparison.

Each objective reports immediate proxy misses, final no-prune byte deltas,
judgments and resource work. Family-level leave-one-out folds choose on/off
using only the other families' exact sizes (ties retain on). A separate margin
estimate includes observed immediate misses; it is not a simulation of the
different future path. An empty miss list never establishes a universal bound.
Pruning off still leaves effort, structural and resource ceilings in force;
it does not enumerate a global optimum.

The evaluation set is held out from parameter selection, not blind to its
author. The manifest records exposure of both the new workloads and previously
inspected ports. Training policy is frozen before evaluation. On an inspected
evaluation failure, retain the old receipt, move that workload and all its
derived variants into development in a **new version** of the corpus, and
replace its evaluation family before making another generalization claim.
Never silently regenerate a seed or tune against repeated evaluation results.
External-library qualification remains V1's responsibility.

The missing historical A1 CPU pair cannot be recreated from current timings.
Current repeated samples establish a new baseline only. Search defaults remain
unchanged here; Q3 owns calibration-driven policy changes.

`policy-provenance.md` records the generic legality and bounded policy behind
every retired NO3 entry. `scripts/lib/size-verdict.mjs` defines `exact-bytes-v1`:
one byte is a real deterministic difference, while robustness, runtime parity
and the compile cost of obtaining it remain separate evidence. Historical
reports keep their original thresholds and verdicts.
