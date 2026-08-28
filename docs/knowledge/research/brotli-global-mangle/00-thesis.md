# Global Brotli mangling: the thesis

Parent: [playbook index](README.md).
Tiny-file lab: [lab.html](lab.html).
This folder scores **real artifacts**: jQuery 87–285 KB, LilScript jQuery 103–163 KB,
gl-matrix 68–142 KB, Monaco LilScript IDE 2.4 MB (plus a ~400 KB prefix).

Numbers are Node zlib Brotli 1.1.0 generic q11 `lgwin=22` and gzip-9.
They are **diagnostic**, not `lilscript-codec` gates. Several mutations are
semantically illegal. They exist to show the codec’s gravity, not to ship.

## The local-optimum trap

A typical mangler is a sequence of greedy local choices:

1. shortest unused name in this scope
2. next letter in `a–zA–Z_$`
3. pool a string if it appears N times
4. emit `!0` because it is two raw bytes shorter
5. keep functions in source / IR order

Each step looks optimal on the function in front of you. On a 100 KB closed
program the codec is doing something else: it is building Huffman trees from
the **whole file**, copying across **function boundaries**, and sometimes
spending a dictionary reference on a word you never intended as an identifier.

The global optimum is the spelling that minimizes **one complete stream**.
That spelling is often ugly, and it is not stable across gzip vs Brotli or
q5 vs q11.

## What this folder measured

Twenty-four mutations × nine corpora, plus the in-tree jQuery audit
emits, a Monaco prefix / full baseline, independent-chunk scores, and
surgical one-name remaps. The same mutation can win 176 Brotli bytes
on LilScript jQuery and lose 12 bytes on Vite gl-matrix. Merging one
hottest name into `t` can “win” 288 bytes and still be illegal. That
is the point.

## Four forces, now at scale

1. **Cross-scope reuse of short names.** Breaking it (`uniquify-short`) costs
   14–21 KB Brotli on minified jQuery. Collapsing every 1-char local to `e`
   (illegal) saves 3–7 KB. Reuse is the largest lever in the folder. It is
   not “use dictionary words as names.”
2. **Which short letters you reuse.** Remapping the hottest locals onto
   letters from `function` / `return` / `length` (`e n i t o a r s …`) saved
   67–187 Brotli bytes on several already-minified files **at equal or
   smaller raw size**. Remapping onto `q w x y z j k` often lost. The
   alphabet is not interchangeable.
3. **File-local copies beat ROM words as identifiers.** Promoting the
   hottest locals to `length` / `index` / `value` inflated raw by 20–40 KB
   and Brotli by 1.5–3 KB. The static dictionary is a first-occurrence
   discount for **literals and host names**, not a reason to throw away
   one-byte locals.
4. **Fewer colors, biased to `e`/`t`.** Remapping only the hottest
   local onto `t` or `e` saved 186–548 Brotli bytes — more than the
   whole alphabet rewrite — and is usually an illegal merge. The legal
   move is interference-aware collapse, not per-scope `a,b,c`. Stacks
   of independently-good families are not additive. [15](15-color-merge.md).

## Codec disagreement is normal

On LilScript jQuery raw, sorting functions by length **lost 185 Brotli
bytes** and **saved 1,188 gzip bytes**. A gzip-optimal layout search would
have shipped a Brotli regression. LilScript already ranks the configured
codec. This folder is evidence that layout and alphabet must stay in that
beam, not in a raw-length proxy.

## How to read the pages

| Page | Question |
|---|---|
| [01 corpora](01-corpora.md) | What files, what baseline sizes |
| [02 reuse](02-reuse.md) | Why unique names are a tax |
| [03 alphabet](03-alphabet.md) | Why `e` is not `q` |
| [04 dictionary-as-names](04-dictionary-as-names.md) | Why ROM words as locals lose |
| [05 literals](05-literals.md) | Quotes, `!0`, pooling |
| [06 declarations](06-declarations.md) | `var` / `let` / `const` at 100 KB |
| [07 layout](07-layout.md) | Function order, gzip/Brotli fights |
| [08 bait-and-glue](08-bait-and-glue.md) | Preambles, `.length` vs `["length"]` |
| [09 audits](09-audits.md) | Compiler settings already in-tree |
| [10 monaco](10-monaco.md) | Does it survive a megabyte |
| [12 codec disagreement](12-codec-disagreement.md) | gzip / q5 / q11 inversions |
| [13 windows and chunks](13-window-chunks.md) | 32K vs whole-file |
| [14 quirk catalog](14-quirk-catalog.md) | The ugly wins |
| [15 color merge](15-color-merge.md) | Hottest local → `e`/`t` |
| [16 identifier cultures](16-ident-cultures.md) | `abc` vs `etn` |
| [11 playbook](11-playbook.md) | Heuristics for candidate search |
| [lab.html](lab.html) | All mutation tables |
| [results.json](results.json) | Raw rows |
