# Where the bits actually are

Parent: [index](README.md). Produced by `census.mjs`, which compresses each
corpus with Brotli q11 `lgwin=22` and then decodes the stream with the
instrumented decoder from [brotli-machine](../brotli-machine.html), counting
every command.

Before arguing about names, it is worth knowing what a stream of ours spends
its bits on. Nobody in the previous rounds had looked, because until the
decoder existed there was no way to ask.

## Output bytes, by where the decoder got them

| Corpus | raw | br11 | from literals | from copies | from the dictionary |
|---|---:|---:|---:|---:|---:|
| jquery-min | 87,533 | 27,445 | 12,099 (13.8%) | 71,654 (81.9%) | 3,780 (4.3%) |
| jquery-src | 285,314 | 69,545 | 11,523 (4.0%) | 253,294 (88.8%) | 20,497 (7.2%) |
| jquery-lil-raw | 102,681 | 33,283 | 15,544 (15.1%) | 83,278 (81.1%) | 3,859 (3.8%) |
| jquery-lil-min | 110,749 | 37,901 | 19,831 (17.9%) | 87,388 (78.9%) | 3,530 (3.2%) |
| glmatrix-js-vite | 73,505 | 14,330 | 6,875 (9.4%) | 66,107 (89.9%) | 523 (0.7%) |
| glmatrix-lil-vite | 68,496 | 14,116 | 6,924 (10.1%) | 61,105 (89.2%) | 467 (0.7%) |
| glmatrix-lil-raw | 116,878 | 17,352 | 6,067 (5.2%) | 110,026 (94.1%) | 785 (0.7%) |

## Stream bytes, by which field consumed them

Each bit is attributed once, to the innermost field that read it.

| Corpus | literals | insert&copy | distances | prefix codes | header |
|---|---:|---:|---:|---:|---:|
| jquery-min | 7,189 | 6,769 | **12,973** | 444 | 54 |
| jquery-src | 6,958 | 16,980 | **44,914** | 621 | 44 |
| jquery-lil-raw | 9,584 | 8,249 | **14,814** | 528 | 75 |
| jquery-lil-min | 12,181 | 9,425 | **15,610** | 572 | 79 |
| glmatrix-js-vite | 3,976 | 4,658 | **4,974** | 560 | 92 |
| glmatrix-lil-vite | 3,794 | 4,549 | **5,137** | 490 | 100 |
| glmatrix-lil-raw | 3,626 | 6,011 | **6,984** | 556 | 105 |

**Distance codes are the largest single consumer of bits in every corpus** —
47% of the jQuery-min stream, 65% of unminified jQuery, 35–40% of gl-matrix.
Literals, the thing identifier spelling is usually argued about, are 26% and
falling as a file gets more repetitive. The prefix codes that everyone worries
about — the whole header machinery — are under 2%.

## Distances: how often the cache is enough

| Corpus | commands | implicit distance | short code | full code | distinct distances |
|---|---:|---:|---:|---:|---:|
| jquery-min | 8,171 | 4.7% | 6.3% | 7,269 | 4,282 |
| jquery-src | 25,159 | 1.2% | 5.9% | 23,364 | 11,882 |
| jquery-lil-raw | 10,003 | 11.3% | 6.6% | 8,215 | 4,915 |
| jquery-lil-min | 11,245 | 15.0% | 7.6% | 8,702 | 5,046 |
| glmatrix-js-vite | 5,462 | 35.3% | 14.3% | 2,753 | 2,060 |
| glmatrix-lil-vite | 5,289 | 33.0% | 12.0% | 2,910 | 2,105 |
| glmatrix-lil-raw | 7,342 | 36.1% | 10.6% | 3,916 | 2,536 |

An *implicit* distance costs nothing at all: command symbols below 128 reuse
the last distance and carry no distance field. A *short code* costs four bits.
A full code costs a symbol plus up to 24 extra bits.

The ranking of the corpora by implicit-distance rate is also, almost exactly,
their ranking by compression: gl-matrix, which reuses the previous distance a
third of the time, lands at 15–21% of raw; the jQuery family, which reuses it
5–15% of the time, lands at 31–34%.

## How far a missed distance missed by

For every full distance code, the distance from the nearest of the four cached
distances:

| Corpus | 1–3 | 4–16 | 17–64 | 65–256 | 257–4k | >4k |
|---|---:|---:|---:|---:|---:|---:|
| jquery-min | 1% | 8% | 12% | 14% | 38% | 27% |
| jquery-lil-raw | 1% | 8% | 11% | 14% | 37% | 29% |
| glmatrix-js-vite | 2% | 16% | 16% | 17% | 34% | 16% |
| glmatrix-lil-raw | 1% | 14% | 16% | 19% | 35% | 15% |

A fifth to a third of full distance codes land within 64 bytes of a distance
the decoder already had. That is the same structure recurring at a slightly
different offset — which is what makes "arrange the output so repeats are
equidistant" an obvious-sounding idea. [06](06-free-order.md) tries it at
function granularity and it does not work: reordering whole functions moves
these buckets by less than a percent and costs Brotli bytes.

## The dictionary is a one-shot device

| Corpus | dictionary references | distinct entries | entries used twice |
|---|---:|---:|---:|
| jquery-min | 529 | 529 | **0** |
| jquery-src | 2,291 | 2,291 | **0** |
| jquery-lil-raw | 545 | 545 | **0** |
| jquery-lil-min | 491 | 491 | **0** |
| glmatrix-js-vite | 81 | 81 | **0** |
| glmatrix-lil-vite | 71 | 71 | **0** |
| glmatrix-lil-raw | 118 | 118 | **0** |

Not one entry is used twice, in any corpus, by a q11 encoder that had the
whole file. After a word's first appearance the file's own history is always
the cheaper source. That single row is the reason
[03](03-dictionary-as-names.md) turns out the way it does.

## Heuristic

- A spelling change that adds literal bytes is fighting the 4–26% of the
  stream that literals occupy. A change that adds *distinct distances* is
  fighting the 35–65% that distances occupy.
- Judge a proposal by what it does to the command count and the distance
  distribution, not by raw length.
- Do not reason about the dictionary as if it had a rate. It has a fixed
  budget of a few hundred first occurrences per artifact — 0.7% to 7% of the
  output — and no encoder will spend it twice.
