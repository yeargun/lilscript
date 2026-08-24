# The hardcoded library, for a JavaScript emitter

Parent: [index](README.md). Regenerate with `node dict-view.mjs`. To browse it
interactively, including every transform applied to any word, use the
dictionary section of [Brotli, the whole machine](../brotli-machine.html).

Brotli ships 122,784 bytes both sides already have: 13,504 words of length
4–24, plus 121 transforms that wrap, cut or capitalise them. It was sampled
from the web of about 2014, and it shows.

## Shape

| length | words | first entry | last entry |
|---:|---:|---|---|
| 4 | 1024 | `time` | (Arabic) |
| 5 | 1024 | `first` | `aguas` |
| 6 | 2048 | `&quot;` | (Arabic) |
| 7 | 2048 | `profile` | `mejorar` |
| 8 | 1024 | `position` | four spaces |
| 9 | 1024 | `resources` | (Hindi) |
| 10 | 1024 | `categories` | (Arabic) |
| 11 | 1024 | `sByTagName(` | `conformidad` |
| 12 | 1024 | `line-height:` | (Arabic) |
| 13 | 512 | `entertainment` | `construcción` |
| 14 | 512 | `"><div class="` | (Arabic) |
| 15 | 256 | `cursor:pointer;` | (Hindi) |
| 16 | 128 | `rss+xml" title="` | (Arabic) |
| 17 | 128 | `robots" content="` | `occasionally used` |
| 18 | 256 | `position:absolute;` | (Arabic) |
| 19 | 128 | `keywords" content="` | `have children under` |
| 20 | 128 | `%3E%3C/script%3E"));` | (Arabic) |
| 21 | 64 | `html; charset=UTF-8" ` | (Hindi) |
| 22 | 64 | `description" content="` | (Russian) |
| 23 | 32 | `<!DOCTYPE html PUBLIC "` | `input type="hidden" nam` |
| 24 | 32 | `<script type="text/javas` | (Hindi) |

5,399 of the 13,504 words are legal JavaScript identifiers standing alone. A
large minority of the rest is CJS/HTML/CSS punctuation — `sByTagName(`,
`"><div class="`, `cursor:pointer;`, `%3E%3C/script%3E"));` — and a large
fraction is natural-language text in a dozen languages, which a minified
artifact will never touch.

## Which spellings are exactly one reference

The useful question is not "is the bare word in there" but "is there a word
plus a transform that spells exactly the bytes I have to emit". 50 of 63
probed JavaScript spellings are:

| spelling | how the dictionary serves it |
|---|---|
| `function` | word `function`, identity |
| `function ` | word `function ` (with the space), identity |
| `function(` | word `function(){`, OMIT_LAST_2 |
| `=function(` | word `=function(){`, OMIT_LAST_2 |
| `){return ` | word `){return`, `+ " "` |
| `);return ` | word `);return `, identity |
| `for(var ` | word `for(var `, identity |
| `}else{` | word `}else{`, identity |
| `var ` | word `var `, identity |
| `typeof ` | word `typeof `, identity |
| `this.` | word `this.`, identity |
| `.length` | word `.length`, identity |
| `.prototype` | word `prototype`, `"." + word` |
| `.call(` | word `call`, `"." + word + "("` |
| `.apply(` | word `apply`, `"." + word + "("` |
| `.push(` | word `.push(`, identity |
| `.indexOf(` | word `.indexOf("`, OMIT_LAST_1 |
| `.toString` | word `toString`, `"." + word` |
| `Math.` | word `Math.floor(`, OMIT_LAST_6 |
| `Object.` | word `Object`, `word + "."` |
| `JSON.` | word `JSON`, `word + "."` |
| `Array` | word `array`, UPPERCASE_FIRST |
| `Promise` | word `promise`, UPPERCASE_FIRST |
| `undefined`, `null`, `true`, `false` | identity |
| `document`, `window`, `.document` | identity or `"." + word` |
| `addEventListener`, `className`, `innerHTML` | identity |

And 13 are not, including several that a modern emitter uses constantly:

| spelling | best the dictionary can do |
|---|---|
| `let ` | nothing |
| `const ` | the first 5 bytes, `const` |
| `=>{` | nothing |
| `await ` | nothing |
| `if(` | nothing |
| `.prototype.` | the first 10 bytes, `.prototype` |
| `constructor` | the first 9 bytes, `construct` |
| `getElementById` | nothing |
| `parentNode` | the first 6 bytes, `parent` |
| `nodeType` | the first 4 bytes, `node` |
| `childNodes` | the first 5 bytes, `child` |

The dictionary is fluent in ES5 and in the DOM of 2014. It has never heard of
`let`, `const`, arrows, or `await`.

78 of the 121 transforms carry code punctuation, which is why so many of the
hits above are `word + "("`, `"." + word`, `word + ":"`, `word + "]"`. The
transform table is the part of the dictionary that a code generator can
actually aim at.

## What our own artifacts pull out of it

| corpus | references | bytes served | share of output | distinct entries | reused |
|---|---:|---:|---:|---:|---:|
| jquery-min | 529 | 3,780 | 4.32% | 529 | 0 |
| jquery-src | 2,291 | 20,497 | 7.18% | 2,291 | 0 |
| jquery-lil-raw | 545 | 3,859 | 3.76% | 545 | 0 |
| jquery-lil-min | 491 | 3,530 | 3.19% | 491 | 0 |
| glmatrix-js-vite | 81 | 523 | 0.71% | 81 | 0 |
| glmatrix-lil-vite | 71 | 467 | 0.68% | 71 | 0 |
| glmatrix-lil-raw | 118 | 785 | 0.67% | 118 | 0 |

Entries our streams actually reach, across all corpora: `typeof `, `var `,
`Object`, `Property`, `Array`, `object`, `);return `, `function `, `.toString`,
`define`, `ARRAY`, `TYPE`, `RANDOM`, `equal`, `Type`, ` jQuery `,
` and other `, `contribut`, `.document`, `window`, `global`, `This`, `push`,
`arguments`.

Two things to read off that list. First, the copyright banner is a meaningful
share of the dictionary hits on jQuery — ` jQuery `, ` and other `,
`contribut`, `license `, `jquery.`, `org/` — which is a real effect and not one
we can act on. Second, the code hits are exactly the ES5 skeleton: `var `,
`typeof `, `function `, `);return `, `.toString`.

## What this rules in and out

- **Out:** using dictionary words as identifiers. There is no rate here to
  exploit; see [03](03-dictionary-as-names.md).
- **Out:** spelling arrows as `function` to reach `function(){`. Already
  measured at +274 Brotli in
  [the audits](../brotli-global-mangle/09-audits.md), and the census explains
  why: the win is one reference, the cost is every later occurrence.
- **In, marginally:** when a *first occurrence* of a long literal is being
  chosen and the alternatives are otherwise equal, the spelling the dictionary
  serves is free. That is a tie-break on pooled strings and host names, worth
  tens of bytes per artifact, not a strategy. [06](06-free-order.md) measures
  the pooled-string version of this.
- **In, as an explanation:** `var ` is in the dictionary and `let ` is not.
  That is worth exactly one reference, so it does not explain the ±44–92 byte
  `var`/`let` swings the playbook measured; those come from the keyword's
  interaction with the rest of the file's letters, not from the ROM.
