# brotli-machine

Sources for [`../brotli-machine.html`](../brotli-machine.html) — a self-contained page
holding a working RFC 7932 encoder and decoder, the static dictionary, and the
instruments that step through both.

## Layout

| file | what it is |
| --- | --- |
| `gen-data.mjs` | extracts the static tables from the vendored Brotli C sources into `data/tables.js` |
| `data/tables.js` | generated and committed: dictionary (raw + Brotli's own compressed copy), 121 transforms, 2,048-byte context table, and the C `kCmdLut` used only to check our derivation |
| `src/10-tables.js` | RFC tables, including the 704-row command table derived from the two 24-entry length tables |
| `src/20-bitio.js` | LSB-first bit reader and writer, both recording the bit span of every field |
| `src/30-huffman.js` | canonical prefix codes; package-merge with a 15-bit cap |
| `src/40-dictionary.js` | word lookup, the 121 transforms, and match search |
| `src/50-decoder.js` | the decoder, instrumented: every read becomes a trace event |
| `src/55-plugins.js` | the encoder's replaceable stages (this is what the page's editors edit) |
| `src/60-encoder.js` | histograms, code writing, serialization |
| `src/70…78-*.js` | the page: helpers, the machine, the reference sections, the editors, boot |
| `page.css`, `page.html` | style and markup, inlined by the renderer |
| `test.mjs` | the engine's tests, run against Node's real Brotli |
| `render.mjs` | runs the tests, then writes `../brotli-machine.html` |

Scripts are plain (no imports); they attach to a `BM` global and are concatenated in
filename order, so the code the tests exercise is exactly the code the page ships.

## Commands

```sh
node docs/knowledge/research/brotli-machine/test.mjs      # engine only
node docs/knowledge/research/brotli-machine/render.mjs    # test, then build the page
node docs/knowledge/research/brotli-machine/gen-data.mjs  # after a Brotli version bump
```

`render.mjs` refuses to write the page if a test fails. `SIZES=1` on `test.mjs` prints
our sizes next to brotli q11 and gzip -9.

## What the tests check

- the decoder against streams from the real library at qualities 0/1/5/9/11 and
  windows 10/16/22/24, over ten inputs, plus Brotli's own compressed dictionary
  (51,687 → 122,784 bytes);
- the encoder's output handed back to the real library, across every literal-tree,
  context-mode, dictionary and lazy-matching combination, plus 60 fuzz cases;
- the derived 704-row command table against the C decoder's `kCmdLut`, row for row
  (in `gen-data.mjs`, which also asserts the dictionary tables tile 122,784 bytes).

## Deliberate limits

The encoder emits one meta-block with one block type per category, probes the
identity / suffix / upper-case-first transform families, and leaves `NPOSTFIX` and
`NDIRECT` at zero. The decoder implements the whole format apart from large-window
(`WBITS > 24`) streams, and keeps the output in one buffer instead of a ring.
