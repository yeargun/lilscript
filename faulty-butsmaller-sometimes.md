afaik, web browsers are fault tolerant.


that being said, can you explain me all the fault tolerancy they have in syntaxes etc.
in a way that literaly.

i am building the world's most advanced compiler/compressor to js,

and im wondering if we could write code in some way that, js is crazy compact. asf..

and there might be a config that intentionaly make the code maybe syntacticaly a little incorrect, but browser recovers from that, so overal size could get minimized.

i know this is unorthodox something but take your time and do thinker.

while doing so, do explain the differences of different browser engines on such recovering things

==

take your time, find all important ones?

===

Short answer up front, because it reframes your whole project: **HTML and CSS are fault tolerant in a *specified*, deterministic way. JavaScript is not fault tolerant at all.** There is no error recovery in a JS parser. One `SyntaxError` anywhere in a script kills the entire script, including code above the error, because every engine fully parses the source before executing a single statement.

So the "intentionally slightly incorrect JS that browsers recover from" config you're imagining can't exist. What *does* exist is a large surface of **specified permissiveness** in JS (ASI, sloppy mode, Annex B) that looks like error tolerance but is actually normative grammar. That's the exploitable part, and it's safe.

Let me go through all three layers properly.

---

# 1. HTML — genuinely fault tolerant, and it's a spec

This is the important thing most people don't know: since HTML5, error recovery is a **byte-exact state machine** in the spec (WHATWG HTML §13.2). Malformed HTML is not undefined behavior. Chrome, Firefox, and Safari produce the *identical DOM tree* from the same broken input, because they all implement the same ~80-state tokenizer and ~23 insertion modes. There is a shared conformance suite (`html5lib-tests`) they all run against.

That means: **for HTML, you can compress aggressively into "invalid" territory with zero cross-engine risk.**

## 1a. Things that aren't even errors (optional tags)

The spec has a section literally called "Optional tags" (§13.1.2.4). These are *conformant* omissions:

```html
<!DOCTYPE html><title>x</title><p>hello<p>world
```

Full list of omittable tags:
- **Start tags:** `<html>`, `<head>`, `<body>`, `<tbody>`, `<colgroup>`
- **End tags:** `</html>`, `</head>`, `</body>`, `</p>`, `</li>`, `</dt>`, `</dd>`, `</rt>`, `</rp>`, `</optgroup>`, `</option>`, `</thead>`, `</tbody>`, `</tfoot>`, `</tr>`, `</td>`, `</th>`, `</caption>`, `</colgroup>`

Each has conditions (`</p>` may be omitted if followed by a block-level element or if the parent has no more content, etc.), but a table-heavy document loses enormous byte count here.

## 1b. Attribute quoting

