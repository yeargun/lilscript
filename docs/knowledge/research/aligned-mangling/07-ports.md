# The three ports: jquerylil, markedlil, solidlil

Parent: [index](README.md). Produced by `ports.mjs` and `port-differential.mjs`,
scored again with `target/release/lilscript-codec` where a number is claimed.

The earlier pages measured benchmark artifacts inside this checkout. This page
measures what the three ports actually publish, and every row here is checked
by running the port, not by reading it.

## How each row is verified

| Port | Battery | Size |
|---|---|---:|
| jquerylil | jsdom: selection, traversal, classes, css, data, events, delegation, Deferred, serialize | 28 observations |
| markedlil | every case in CommonMark 0.31.2 and GFM 0.29, compared byte for byte | 680 observations |
| solidlil | the reactive core driven through `solidlil/index.js`: signals, memos, computed order, render effects, batch, untrack, `on`, cleanup, nested roots, context, selectors, `mapArray`, `indexArray`, `children`, owners | 18 observations |

A mutation is reported only if the mutant's observations are identical to the
baseline's. All of the wins below are.

## Is there a cheaper legal naming?

Δ Brotli-11 for the best legal renaming of each artifact, against what the port
ships today.

| Artifact | raw | br11 | best naming | Δ br11 | Δ raw | distinct names | verified |
|---|---:|---:|---|---:|---:|---|---|
| `jquerylil/dist/jquery.raw.js` | 92,668 | 30,890 | source / reversed | **−770** | −524 | 455 → 454 | 28/28 identical |
| `jquerylil/dist/jquery.esm.js` | 92,765 | 30,973 | first-use / reversed | **−776** | −524 | 455 → 454 | 28/28 identical |
| `markedlil/dist/marked.bytes.js` † | 33,548 | 9,456 | source / dialect | −37 | −34 | 188 → 175 | 680/680 identical to *itself* |
| `markedlil/dist/marked.gzip.js` | 36,220 | 9,543 | source / dialect | −5 | −2 | 198 → 197 | 680/680 identical |
| `markedlil/dist/marked.raw.js` | 35,901 | 9,543 | source / dialect | +14 | −4 | 198 → 197 | 680/680 identical |
| `markedlil/dist/marked.esm.js` | 35,985 | 9,589 | first-use / dialect | +6 | −4 | 198 → 197 | 680/680 identical |
| `solidlil/reactive.generated.js` | 13,500 | 4,377 | source / abc | **−96** | −63 | 94 → 83 | 18/18 identical |
| `solidlil-core-open.js` (bundled) | 30,074 | 8,433 | source / dialect | −4 | −15 | 132 → 133 | app bundle, not driven |
| `solidlil-lsx-vite.js` (bundled) | 34,209 | 10,741 | first-use / adaptive | −1 | −3 | 296 → 296 | app bundle, not driven |
| `solidlil-web.js` (bundled) | 40,467 | 11,667 | first-use / dialect | +7 | 0 | 136 → 136 | app bundle, not driven |

Two clean groups again, and they are the same two as
[05](05-concentration.md):

- **What the compiler emits and the package ships** — jquerylil's dist files
  and solidlil's reactive core — carries **2.2% to 2.5%**.
- **What a downstream bundler re-mangles** — every solidlil bundle — carries
  nothing. Rolldown and esbuild already take it.

† `marked.bytes.js` is not a shippable baseline: it is miscompiled, see below.
The renaming round-trips against it, but the artifact underneath is wrong, so
that −37 is not an available win.

markedlil sits in between and is the interesting case: its naming is nearly
optimal already, and the headroom is somewhere else entirely.

## markedlil: one cost model miscompiles

markedlil compiles the same sources four times, changing one knob each time
(`scripts/build.mjs`, four `lilscript.*.toml`). Scored with the gate codec, and
— because a size comparison between builds that do not compute the same thing
is not a size comparison — run through all 680 CommonMark 0.31.2 and GFM 0.29
spec cases:

| Artifact | knob | raw | gzip-9 | Brotli-11 | spec failures |
|---|---|---:|---:|---:|---:|
| `marked.bytes.js` | `cost_model = "raw"` | 33,548 | 10,554 | 9,456 | **208** |
| `marked.closed.js` | `cost_model = "brotli"`, `extern_fields = false` | 35,621 | 10,678 | **9,475** | 206 |
| `marked.raw.js` | `cost_model = "brotli"` | 35,901 | 10,711 | 9,543 | 206 |
| `marked.gzip.js` | `cost_model = "gzip"` | 36,220 | 10,674 | 9,543 | 206 |
| `marked.esm.js` | what the package publishes | 35,985 | 10,766 | 9,589 | 206 |

206 failures is this port's normal state — marked is not fully CommonMark
compliant, and four of the five builds fail exactly the same 206 cases and
produce byte-identical output on all 680.

