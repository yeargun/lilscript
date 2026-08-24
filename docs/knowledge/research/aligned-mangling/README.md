# Aligned mangling: naming, the dictionary, and where the bits actually are

Parent: [research](../README.md). Prior work this builds on:
[brotli global-mangle playbook](../brotli-global-mangle/README.md). Format
reference and the tooling used here: [Brotli, the whole
machine](../brotli-machine.html).

This folder answers two questions that keep coming back, and it answers them by
taking real streams apart rather than by reasoning about them:

1. **Should mangled names be dictionary words instead of `a`, `b`, `c`?**
   No. Not for hot names, not for cold ones, not on any corpus. Every
   dictionary reference in a real stream of ours is used **exactly once**
   ([01](01-where-the-bits-are.md)), which is the whole mechanism
   ([03](03-dictionary-as-names.md)).

2. **Can naming be aligned across scopes so repeated shapes spell themselves
   the same way — `a[1]`, `a[2]` in both closures instead of `a[1]`, `b[1]`?**
   The idea is right about the *pattern* and wrong about the *headroom*.
   Functions that are twins up to renaming barely exist in real code
   (0–1 groups per corpus), ordinary frequency-ordered mangling already aligns
   the hot indexed receivers, and a greedy aligner scored with a codec-shaped
   cost model changes nothing it did not already do
   ([04](04-alignment.md)).

What the measurements *did* find is next to those questions, not inside them:

| Finding | Corpus | Δ Brotli-11 | Legal? |
|---|---|---:|---|
| A fold sinks a match-group read past the rebinding of its own variable — the `ident` class, in a shipped port | markedlil under `cost_model = "raw"` | (its −87 is not a win) | 2 CommonMark cases the other models pass |
| A ±3-byte Brotli tie is being broken toward the **920-raw-byte-larger** side | markedlil | −920 raw for free | measured per family |
| A 12-second grid search over 14 shipped artifacts from five libraries | jquerylil, motionlil, posthoglil, markedlil, solidlil | **−952** (−0.48%; up to −2.90% per module) | yes, every step proved and every winner behaviour-checked |
| Naming + pool order together, found by a 64-point grid | jquery-lil-raw | **−1,060** (−3.18%) | yes, 37/37 jsdom observations |
| Release names not referenced in a function's subtree | jquery-lil-raw | −801 (−2.41%) | yes, behaviour-verified |
| Same, on the published jquerylil package | jquerylil dist | **−770 / −776** | yes, 28/28 jsdom observations |
| Same, on solidlil's reactive core | solidlil | **−96** (−2.2%) | yes, 18/18 observations |
| Same, on markedlil's smallest build | markedlil | −37 | yes, 680/680 spec cases |
| Same, on the downstream-minified port | jquery-lil-min | −617 | yes |
| String-pool declaration order | jquery-lil-min | −70 | yes, free |
| Function-declaration layout, all six orders | every corpus | +13 … +433 | yes, and it loses |
| Dictionary words as names, cold (≤3 uses) | every corpus | +1017 … +2437 | yes, and it loses badly |
| Dictionary words as names, hot | every corpus | +1953 … +6341 | yes, and it loses badly |

The −801 is the headline: it is bigger than every row in the
[global-mangle playbook](../brotli-global-mangle/README.md), it is a legal
rewrite that passes a 37-observation behavioural differential against the
shipped artifact, and it was confirmed with `lilscript-codec`, not only with
the diagnostic scorer. [05](05-concentration.md) has the mechanism;
[PLAN.md](PLAN.md) has what to do about it.

## Pages

