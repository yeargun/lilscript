# Merge the hottest color into `e` / `t`

Parent: [index](README.md). Follow-up probes in [extra.json](extra.json).

This is the strongest **legal-looking number** in the folder that is
still usually **illegal**. It is the quirk the rest of the playbook
was aiming at.

## What we did

Take only the single hottest local name (1–2 characters) and rewrite
every non-property use of it to another letter. One name changes.
Raw length is identical. If that letter is already a local in the
same scope, bindings collide. The probe does not check interference.

## Hottest names already differ by culture

| Corpus | Hottest local | Count | Next |
|---|---|---:|---|
| jquery-min | `e` | 1835 | `t` 1186, `n` 872 |
| jquery-lil-raw | `a` | 2290 | `b` 1766, `c` 1320 |
| jquery-lil-min | `r` | 2836 | `e` 2286, `t` 1609 |
| glmatrix-lil-vite | `e` | 2821 | `t` 2061, `n` 1596 |
| glmatrix-js-vite | `e` | 2975 | `t` 1670, `n` 1283 |

jquery.min and Vite gl-matrix already speak Terser (`e t n`).
LilScript jQuery raw speaks the canonical alphabet (`a b c d`).
LilScript jQuery **min** (downstream minify) speaks a third dialect
(`r e t`). Three already-minified files, three name tables.

## Remap that one name (Δ br11)

| Corpus | → `e` | → `t` | → `n` | → `a` | → `q` | → `x` |
|---|---:|---:|---:|---:|---:|---:|
| jquery-min (`e` →) | — | **−288** | −192 | −108 | +18 | +10 |
| jquery-lil-raw (`a` →) | **−318** | +15 | −5 | — | +37 | +53 |
| jquery-lil-min (`r` →) | **−548** | −370 | −229 | −42 | +59 | +14 |
| glmatrix-lil-vite (`e` →) | — | **−186** | −121 | −19 | +9 | −11 |
| glmatrix-js-vite (`e` →) | — | **−200** | −60 | −23 | −29 | −11 |

gzip tracks Brotli on these rows (jquery.min `e`→`t` is −334 gzip /
−288 Brotli). This is not a codec fight. It is **reuse gravity on
one spelling**.

Compare to the best *legal-looking* whole-alphabet pass
(`alphabet-function-letters`): −67 / −176 / −31 / −79 / +12. Merging
one name into `t` or `e` beat the whole alphabet rewrite on every
file except LilScript jQuery raw, where `a`→`e` (−318) also beat
it (−176).

Hostile letters (`q`, `x`) are flat or slightly worse. The win is
not “change the hottest name.” The win is “make the hottest name
the letter the file already copies, and make that letter **even
hotter** by folding a second color into it.”

## Why this is usually illegal

jquery.min `e`→`t` means every `function(e,t)` becomes
`function(t,t)`. The two parameters alias. Runtime is wrong.
The codec does not care. It sees one more `t` in a file that is
already full of `t` from `function`, `return`, `length`, `document`,
`undefined`, and the existing local `t`.

`collapse-to-e` in [02](02-reuse.md) is the same idea taken all the
way (−3730 to −7015). This page is the **first step** of that
collapse: merge color 1 into color 2.

## The legal compiler move

Graph-color the interference graph, then **prefer fewer colors
than the chromatic number’s greedy maximum**, assigned from the
`e t n i o a r s` end of the alphabet.

Today’s typical mangler is the opposite greedy:

1. shortest unused name in this scope
2. next letter in `a–z`

That **maximizes** distinct short spellings globally (every scope
gets a fresh `a` *or* reuses `a` by accident). LilScript’s
`local_name_reserve` reuses the first N letters on purpose. This
probe says: after reserve, the remaining win is **collapsing two
reserved colors into one** wherever interference allows.

If `e` and `t` never interfere in a function, they should be the
**same** name. A coloring that keeps both because “we have letters
left” is leaving 200–500 Brotli bytes on the table **per merge**,
before you even talk about the illegal all-`e` ceiling.

`stable_local_names` (source-local affinity) pushes the other way:
the same source binding keeps the same color across functions,
which is good for reuse, but it also **prevents** two source
bindings from sharing a color. `audit-unstable-locals` was
byte-identical to `audit-no-reserve` on the checked-in emit
(33648). On that artifact, turning affinity off did not find the
merge. The merge is a **coloring objective**, not “shuffle
stability.”

## Stacks are not additive

Same files, combining alphabet + `var`→`let`:

| Corpus | alphabet-eni | let only | both |
|---|---:|---:|---:|
| jquery-min | −67 | +17 | **−19** |
| jquery-lil-raw | −176 | −56 | −150 |
| jquery-lil-min | −31 | −44 | −43 |
| glmatrix-lil-vite | −79 | −65 | −63 |
| glmatrix-js-vite | +12 | +10 | −1 |

The product is **worse than the better part** on four of five
files. jquery.min’s alphabet win and `let` fight each other
(`let` introduces `e t` as a keyword while the alphabet is also
moving locals onto `e n i`). LilScript jQuery raw’s −176 alphabet
plus −56 `let` is not −232; it is −150.

Heuristic: Cartesian-expand the families, then **score the
product**. Do not add independently-measured deltas. The compiler’s
beam already does this; a profitability filter that sums proxies
will keep the wrong pair.

## What “clever” is here

Not a ROM word. Not a preamble. A **bias in the register
allocator of names**:

- colors are cheap only if they are the same color
- the first colors should be letters the Huffman tree already
  paid for (`e` from `function` / `return` / `let` / `length`)
- leftover colors are a tax, not a convenience
- colliding two non-interfering live ranges onto `t` is the
  global optimum the greedy per-scope `a,b,c` assignment cannot
  see

Measure it on the complete artifact. If the merge is illegal,
skip it. If it is legal, it is worth more beam budget than quote
style or bait.