Unquoted attribute values are legal. The value must not be empty and must not contain: whitespace, `"`, `'`, `=`, `<`, `>`, or `` ` `` (the backtick rule exists purely because of old IE).

```html
<div class=card id=main data-n=42>
```

**Trap:** in the unquoted-attribute-value state, `/` is *not* a terminator. So:

```html
<a href=/x/>     <!-- href is "/x/", NOT "/x" with a self-close -->
<img src=a.png/> <!-- src is "a.png/" — broken! -->
```

You need `<img src=a.png >` or `<img src="a.png"/>`. Your emitter must special-case trailing slashes.

## 1c. Boolean attributes

```html
<input disabled>          <!-- not disabled="disabled" or disabled="" -->
<script src=a.js defer>
```

## 1d. Actual parse errors that recover deterministically

- **Unclosed tags at EOF** — all open elements are popped and closed.
- **`</br>`** — treated as `<br>` (spec-mandated, in-body insertion mode).
- **Stray end tags** with no matching open element — silently ignored.
- **Duplicate attributes** — first occurrence wins, later ones dropped.
- **Implied `<tbody>`** — `<table><tr>` gets a `tbody` inserted.
- **Foster parenting** — non-table content inside `<table>` gets moved *before* the table in the DOM. Deterministic, but a great way to silently mangle output; make sure your compiler never emits it accidentally.
- **`<div/>` on a non-void element** — the `/` is a parse error and is *ignored*. The div stays open. Self-closing syntax only actually works in foreign content (SVG/MathML).

## 1e. The `&` win

You do not need `&amp;` everywhere. A bare `&` is only a problem if it forms an "ambiguous ampersand" — i.e. it's followed by alphanumerics and then a `;`. In attribute values the rule is even looser (the trailing `=` case). So:

```html
<a href=?a=1&b=2>   <!-- fine, &b= is not a named ref -->
<p>Tom & Jerry      <!-- fine -->
<p>AT&amp;T         <!-- needed? "&T" isn't a named ref, so &T is fine too -->
```

Your compiler should escape `&` *conditionally* by checking against the named-character-reference table, not unconditionally.

## 1f. Non-negotiables

- **Never omit `<!DOCTYPE html>`.** Omitting it triggers quirks mode — different box model, different table cell inheritance, different line-height handling. That's not "recovery," it's a whole alternate rendering mode. 15 bytes, always pay it.
- **`<meta charset=utf-8>` must land in the first 1024 bytes** or the prescan gives up and you fall to a locale-dependent default.

---

# 2. CSS — fault tolerant, also specified, with one nasty exception

CSS Syntax Level 3 defines exact error handling. The core rule: **on a bad declaration, skip forward to the next `;` or the closing `}` of the current block. On a bad at-rule, skip to the end of its block or the next `;`.** This is how progressive enhancement has always worked.

## 2a. Free bytes

```css
a{color:red}          /* last ; in a block is droppable */
a{color:red           /* EOF closes all open blocks — the FINAL } is droppable */
margin:0              /* no unit on zero */
opacity:.5            /* leading zero droppable */
#fff                  /* 3-digit hex; #fffa for 4-digit with alpha */
url(a.png)            /* quotes droppable if no parens/whitespace/quotes */
```

The "drop the very last `}` at EOF" one is guaranteed by the spec's `consume a simple block` step — it's a parse error, and the parse error handling is to return the block. Every engine does it.

## 2b. The exception that will bite you: selector lists

CSS error recovery is per-declaration, but **selectors are all-or-nothing**. One unrecognized selector in a comma-separated list invalidates the *entire rule*:

```css
::-webkit-thing, .btn { color: red }   /* .btn gets NOTHING in Firefox */
```

The fix is forgiving selector lists — `:is()` and `:where()` swallow unknown selectors without killing the rule:

```css
:is(::-webkit-thing, .btn) { color: red }
```

`@media` and `@supports` conditions behave the same way: an unparseable condition drops the whole block.

## 2c. Historical note

The old IE hacks (`*zoom:1`, `_height:1px`, `color:red\9`) were pure exploitation of *divergent* error recovery between engines. That era is over — modern engines agree. Don't build anything on engine-divergent CSS parsing; there isn't any left worth using.

---

# 3. JavaScript — zero error recovery, but lots of legal weirdness

## 3a. Why "slightly incorrect JS" is a dead end

Every engine (V8, SpiderMonkey, JavaScriptCore) does a full **pre-parse** of the whole script or module before executing anything. V8's `PreParser` skips *function bodies* for lazy compilation, but it still validates their syntax. So:

```js
console.log("this never runs");
function f() { let let = ; }   // SyntaxError
```

Nothing runs. No recovery, no partial execution, no "browser figures it out." This is uniform across all engines and is not going to change — it's required for correct early-error semantics (`var`/`let` redeclaration, strict-mode checks, etc.).

There is exactly one nuance: `<script>` elements are independent parse units, so a broken script doesn't kill the next one. That's isolation, not recovery.

## 3b. ASI — the real "tolerance," and it's grammar

Three insertion rules:
1. Offending token is preceded by a line terminator, **or** is `}` → insert `;`
2. EOF reached → insert `;`
3. **Restricted productions**: a line terminator after `return`, `throw`, `break`, `continue`, `yield`, before postfix `++`/`--`, before `=>`, or between `async` and `function` **forces** insertion.

For a minifier this mostly gives you two things:
- Drop `;` before any `}`
- Drop the final `;` at EOF

Rule 3 is the one that will silently break your output if you reorder or wrap statements. Anything that puts a newline between `return` and its argument changes semantics.

## 3c. Sloppy mode vs strict mode vs modules

This is the biggest architectural decision for your compiler, because **modules kill almost every trick below**. Modules are always strict, always deferred, `this` is `undefined` at top level, no `with`, no HTML comments, no implicit globals.

If you want the maximum-compression path, emit **classic scripts in sloppy mode**.

Sloppy-only wins:

```js
x=1                      // implicit global; saves "var " / "let " (4 bytes each)
with(document){write(a)} // scope injection for repeated property access
010                      // legacy octal
function f(a,a){}        // duplicate params allowed
```

`with` is genuinely interesting for your use case — it's the only construct in the language that lets you elide a repeated receiver across arbitrary expressions. It's also the reason V8 deoptimizes the enclosing function and disables most scope analysis. If you're optimizing bytes over speed, it's on the table; if runtime perf matters at all, it isn't.

## 3d. Annex B — normative for browsers, absent in Node's strict paths

Annex B is the "web reality" appendix. Browsers must implement it. Highlights:

**HTML-like comments in scripts:**
```js
<!-- this is a line comment
--> so is this (only at start of a line)
```
Real, spec'd, works in every browser. Not in modules.

**`document.all` and the `[[IsHTMLDDA]]` slot** — the only object in the language that is falsy and `== null` while being a real object:
```js
document.all == null   // true
typeof document.all    // "undefined"
!document.all          // true
```
Handy as a short falsy sentinel if you're truly counting bytes.

**Legacy RegExp grammar** — Annex B relaxes the pattern grammar so unescaped `]`, `{`, `}`, and invalid identity escapes are accepted:
```js
/}/  /]/  /a{/    // all valid in browsers, invalid under the strict grammar
```
Saves backslashes. Note: the `u` and `v` flags opt *out* of Annex B and enforce the strict grammar.

**Block-level function declarations** get web-compat hoisting semantics in sloppy mode.

## 3e. Expression-level byte tricks (all standard, all safe)

```js
!0            // true      (-2)
!1            // false     (-3)
void 0        // undefined (-4)
1/0           // Infinity  (-4)
[]+[]         // ""
new Date      // parens droppable on zero-arg new
f`x`          // tagged template instead of f("x")  (-1, but changes arg shape)
+"1"          // Number("1")
~~x           // Math.trunc for int32 range
x|0           // same
[,,]          // holes
```

Plus the structural ones you'd expect: arrow functions with implicit return, comma operator to avoid block braces, property shorthand, destructuring, `for(;;)` over `while(1)` (equal), label-free `continue`.

---

# 4. Engine differences that actually still exist

Parsing behavior has converged hard. What's left:

**Behavioral, real:**
- **Proper tail calls** — implemented only in JavaScriptCore (Safari). If your compiler does recursion-heavy transforms, Safari won't overflow where V8 and SpiderMonkey will. Don't rely on it.
- **Stack and nesting limits differ.** This one matters for you specifically: if your compressor emits deeply nested ternaries, comma chains, or nested array/object literals, you can hit a parser nesting limit (`RangeError: Maximum call stack size exceeded` *at parse time*) in one engine and not another. V8, SpiderMonkey, and JSC all have different thresholds. **Cap your nesting depth explicitly.**
- **Max string length**: V8 ~2³⁹ chars on 64-bit (~512MB), JSC and SpiderMonkey differ. Relevant if you're doing eval-based self-extraction with a giant payload string.
- **Function argument count limits** differ (matters for `f(...bigArray)` patterns).
- **`Function.prototype.toString`** must return exact source, but engines differ on synthesized/native functions. Don't build source-reading tricks.

**Performance, not correctness:**
- V8 has an **IIFE heuristic**: functions wrapped in parens get eagerly compiled instead of lazily pre-parsed. This is what `optimize-js` exploited. It's largely obsolete now — V8's code cache and lazy-parse improvements made it a wash or a regression. Measure before you do anything here.
- V8, SpiderMonkey, and JSC have different lazy-parse heuristics generally, so how you group functions affects startup cost.

**Dead, don't bother:**
- Conditional catch (`catch(e if c)`), E4X, expression closures — SpiderMonkey-only, all removed.
- JScript conditional compilation (`/*@cc_on`), the IE named-function-expression leak — IE is gone.

---

# 5. The thing that will decide whether your project wins

**Optimize for post-Brotli size, not raw bytes.**

Almost everything ships over `content-encoding: br` or `gzip`. Two consequences that cut against naive golfing:

1. **Brotli has a built-in static dictionary** — 122,784 bytes containing 13,504 common web strings, including things like `<!DOCTYPE html>`, `function`, `</script>`, `text/javascript`, `return`. Canonical, "verbose" forms can compress to *fewer* bytes than your clever short forms, because the short forms aren't in the dictionary and don't repeat. `!0` beats `true` raw; `true` may well beat `!0` after Brotli.

2. **Repetition is worth more than brevity.** Terser's mangler producing hundreds of instances of the same short identifiers gives LZ77 excellent matches. A transform that makes each site *shorter but more unique* can lose. Consistent token ordering, consistent property access shapes, and stable identifier assignment across the whole bundle are worth more than per-expression golf.

This also means the classic self-extracting approach — pack a dictionary into a string, `eval` or `new Function` to unpack — almost always **loses** to Brotli now, and additionally: breaks under any CSP without `unsafe-eval`, defeats code caching (V8 can't cache `eval`'d code the same way), and delays first execution. It's only viable in a size-constrained-no-server context (js13kGames, demoscene).

---

# 6. Practical tier list for your compiler's config surface

**Tier A — spec-guaranteed, ship by default:**
HTML optional tags, unquoted attributes, boolean attributes, conditional `&` escaping, CSS trailing `;` and final `}` drop, zero-unit and leading-zero drops, JS `;`-before-`}` and EOF drops.

**Tier B — safe but mode-gated, expose as flags:**
Sloppy-mode implicit globals, `with`, Annex B HTML comments, Annex B regex relaxations, `document.all` sentinel. All of these require classic-script output; gate them behind an `output: "script" | "module"` switch and hard-error if someone requests both.

**Tier C — measure, don't assume:**
`!0`/`void 0`-style substitutions, IIFE paren hints, eval-based packing. Every one of these needs an A/B against `brotli -q 11` on your actual corpus.

**Tier F — doesn't exist:**
Relying on JS parse-error recovery. There is none.

---

# 7. Testing infrastructure you'll want

- **HTML**: `html5lib-tests` (the shared conformance suite) and `parse5`/`html5ever` as reference implementations. If your output round-trips to the same DOM through those, all three browsers agree. This is a hard guarantee, not a heuristic.
- **CSS**: implement against css-syntax-3's algorithms directly; `csstree` follows them closely.
- **JS**: test262 for grammar validity, plus a differential harness that runs your output in V8, SpiderMonkey, and JSC (all three ship standalone shells: `d8`, `js`, `jsc`). Since JS has no recovery, "did it parse in all three" is a cheap, decisive test — run it on every emit.
- **Nesting fuzzer**: specifically fuzz your output's expression nesting depth against all three shells to find your safe ceiling.

The core mental model to hold onto: HTML's forgiveness is a *feature you can spend*, CSS's is a *feature with one sharp edge*, and JavaScript's is *not forgiveness at all* — it's a permissive grammar that you must stay inside of exactly.

---

# 8. LilScript compiler audit (2026-08-08)

This file is now treated as an optimization research queue, not as permission
to emit malformed assets.

## Current compiler scope

LilScript currently emits JavaScript or ESM chunks. It does not own an HTML or
CSS serialization/minification stage. HTML optional-tag and CSS EOF-closing
ideas therefore belong in a future deploy-asset optimizer, where they can be
accepted only after an exact DOM/CSSOM equivalence check. Adding an unused
"browser recovery" switch to the JavaScript backend would provide no safe
capability.

## Implemented, standards-valid, and tunable

`javascript.compression = ["standard-grammar-elision"]` permits three
independent complete-artifact candidates:

- omit a generated block-terminal or EOF semicolon only where ECMAScript's
  [ASI rules](https://tc39.es/ecma262/multipage/ecmascript-language-lexical-grammar.html#sec-automatic-semicolon-insertion)
  supply it;
- spell a zero-argument construction as `new C`, which the
  [ECMAScript `new` evaluation rules](https://tc39.es/ecma262/multipage/ecmascript-language-expressions.html#sec-new-operator)
  define as construction with an empty argument list;
- remove redundant grouping from call chains (`(f()).x` -> `f().x` and
  `(f())[x]` -> `f()[x]`) while retaining required grouping for a no-argument
  `new` receiver (`(new Map).size`).

The permission is one config item, but the search dimensions remain separate.
That matters: in a local repeated-function probe, terminal-semicolon elision
reduced raw bytes from 30,980 to 30,180 and gzip from 3,971 to 3,956, but made
Brotli-11 worse (1,674 to 1,688). The compiler therefore keeps punctuation
variants in its bounded beam and lets the configured raw/gzip/Brotli encoder
score the complete artifact. `compression = []` disables this family.

`javascript.startup.max_nesting` is also available as an optional absolute
ceiling. Unlike relative parse/compile/memory overhead limits, it can reject a
small candidate whose nested ternaries, comma expressions, arrays, or blocks
would exceed a project's cross-engine safety budget.

Local parser probes (Node 20.12/V8 and Bun 1.1/JSC) found no material penalty
from the accepted spellings; the shorter repeated probes were generally a
little faster to parse. Those are development proxies, not browser-performance
claims, so release artifacts still need pinned Chromium, Firefox, and WebKit
measurements.

## Deliberately not implemented

- Invalid JavaScript recovery: a syntax error rejects the script/module.
- Implicit globals, `with`, and Annex B spellings: they change semantics,
  conflict with modules/strict mode, or damage optimization.
- `document.all`: host-specific observable semantics, not a spelling choice.
- `eval`/`Function` self-unpacking: CSP, code-cache, parse, and startup costs.
- Tagged templates in place of calls: the argument protocol is different.
- Unproved `~~x`, `x|0`, `+x`, or truthiness substitutions: retain only where
  LilScript's typed/range analysis proves the language-level equivalence.
- HTML duplicate attributes, foster parenting, implied table structure, and
  generic self-closing syntax: recovery can change the DOM even when every
  browser follows the same algorithm.

For a future HTML/CSS optimizer, use the normative
[HTML optional-tag conditions](https://html.spec.whatwg.org/multipage/syntax.html#optional-tags)
and [CSS Syntax EOF rules](https://drafts.csswg.org/css-syntax/#error-handling),
then require identical DOM/CSSOM plus style/layout and parse-time regression
gates. Interoperable recovery alone is not an equivalence proof.