| Page | Question |
|---|---|
| [00 questions](00-questions.md) | The two questions, stated precisely, answered short |
| [01 where the bits are](01-where-the-bits-are.md) | A census of real streams: literals, copies, distances, dictionary |
| [02 the hardcoded library](02-the-hardcoded-library.md) | What is actually in the 122,784-byte dictionary, for a JS emitter |
| [03 dictionary as names](03-dictionary-as-names.md) | Why ROM words lose at every frequency |
| [04 alignment](04-alignment.md) | Twins, the LZ objective, the `a[1]` ceiling, layout |
| [05 concentration](05-concentration.md) | The win: fewer distinct names, and where it already lives in the compiler |
| [06 free order](06-free-order.md) | Emission orders that cost nothing to change |
| [07 ports](07-ports.md) | jquerylil, markedlil and solidlil: what each one is leaving, and the cost-model inversion |
| [08 search](08-search.md) | Do transform families add up? A factorial says yes — once you factor along the right axes |
| [09 the equation](09-the-equation.md) | The closed form, why its gradient is useless, the cheap re-solve, and −952 bytes across the shipped libraries |
| [RULES](RULES.md) | The compiler-facing rules, distilled, with the evidence behind each |
| [PLAN](PLAN.md) | The migration plan |
| [findings.html](findings.html) | The same evidence as one page, generated from the JSON (`node render-findings.mjs`) |

## Reproduce

```bash
node docs/knowledge/research/aligned-mangling/census.mjs         # stream census
node docs/knowledge/research/aligned-mangling/dict-view.mjs      # the dictionary, for JS
node docs/knowledge/research/aligned-mangling/twins.mjs          # twins up to renaming
node docs/knowledge/research/aligned-mangling/experiments.mjs    # the mutation table
node docs/knowledge/research/aligned-mangling/concentration.mjs  # namings vs proxies
node docs/knowledge/research/aligned-mangling/pool.mjs           # string-pool order
node docs/knowledge/research/aligned-mangling/layout.mjs         # function order
node docs/knowledge/research/aligned-mangling/indexed.mjs        # the a[1] ceiling
node docs/knowledge/research/aligned-mangling/ports.mjs          # the three ports
node docs/knowledge/research/aligned-mangling/port-differential.mjs  # and their behaviour
node docs/knowledge/research/aligned-mangling/costmodel.mjs      # markedlil's configs, size + correctness
node docs/knowledge/research/aligned-mangling/shapediff.mjs <a.js> <b.js>   # why is one bigger
node docs/knowledge/research/aligned-mangling/families.mjs <artifact.js>    # per-family Δ
node docs/knowledge/research/aligned-mangling/factorial.mjs <artifact.js>   # interaction structure
node docs/knowledge/research/aligned-mangling/analytic.mjs       # closed-form scorer vs the codec
node docs/knowledge/research/aligned-mangling/libraries.mjs      # search the shipped libraries
node docs/knowledge/research/aligned-mangling/verify-winners.mjs # and check they still behave
node docs/knowledge/research/aligned-mangling/render-findings.mjs  # findings.html
```

Generated rows land in `census.json`, `results.json` and `concentration.json`.

## How the numbers were produced

- **Legality is not assumed.** Mutations go through `scope.mjs`, a scope
  analyser that marks anything it does not fully understand as unrenamable,
  and every rewrite is re-analysed afterwards: same binding count, same
  binding-graph shape, same free names, or the row is reported and not scored.
  Renames are applied by splicing the original text, so two scored artifacts
  differ only in the spelling of names.
- **Behaviour is checked where it can be.** `differential.mjs` drives 37 jQuery
  observations through baseline and mutant in separate jsdom instances;
  `port-differential.mjs` does the same for the three ports, including all 680
  CommonMark and GFM spec cases for markedlil and 18 reactive observations for
  solidlil. Every win quoted here produces byte-identical observations.
  That gate earned its keep: it caught a real bug in this folder's own scope
  analyser that the structural check could not see ([07](07-ports.md)).
- **Two scorers.** Tables use Node zlib Brotli 1.1.0 q11 `lgwin=22` and gzip-9,
  the diagnostic family the other research folders use. The headline result was
  re-scored with `target/release/lilscript-codec`, which is the gate, and the
  numbers agreed.
- **Illegal probes are labelled.** [04](04-alignment.md) contains one: it bounds
  a family, it is not a candidate.

This folder does not change the compiler.
