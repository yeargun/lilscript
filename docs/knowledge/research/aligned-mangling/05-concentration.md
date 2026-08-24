# Fewer names, not shorter ones

Parent: [index](README.md). Produced by `experiments.mjs`,
`concentration.mjs`, `differential.mjs`, and confirmed with
`target/release/lilscript-codec`.

This is the one place the measurements found real money, and it is not where
either of the two questions was looking.

## The result

Re-mangle each corpus from scratch through `scope.mjs`, using precise
availability rules — a name is unusable only when it would collide with a
sibling, capture a reference in the scope's subtree, or be shadowed between a
reference and its own declaration — and score the best legal naming against
what the artifact ships with:

| Corpus | shipped br11 | best legal naming | Δ br11 | Δ raw | distinct names, shipped → best |
|---|---:|---|---:|---:|---|
| jquery-min | 27,445 | frequency / adaptive | −13 | 0 | 165 → 165 |
| glmatrix-js-vite | 14,330 | as shipped | 0 | 0 | 36 → 36 |
| glmatrix-lil-vite | 14,116 | first-use / dialect | −6 | 0 | 32 → 33 |
| **jquery-lil-raw** | **33,283** | source / reversed | **−801** | −542 | **106 → 25** |
| **jquery-lil-min** | **37,901** | first-use / dialect | **−617** | −95 | 31 → 29 |
| glmatrix-lil-raw | 17,352 | first-use / reversed | −1,151 | −5,769 | 95 → 33 |
| jquery-src | 69,545 | source / etn | −6,428 | −47,043 | (unminified: mangling it at all) |

Two clean groups. On artifacts a mature JavaScript minifier produced —
jquery.min, gl-matrix through Vite — there is nothing left: 0 to 13 bytes. On
**LilScript's own emits** there is 0.04% to 6.6%.

## The −801 is real

For `jquery-lil-raw`, the best variant was checked three ways:

| Check | Result |
|---|---|
| Binding graph re-analysed after the rewrite | same count, same shape, same free names |
| jsdom differential against the shipped artifact, 37 observations | 0 differences (2 observations throw in both) |
| `lilscript-codec --json` (bundled Google Brotli 1.1.0 q11 lgwin 22) | 33,283 → **32,482**, raw 102,681 → 102,139 |

32,482 is smaller than every row in the
[global-mangle playbook](../brotli-global-mangle/README.md), whose best full
port was `audit-positional` at 33,165.

The three best namings land within 66 bytes of each other (32,482 / 32,544 /
32,548) from quite different alphabets, which says the win is not the alphabet.

## The mechanism

| Artifact | renamable bindings | distinct names | name entropy | top-5 share |
|---|---:|---:|---:|---:|
| jquery-lil-raw, as shipped | 2,174 | 106 | 3.94 bits | 68.2% |
| jquery-lil-raw, re-mangled | 2,174 | **27** | **3.30 bits** | **75.1%** |
| jquery-lil-min, as shipped | 2,659 | 31 | 3.60 | 67.7% |
| jquery-lil-min, re-mangled | 2,659 | 32 | **3.14** | **77.6%** |
| jquery-min (Terser), as shipped | 1,802 | 165 | 4.49 | 56.2% |
| jquery-min, re-mangled | 1,802 | 165 | 4.45 | 59.8% |

LilScript spends **106 distinct spellings on 2,174 bindings where 27 suffice**,
and 76 of them are two characters long where one was available. The rewrite
does not shorten names by being clever; it stops handing out new ones. Raw
falls 542 bytes, and Brotli falls 801 — more than the raw saving, because
every occurrence of a rarer name was also a more expensive literal.

This is the legal form of the effect the playbook could previously only reach
with an illegal probe. [02 reuse](../brotli-global-mangle/02-reuse.md) measured
`collapse-to-e` at −5,720 on this file as an unshippable ceiling, and
[15 color-merge](../brotli-global-mangle/15-color-merge.md) measured merging a
single hot name at −318. A legal, verified −801 sits between them, which is
exactly where a real interference-aware collapse should sit.

## Where this already lives in the compiler

The compiler is not missing the idea. `src/codegen_ir_js.rs` has two regimes
for deciding which names a nested function may reuse:

- the default path releases only function names and the explicit
  `local_name_reserve` entries that the function does not reference;
- `precise_cross_scope_shadowing` releases **every** reserved name the
  function's subtree does not reference — the rule this folder's re-mangler
  implements.

`precise_cross_scope_shadowing` is `false` in the configured emission path
(`src/config.rs:357`), deliberately: the comment there explains that the
pinned path stays conservative so an incomplete transitive-reference proof can
be rejected without invalidating the mandatory fallback. It is not a TOML key.
The only way to reach it is as a candidate-beam proposal
(`src/compiler.rs:3310`, `:4464`), which flips it and scores the result.

So the question this result raises is narrow and answerable: **the beam can
propose this, the port config asks for `candidate_search = "production"`, and
the checked-in artifact still spells 106 names.** Either the proposal is not
reaching the finalists on this artifact, or the checked-in artifact predates
the current beam. [PLAN.md](PLAN.md) turns that into a task rather than a
guess.

## Proxies do not rank namings

If a cheap number could rank candidate namings, the beam could try many. It
cannot. Across 19 legal namings per corpus, correlation with Brotli-11:

| Corpus | name bytes | name entropy | distinct names |
|---|---:|---:|---:|
| jquery-min | 0.97 | 0.92 | 0.98 |
| jquery-src | 0.97 | 0.92 | 0.98 |
| jquery-lil-raw | 0.35 | 0.66 | 0.42 |
| jquery-lil-min | 0.23 | 0.26 | 0.47 |
| glmatrix-js-vite | 0.00 | 0.14 | 0.35 |
| glmatrix-lil-vite | 0.00 | 0.38 | 0.23 |

The strong correlations are on the corpora whose variants differ in raw size by
tens of kilobytes — the proxy is measuring length, which everyone already
knows. Hold raw size constant, as the equal-raw corpora do, and every proxy
collapses to noise. Entropy explains the *direction* of the big win and cannot
rank the small ones.

## Heuristic

- The objective for name assignment is **the number of distinct spellings and
  how concentrated their use is**, not the length of any one name. Shortening
  a name that adds a new spelling can lose.
- Two live ranges that do not interfere should get the same name even when a
  fresh letter is free. This is a colouring objective, and it is worth more
  than every alphabet, keyword and quote decision in the playbook combined.
- Rank namings with the configured codec. No proxy survives holding raw size
  constant.
- Any naming change must carry a behavioural differential, not only a size
  delta. Three of the strategies tried in this folder were rejected by the
  binding-graph check before they could be scored.
