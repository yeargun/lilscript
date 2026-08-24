# Dictionary words as names lose at every frequency

Parent: [index](README.md). Extends
[04 dictionary-as-names](../brotli-global-mangle/04-dictionary-as-names.md),
which measured hot locals with a token rewrite. This page measures the
complement — the *cold* ones — with a scope-correct rename, and gives the
mechanism.

## The question

If the codec already knows `length`, `index`, `value`, `callback`, why keep
spending our own bytes teaching it `a`, `b`, `c`? Surely a name it has in ROM
is cheaper than a name it has to learn.

## What we did

Through `scope.mjs`, so these are legal renames and not token substitutions:

- **hot:** the 400 most-referenced renamable bindings get distinct dictionary
  words;
- **cold:** the 400 least-referenced bindings with **three uses or fewer** get
  distinct dictionary words.

Words are drawn from the dictionary's own identifier-shaped entries, skipping
any spelling the file already uses, so nothing collides and nothing is
captured. Both variants pass the binding-graph check; the jQuery ones also
pass the 37-observation behavioural differential.

## Result

Δ Brotli-11 against each corpus's own baseline.

| Corpus | cold (≤3 uses) | hot (top 400) |
|---|---:|---:|
| jquery-min | **+1,734** | +3,711 |
| jquery-src | +1,017 | +1,953 |
| jquery-lil-raw | +1,653 | +5,203 |
| jquery-lil-min | +1,675 | +5,466 |
| glmatrix-js-vite | +2,437 | +5,681 |
| glmatrix-lil-vite | +2,208 | +6,341 |
| glmatrix-lil-raw | +2,039 | +5,859 |

There is no corpus, and no frequency band, where this wins. The cold band is
the interesting one: those bindings are used once, twice or three times, which
is exactly the regime where "the dictionary pays the first occurrence" should
have applied. On gl-matrix the cold variant costs **2,208 Brotli bytes** while
growing raw by only 2,703 — nearly the entire raw growth is paid in full, at
Brotli's *worst* rate, as if the codec had no dictionary at all.

## Why the first-occurrence discount does not arrive

From [01](01-where-the-bits-are.md): in every corpus, **every dictionary
reference is used exactly once**, and the dictionary supplies 0.7%–7.2% of
output bytes. It is a fixed budget of a few hundred first occurrences per
artifact, not a rate.

Three things then go wrong at once when a local is renamed to `length`:

1. **The discount is one reference deep.** The first `length` may come from
   ROM at ~25 bits. The second one is a copy — of six bytes, at a distance the
   coder must also spell. The 400th is still a six-byte copy. A one-byte `a` in
   a file already full of `a` is a literal costing two or three bits, in a
   context the literal model has already learned.
2. **The context model is damaged.** Literals are coded under a context taken
   from the two preceding bytes. `a` after `(` is one of the most predictable
   bytes in a minified file. A six-letter word after `(` spends five more
   literals in contexts that were previously sharp.
3. **The dictionary reference competes with itself.** `length` as a local
   floods the file with a token that the `.length` transform was already
   serving. The measured implicit-distance rate drops with every one of these
   mutations — from 33.0% to 26.6% on gl-matrix — because the copy structure
   that the cache was riding gets broken up.

## The crossover, stated properly

There is no crossover in this population. For a binding to profit from a
dictionary spelling it would have to be referenced **once**, be at least
several bytes long, sit where the surrounding context is otherwise unpredictable,
and displace nothing. That is not a description of an identifier; it is a
description of a **string literal**, which is where the dictionary does earn
its keep — [02](02-the-hardcoded-library.md) and the pooled-string ordering in
[06](06-free-order.md).

## Heuristic

- Never generate "identifier ← dictionary word" as a candidate, at any
  frequency. It is a strong, repeatable loss, and the cold band is not an
  exception.
- The dictionary's audience is *first occurrences of literals and host names
  we must emit anyway*. Spend tie-breaks there.
- If a future proposal claims a dictionary win, ask it for its census row: how
  many additional dictionary references did the stream actually make? If the
  answer is not "several hundred more", the mechanism it claims does not exist.
