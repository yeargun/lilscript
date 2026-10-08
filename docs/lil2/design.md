# lil2: the typed, flat markdown stack

Brief: [finer/intent/2026-10-04.md](../../finer/intent/2026-10-04.md). The `lil2-<library>` family reimplements
the react-markdown stack in typed LilScript. The existing `<library>lil` ports stay as they are.

## 1. The contract

- **Behaviour equals upstream.** The same markdown in gives the same HTML and the same React element tree out,
  including positions, keys, error-free edge cases, and GFM/math/KaTeX output. Upstream versions are pinned in §8.
- **The API may change.** Every repo publishes a lean typed API; upstream's object-shaped APIs are not kept.
  Each repo ships a *differential* suite that feeds identical inputs to upstream and to lil2 and compares the
  observable output, in Node and, for the browser build, in Chromium and Firefox.
- **Flat, and ints for every id and enum.** No per-character, per-token, per-event or per-node object: data
  lives in parallel arrays indexed by int ids. Every enumeration is an int — token types, construct ids, node
  kinds, tag names, property names, enumerated attribute values, alignments, reference types, identifiers.
  A string exists only as content (text, URLs, class values) or where a name must leave the program (the React
  element type and prop keys), read from a name table by its id. No glue layers: a layer is redesigned instead
  of adapted.
- **Smaller, faster, quicker to load.** Each repo reports raw/gzip/Brotli against Terser, esbuild and Oxc on the
  upstream bundle, plus steady-state, cold first render and load timings in Chromium and Firefox on the
  Playwright harness. Same-or-better in every cell is the floor; browser cells decide.

## 2. Data model

### 2.1 Codes and positions (`lil2-micromark`)

`preprocess(value)` turns the document into micromark codes (`-5` CR, `-4` LF, `-3` CRLF, `-2` HT, `-1`
virtual space, `-6` EOF). Tokenizers keep upstream's point arithmetic (buffer index, line, column, offset,
`defineSkip`) on themselves; a token stores only its start/end buffer index and offset. Under that arithmetic
`column - 1 == offset - lineStart(line)`, so line and column are read from the parser's line table on demand
(verified on every event of the corpus). Token text is a slice of the source.

### 2.2 Tokens, events, constructs

```
class Tokens { int[] type, contentType, flags, sIndex, sOffset, eIndex, eOffset, previous, next,
               tokenizer, child, marker; int[] aux; }
```

A token is an int; an event is `token * 2 + kind`. Token types (`tXxx`) and construct ids (`C_XXX`, upstream's
construct `name`, used by `disable`) are compile-time ints; each extension package owns a range
(`TYPES_GFM`, `TYPES_MATH`, `CONSTRUCTS_GFM`, …). `aux` holds int runs an extension attaches to a token through
`marker` (a GFM table's alignments). Identifiers are interned by the parser: a label's normalized key becomes an
int id that every later stage uses; flags per id say whether the parse defined it (`parser.defined`,
`parser.gfmFootnotes`).

### 2.3 Trees

mdast and hast are arenas of parallel arrays with offsets (+ the line table) for positions. mdast: `kind`
(`K_*`), flags, links, `num`, three string slots, `identifier` (an id into `identifiers`), `label`, `aux`.
hast: `kind` (`H_*`), `tag` (`TAG_*`, names in `tagNames`, interned past the core table for parsed HTML),
`value`, `meta`, and per element an ordered property list of `(prop id, value kind, string, number)` — value
kinds string, number, boolean, list, and keyword (`KW_*`: alignments, `checkbox`).

Each layer's public result is its arena as positional columns (the arrays themselves, zero-copy) plus the name
tables a consumer needs. Callbacks and components get node ids plus those columns, never objects.

## 3. Plugins, typed

| Upstream | lil2 |
|---|---|
| micromark extension `{document, flow, text, …: {[code]: Construct}}` | `SyntaxExtension`: per hook, code → `Construct[]`, merged once into dense arrays indexed by code; the last combination is reused while the same extension objects come back |
| `Construct {name, tokenize, resolve, …}` | `class Construct` with an int id and typed function fields; states are `func(int)->void` setting `t.next` |
| `disable: {null: ['codeIndented']}` | `disable: [C_CODE_INDENTED]`, a bool table by construct id |
| token type strings | int constants in the package's range |
| from-markdown / to-hast handler maps | handler tables indexed by token type / node kind |
| unified `use(plugin, options)` | compiled-in plugins: an installer `func(Pipeline)->void` per plugin id adds syntax extensions, mdast handlers, mdast transforms, to-hast handlers, hast transforms and (KaTeX) the JSX style parser; plugin options are Markdown props |
| react-markdown `components={{h1: C}}`, `allowedElements: ['p']`, `urlTransform(url, 'href', node)` | `components: [TAG_H1, C, …]` pairs, `allowedElements: [TAG_P]`, `urlTransform(url, PROP_HREF, node, tree)`; the constants come from the package's `constants` entry |

**Flavors, not split chunks.** One program per flavor file (`single` delivery): `lil2-react-markdown` (core),
`/gfm`, `/full` (GFM, math, KaTeX, breaks), `/constants`. A flavor registers its plugins when it loads; `plugins`
applies them by id, in upstream's order. The compiler's `split` delivery would share one copy, but it cut the core
alone into 27 files, which costs load time.

**KaTeX output.** rehype-katex reads KaTeX's HTML as upstream does per environment: the browser build lets the
document parse it (a `<template>`); elsewhere a tokenizer follows the HTML spec's attribute rules (KaTeX's stray `'`
after an `\includegraphics` image's style is an attribute, as for parse5). Attributes map to property ids with
value kinds generated from property-information (scripts/generate-vocabulary.mjs), and to-jsx renders `style`
through a typed port of style-to-js that only the KaTeX plugin installs.

## 4. Repos, bottom to top

Each repo: `~/lil2-<name>`, GitHub `yeargun/lil2-<name>`, Pages `https://yeargun.github.io/lil2-<name>/` (generated
into `docs/` by `~/lilscript-work/lil2/pages/build.mjs`, in the react-markdownlil page layout), npm
`@itslil/lil2-<name>`. Every repo keeps its own code in `src/<layer>/` and embeds the lower layers as sibling
directories (`src/micromark`, `src/mdast`, `src/hast`, `src/jsx`, `src/gfm`, `src/math`, `src/katex`,
`src/breaks`), byte for byte and pinned by hash (`scripts/shared-sources.mjs --sync`). Layers import each other as
`../<layer>/…`, so one copy of each serves the whole program.

1. `lil2-micromark` (HTML out), 2. `lil2-mdast-util-from-markdown`, 3. `lil2-mdast-util-to-hast`,
4. `lil2-hast-util-to-jsx-runtime` (ships inside 5), 5. `lil2-react-markdown`; plugins `lil2-remark-gfm`,
`lil2-remark-math`, `lil2-rehype-katex` (KaTeX stays the `katex` package), `lil2-remark-breaks`.

**Two builds per package**, mirroring decode-named-character-reference's condition map: `dist/` (Node,
workers, Deno, …) carries the 2,125-entry named-reference table; `dist/browser/` (the `browser` condition)
decodes named references with the document's own HTML parser, as upstream's `index.dom.js` does, so the table
is neither downloaded nor unpacked at load. The choice is a `define bool BROWSER` read in a module-level
initializer (`entities = if (BROWSER) {undefined} else {table}`): a define read inside a function is not folded
once the program imports a host module (React).