**The fifth does not.** `cost_model = "raw"` fails two cases the others pass:

```text
commonmark 0.31.2 #604 (Autolinks)
  markdown  <foo@bar.example.com>
  expected  <p><a href="mailto:foo@bar.example.com">foo@bar.example.com</a></p>
  got       <p><a href="foo@bar.example.com">foo@bar.example.com</a></p>

commonmark 0.31.2 #605 (Autolinks)
  markdown  <foo+special@Bar.baz-bar0.com>
  expected  <p><a href="mailto:foo+special@Bar.baz-bar0.com">…</a></p>
  got       <p><a href="foo+special@Bar.baz-bar0.com">…</a></p>
```

The `mailto:` prefix is gone. Same sources, same compiler, one knob apart, and
`candidate_search = "production"` in both: **the search under the raw cost
model ranked and shipped a candidate that changes what the program computes.**

That is the finding. The 87 Brotli bytes that build appears to save are not a
saving — the artifact is smaller because it does less. Any comparison against
it is void, and the two spec cases are a minimal reproduction that fits in a
test.

**It reproduces at HEAD.** Rebuilding `src/entry.lil` under
`lilscript.bytes.toml` with the compiler in this checkout produces a 33,484-byte
artifact that fails the same two cases — 208 against the other builds' 206. The
matching Brotli-model rebuild is 36,018 bytes and passes. Not a stale dist file.
(Each build took 45–50 minutes wall for a ~35 KB output, which is its own note
for anyone planning to iterate here.)

### What the raw model actually did differently

`shapediff.mjs` on the two fresh builds:

| token | `cost_model = "brotli"` | `cost_model = "raw"` | diff |
|---|---:|---:|---:|
| `.slice(` call sites | 63 | 2 | −61 |
| `.exec(` call sites | 39 | 1 | −38 |
| `.replace(` call sites | 28 | 1 | −27 |
| `;` statement ends | 1,092 | 383 | −709 |
| `,` separators | 790 | 1,469 | +679 |
| `var ` declarations | 240 | 75 | −165 |
| `let ` declarations | 88 | 7 | −81 |
| `for(` loops | 50 | 1 | −49 |
| `while(` loops | 0 | 49 | +49 |
| AST nodes | 11,298 | 10,355 | −943 |

Four families, applied by the raw model and declined by the Brotli model:
outlining repeated member calls into helpers, fusing statements into comma
sequences (which lets 99 blocks lose their braces), merging adjacent
declarations, and rewriting `for(;t;)` as `while(t)`.

### Which of them would have helped Brotli

`families.mjs` applies one family at a time to the **Brotli**-model artifact
and scores each with the gate codec:

| family | sites | Δ raw | Δ gzip | Δ br11 |
|---|---:|---:|---:|---:|
| merge adjacent declarations | 230 | **−920** | −60 | **−3** |
| `for(;t;)` → `while(t)` | 49 | 0 | −5 | **+19** |
| outline `.slice`/`.exec`/`.replace` | 130 | −187 | +79 | **+126** |

So the Brotli model is right to decline two of them: `while` costs it 19 bytes,
and a naive outlining costs 126. But **merging adjacent declarations is 920 raw
bytes for a Brotli difference of ±3** — a tie on the ranked metric, and the
compiler is taking the 920-byte-larger side of that tie.

The same probe on the other artifacts finds **zero** opportunities:

| artifact | merge sites | `for`→`while` | outlining |
|---|---:|---:|---:|
| `jquerylil/dist/jquery.esm.js` | 0 | +46 | +53 |
| `jquery-lilscript.raw.js` (in-tree) | 0 | — | — |
| `solidlil/reactive.generated.js` | 0 | — | — |
| `markedlil/dist/marked.raw.js` | **230** | — | — |

jquerylil and solidlil already merge every adjacent declaration. markedlil does
not. The obvious suspect was configuration — jquerylil's `lilscript.toml`
carries an explicit `compression = [...]` list of 30 passes while markedlil's
takes the defaults — so markedlil was rebuilt with that list under
`cost_model = "brotli"`:

| build | raw | gzip-9 | Brotli-11 | adjacent declarations left unmerged |
|---|---:|---:|---:|---:|
| default config, `cost_model = "brotli"` | 36,018 | 10,700 | 9,509 | 230 |
| **+ jquerylil's 30-pass list** | 36,062 | 10,680 | **9,504** | **230** |
| default config, `cost_model = "raw"` | 33,484 | 10,556 | 9,441 | 1 |

**The pass list is not the cause.** It moves five Brotli bytes and leaves all
230 declarations unmerged; the raw-cost build leaves one. The merging decision
follows the cost model, not the enabled passes. (The list also roughly doubled
search time — ~110 CPU-minutes against ~52.)

So the size half of this is not "the Brotli cost model is broken". It is
narrower and more fixable: **a Brotli tie is being broken the wrong way**,
costing 920 raw bytes on this artifact, and it is the cost model — not the pass
list — that decides it.

