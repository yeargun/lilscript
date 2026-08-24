# The two questions, answered short

Parent: [index](README.md). Evidence in the pages linked from each answer.

## 1. Why keep spelling names `a`, `b`, `c` instead of using words the dictionary already has?

**Because the dictionary has no rate — only a few hundred first occurrences per
artifact, and it never spends one twice.**

Measured on every corpus, at every frequency band, with legal scope-correct
renames: dictionary words as identifiers cost **+1,017 to +2,437** Brotli bytes
for bindings used three times or fewer, and **+1,953 to +6,341** for hot ones
([03](03-dictionary-as-names.md)).

The mechanism is one row of the census: in a real Brotli stream of ours, of the
529 dictionary references in jQuery-min, **529 are distinct** — and the same is
true of every other corpus ([01](01-where-the-bits-are.md)). The dictionary
supplies 0.7%–7.2% of output bytes, entirely as first occurrences. After that,
the file's own history is cheaper than the ROM, and a one-byte `a` in a file
full of `a` is cheaper than both.

Where the dictionary *does* pay is first occurrences of literals and host names
we have to emit anyway. Measured, that is worth −9 to −15 bytes as a tie-break
on pooled-string order ([06](06-free-order.md)), and the catalogue of exactly
which JavaScript spellings are one reference is in
[02](02-the-hardcoded-library.md) — `function(`, `);return `, `for(var `,
`}else{`, `.length`, `.call(`, `Math.`, `var `, but not `let `, `const `,
`=>{`, `await `, or `constructor`.

## 2. Could naming be aligned across closures, so `a[1]`, `a[2]` recurs instead of `a[1]`, `b[1]`?

**The pattern is real; the headroom has already been taken by frequency-ordered
mangling, and what remains is not reachable by aligning spellings.**

Three measurements ([04](04-alignment.md)):

- **Twins up to renaming do not exist.** Across seven corpora there are 0–1
  groups of functions that are the same code modulo naming, and those are
  already byte-identical. Frequency order is itself a canonical order, so two
  functions of the same shape already receive the same names.
- **An aligner that maximises copyability loses** (+49 to +673 br11), because
  choosing a name to match earlier text flattens the name distribution and
  literals get more expensive faster than copies get longer.
- **Under a codec-shaped cost model the greedy assignment is already a local
  optimum**: the bit-cost aligner made *zero* changes on every corpus.

For the exact `a[1]` shape: on the jQuery family, `name[constant]` is 0.5–1.4%
of the file and the **illegal** ceiling for perfect receiver alignment is
−57 to −96 bytes. On gl-matrix it is 19–20% of the file and the ceiling is
−327 to −395 — but the hot receivers there are *already* `e[0..3]` and
`t[0..3]` across hundreds of functions. What is left is the interference case,
where two receivers are live at once and cannot share a name.

## What the questions found on the way past

The legal move behind both intuitions is not spelling but allocation: **hand
out fewer distinct names**. On LilScript's own jQuery artifact that is worth
**−801 Brotli bytes (−2.41%)**, behaviour-identical, confirmed with
`lilscript-codec` — larger than anything in the previous playbook, and larger
than the entire aligned-naming ceiling on the same file
([05](05-concentration.md)).

The compiler already has the regime that produces it. It is off by default and
reachable only through candidate search. [PLAN.md](PLAN.md) is about closing
that gap.