## 5. Tests

- Differential suites on the corpus (CommonMark spec, every named entity, edge cases, bench documents): HTML,
  events (type, line, column, offset), mdast and hast as rows of indexed arrays (upstream's objects are
  flattened in test code only), React element trees (type, key, props, the component's node row) and
  `renderToStaticMarkup`, react-markdown under nine option sets.
- Browser suites run the `browser` build in Chromium and Firefox against upstream's output.

## 6. Measurement

Per repo: raw/gzip/Brotli against the best of Terser, esbuild and Oxc on the upstream ESM bundle for the same
condition (browser vs browser). Performance: the Playwright harness (`~/lilscript-work/lil2/perf-rm`), steady
state, cold first render and load, Chromium and Firefox, on identical outputs. Final: lil2-react-markdown vs
react-markdown vs react-markdownlil, and every submodule.

## 7. Compiler gaps to watch (found 2026-10-04)

- Field update of a struct stored in an array rebuilds the tuple (`h[a]=[h[a][0],!1,h[a][2]]`). Hence
  struct-of-arrays classes, not arrays of structs.
- `[...xs, y]` snapshots `xs` first.
- `const Record<string>` emits a `{__proto__:null}` literal that front-coding does not pack; a
  `JsValue object {}` is packed.
- A define read inside a function is not folded when the program has an `import extern` (initialization
  analysis treats entry exports as callable at instantiation). See §4 for the workaround; a compiler task is open.
- Load time: V8 parses an arrow's body together with the code that creates it (it defers only `function`
  bodies outside a script's top level), so a module of arrows parses every body at import. The compiler's
  `[delivery] lazy_functions` (branch `lil2-lazy-functions`, commit 90cfbad1, off by default and byte-identical
  when off) spells every function the root creates as `function`; every lil2 config turns it on.
  lil2-micromark's browser build: 13,254 to 13,344 B Brotli; Chromium cold import 6.6 to 5.4 ms (upstream 5.0),
  import plus first run 17.1 to 15.7 ms (upstream 16.1).
- `split` delivery emits one chunk per module group for these programs (27 files for react-markdown's core).
- Search cost: the packages build at effort level 12 (all builds of all packages: about 4 minutes). On lil2-micromark's browser build, level 12 takes 3.1 s
  (13,142 B Brotli); level 13 takes 334 s for 13,078 B (2,428 canonical Brotli encodes and 2,594 formations
  against a fixed probe budget of 1,536); level 15 takes ~570 s and is byte-identical to 13. The level-13
  search does not stop when its gains dry up. A compiler task is open.
- Typed arrays have no `sort` (lists use a merge sort); `pgrep -f` patterns match the calling shell.

## 8. Pinned upstream

micromark 4.0.2, micromark-core-commonmark 2.0.3, decode-named-character-reference 1.3.0,
mdast-util-from-markdown 2.0.3, mdast-util-to-hast 13.2.1, hast-util-to-jsx-runtime 2.3.6,
property-information 7.2.0, react-markdown 10.1.0, remark-gfm 4.0.1, remark-math 6.0.0, rehype-katex 7.0.1,
katex 0.16.22, remark-breaks 4.0.0.