### The correctness half is the `ident` bug class

The two builds' autolink handler, side by side:

```js
// cost_model = "brotli"  — correct
let dr = e => {
  var t = ae.exec(e);                              // t = the match
  if (!t) return null;
  e = Ir(t);
  var r = "@" == At(t, 2) ? "mailto:" + e : e;     // reads group 2 of t …
  t = a(15, t[0] + "");                            // … before t is reused
  t.text = e; t.href = r;
  …
};

// cost_model = "raw"  — wrong
A => {
  var p = R(up, A);                                // p = the match
  if (!N(p)) return null;
  A = a(p, 1),
  p = i(15, M(p)),                                 // p reassigned to the token
  p.text = A,
  p.href = "@" == a(p, 2) ? "mailto:" + A : A;     // reads group 2 of p — too late
  …
}
```

The statement-fusion family put those four assignments into one comma sequence,
and the ternary that reads match-group 2 was sunk **past the reassignment of the
variable it reads**. `a(token, 2)` is not `"@"`, so the `mailto:` branch never
runs.

That is not a cost-model bug. It is
[`ident-01`](../../migration/board/notes/ident-01.md)'s invariant — *a saved
value must stay readable across its own update* — and exactly the class
`ident-02` exists to generalise: a rematerialisation/sinking fold that does not
check whether its receiver was rebound in between. The raw cost model does not
cause it; it just buys enough fusion for the fold to fire.

**This is a live reproduction of the `ident` lane's bug class in a shipped
port, with a one-knob trigger and a two-case spec signature.**

Among the four builds that do compute the same thing:

- the smallest is `marked.closed.js` at **9,475**;
- the package publishes `marked.esm.js` at **9,589** — 114 Brotli bytes above a
  correct build it already knows how to produce;
- naming headroom on those builds is nil: −5 to +14.

So markedlil's list, in order: fix the miscompilation, publish the smallest
correct build, and do not bother with naming here.

## jquerylil: the same shape as the in-tree port

`jquerylil/dist/jquery.raw.js` spells 455 distinct names across 2,460
bindings; a legal renaming with the same raw budget spells the same program 770
Brotli bytes smaller, with name entropy dropping 5.10 → 4.78. The in-tree
benchmark port ([05](05-concentration.md)) gives −801 on a different build of
the same library. Two independent builds, the same 2.4%.

## solidlil: the win exists and then the bundler eats it

`reactive.generated.js` is the compiler's emit: 94 distinct names over 470
bindings, −96 Brotli bytes (2.2%) available, all 18 reactive observations
identical.

Every bundle built from it shows nothing: −4, −1, +7. Rolldown and esbuild
re-mangle on the way through, and whatever LilScript did or did not do about
naming is gone by the time the app ships.

For the LilScript-versus-JavaScript question the bundles say something else
worth keeping:

| Pair | LilScript | JavaScript | Δ |
|---|---:|---:|---:|
| `lsx-vite` app bundle | 10,741 | 12,560 | **−1,819 (−14.5%)** |
| `core-open` | 8,433 | 8,551 | −118 |
| `web` | 11,667 | 11,655 | +12 |

Those gaps are shape, not spelling — and they are much larger than anything
naming can move.

## What this changes in the plan

1. **`markedlil`'s miscompilation becomes Phase A0**, ahead of everything. A
   search that ranks a behaviour-changing candidate is the same class as the
   board's `ident` lane, and this one has a two-case reproduction and a
   one-knob trigger.
2. **The naming work is worth doing where the emit ships directly.** jquerylil
   and solidlil's core are exactly that. For anything that goes through a
   bundler, spend the effort on shape instead.
3. **markedlil should publish `marked.closed.js`** (9,475) rather than
   `marked.esm.js` (9,589) — 114 bytes, no behaviour change — and should not
   publish `marked.bytes.js` at all until the miscompilation is fixed.

## A note on how the bug in this folder was found

The first run of these batteries failed: markedlil's mutants threw
`t is not defined` on all 680 spec cases, and solidlil's threw too, while the
structural binding-graph check reported the rewrite as legal. The analyser was
declaring `let`/`const` only when it *reached* them, so a function that
referenced a module-level `let` declared later in the file resolved as free —
and the renamer moved the declaration out from under it. Both the before and
after analyses agreed, so no structural check could see it.

The fix is in `scope.mjs`: every scope's lexical declarations are now hoisted
before any of its statements are walked, and `verify` compares the *resolution
sequence* — which binding each identifier occurrence resolves to, in source
order — instead of comparing graph shapes.

The benchmark corpora were unaffected (they are `var`-shaped ES5 emits) and
every number on the other pages re-ran identically. The lesson is the one this
repository already states: a semantic gate is not optional, and a structural
proof is not a semantic gate.
