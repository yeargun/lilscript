# Windows, chunks, and independent streams

Parent: [index](README.md).

HTTP compresses each response independently. A bundler that wins
on the concatenated stream can lose on the served chunks, and the
reverse.

## LilScript jQuery raw, same bytes, three deliveries

| Delivery | br11 | Δ vs whole |
|---|---:|---:|
| one stream | 33283 | 0 |
| independent 64 KiB cuts | 35554 | **+2271** |
| independent 32 KiB cuts | 36730 | **+3447** |

Splitting the **same file** into gzip-sized windows costs 7–10%
Brotli. Cross-cut copies disappear. Huffman trees are rebuilt.
The 32 KiB number is not a curiosity: it is gzip’s history and
the compiler’s gzip layout-proposal window.

Brotli’s configured window here is `lgwin=22` (4 MiB nominal).
The 103 KB file fits in one window. Cutting it is a self-inflicted
dictionary amputation.

## Monaco says the same thing upward

| Delivery | Raw | br11 | ratio |
|---|---:|---:|---:|
| full IDE | 2371075 | 423674 | 17.9% |
| first ~210 KB alone | 210467 | 54426 | 25.9% |

The prefix cannot see the rest. The rest, in the full stream,
copies from the prefix (nls tables, AMD glue, repeated helpers).
A “measure the first chunk, assume the rest is similar” lab
**overestimates** transfer.

## What layout search is actually for

[javascript-emission.md](../../compilation/javascript-emission.md)
already discounts layout matches beyond gzip 32 KiB or Brotli 4 MiB.
That is the right *proposal* window. It is not proof the encoder
uses those matches.

This folder’s function-reorder mutations kept the file as **one
stream** and still lost Brotli more often than they won. At q11
+ 4 MiB the encoder already finds distant copies. Putting twins
next to each other is a gzip-32K tactic. See [07](07-layout.md).

The chunk experiment is different: it **forbids** distant copies.
Then locality matters again. If you will serve 32 KB HTTP chunks,
layout-inside-chunk is real. If you will serve one 100 KB
`jquery.min.js`, it is mostly not.

## Heuristic

1. Decide the **delivery object** first: one script, ESM graph,
   lilpack chunks, or HTTP/2 many files.
2. Score each object independently, then sum. Do not score the
   concatenation and call it the website.
3. Use 32 KiB clustering only when a chunk or a gzip objective
   actually has a 32 KiB history.
4. For a Brotli-first single file under ~4 MB, prefer alphabet
   and reuse over function-order cleverness.
5. Report initial / lazy / total separately, as
   [gzip-brotli.md](../gzip-brotli.md) already asks.
