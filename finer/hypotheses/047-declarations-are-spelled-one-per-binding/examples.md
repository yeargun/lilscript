# katexlil vs Terser: the same functions, side by side

Left: `dist/katex.raw.js` from katexlil 70f2f6b (lilscript feature/source-maps + e0c1c22),
65586 Brotli as the shipped ESM. Right: katex@0.16.22's Flow sources through esbuild and
Terser (`scripts/lib/official.mjs fromSource`), 61758. Pairs found by shared string literals;
the `.lil` and upstream source are quoted where the difference is the port's spelling.

## 1. `Span.toMarkup` — the port spells `for-in` as `Object.keys` + a counter loop

upstream `src/domTree.js`:
```js
for (const style in this.style) {
    if (this.style.hasOwnProperty(style)) {
        styles += `${utils.hyphenate(style)}:${this.style[style]};`;
    }
}
```
port `src/domTree.lil`:
```lilscript
JsValue keys16 = objectKeys(obj15);
int ki17 = 0;
int kn18 = len(keys16);
while (ki17 < kn18) {
  JsValue style = keys16[ki17];
  if (hasOwn(self["style"], style)) {
    styles = JS.add(styles, toStr(JS.invoke(utils, "hyphenate", style)) + ":" + toStr(self["style"][style]) + ";");
  }
  ki17 = ki17 + 1;
}
```
LilScript output:
```js
for(var i,e,h=oa(this.style),f=h.length,b="",d=0;d<f;d++)i=h[d],fa(this.style,i)&&(e=ba.hyphenate(i)+":",b=b+(e+this.style[i]+";"));
```
Terser output:
```js
for(const e in this.style)this.style.hasOwnProperty(e)&&(r+=`${y.hyphenate(e)}:${this.style[e]};`);
```
`oa` is the port's `objectKeys` helper (`Object.keys`), `fa` its `hasOwn` (`Object.prototype.hasOwnProperty.call`):
121 calls to five such helpers in the artifact. The compiler did its part (the `while` became a
`for`, the helpers are two letters), but it cannot turn a key array walk back into `for-in`.
Port shape.

## 2. `renderError` — arrays built by push instead of written as literals

LilScript:
```js
de=(a,b,c)=>{var i;if(c.throwOnError||!ga(a,Y))throw a;i=[],Array.prototype.push.call(i,new pa(b)),b=[],Array.prototype.push.call(b,"katex-error"),b=W.makeSpan(b,i),b.setAttribute("title",a.toString()),b.setAttribute("style","color:"+c.errorColor);return b}
```
Terser:
```js
Kn=function(e,t,r){if(r.throwOnError||!(e instanceof u))throw e;const n=Ye.makeSpan(["katex-error"],[new ie(t)]);return n.setAttribute("title",e.toString()),n.setAttribute("style",`color:${r.errorColor}`),n}
```
The port writes `newArray()` + `push()` where upstream writes `["katex-error"]`; `JS.array(x)`
exists for exactly this. `ga(a,Y)` is the port's `instanceOf` helper
(`a==null?!1:…&&b.prototype.isPrototypeOf(a)`) where upstream has `instanceof`. 86
`Array.prototype.push.call` sites remain after every fold the compiler has. Port shape.
(Spelling `JS.push` as a method invoke made it worse, +696: the intrinsic is what the
compiler's array folds recognise, an opaque call is not.)

## 3. `sqrt` html builder — a `+` on every operand, and one expression split into four

LilScript:
```js
var i=b.fontMetrics().defaultRuleThickness,c=b.style.id<$.TEXT.id?b.fontMetrics().xHeight:i;c=+c,c=i+c/4,i=sa.sqrtImage(d.height+d.depth+c+i,b);var h=i.span,e=i.ruleWidth,f=i.advanceWidth,g=+h.height-+e;g>d.height+d.depth+c&&(c=+(c+g),c=+(c-+d.height),c=+(c-+d.depth),c=c/2)
```
Terser:
```js
const n=t.fontMetrics().defaultRuleThickness;let o=n;t.style.id<C.TEXT.id&&(o=t.fontMetrics().xHeight);let s=n+o/4;const a=r.height+r.depth+s+n,{span:i,ruleWidth:l,advanceWidth:h}=Tr.sqrtImage(a,t),c=i.height-l;c>r.height+r.depth+s&&(s=(s+c-r.height-r.depth)/2)
```
Every `JsValue` operand is coerced (`+h.height`, `+e`: 115 member coercions in the artifact,
Terser has 2), and the port assigns through `toNum()` step by step, so one subtraction chain is
four statements each re-coerced: `+(c-+d.height)`. Two parts: the port's untyped fields (a
`number height` field needs no `+`), and a generic compiler gap — `+(a-b)` is already a number,
64 such sites (Terser 3), and `-1` is spelled `0-1` at 2 sites. The latter two are folds to write.

## 4. `MacroExpander.expandNextToken` / `isDefined` — where the shapes are close

LilScript:
```js
expandNextToken(){while(!0)if(!1===this.expandOnce()){var a=this.stack.pop();!a.treatAsRelax||(a.text="\\relax");return a}}
isDefined(a){var b=this.macros.has(a);b=b||fa(Ja,a),b=b||fa(ia.math,a),b=b||fa(ia.text,a),b=b||fa(Tc,a);return b}
```
Terser:
```js
expandNextToken(){for(;;)if(!1===this.expandOnce()){const e=this.stack.pop();return e.treatAsRelax&&(e.text="\\relax"),e}throw new Error}
isDefined(e){return this.macros.has(e)||zn.hasOwnProperty(e)||ge.math.hasOwnProperty(e)||ge.text.hasOwnProperty(e)||Pn.hasOwnProperty(e)}
```
The class is a real `class` now (047's rewrite), the unreachable `throw` is gone, `while(!0)`
costs two bytes over `for(;;)`, `!x||(…)` one over `x&&(…)` (23 sites). `isDefined` is the port:
the `||` chain was written as five `b = b || …` statements, so a temporary and four assignments
stay. Port shape, and small.

## 5. `\href` — where the compiler is ahead

LilScript:
```js
handler:(a,b)=>{var c=a.parser,d=b[1],i=da(b[0],"url").url;if(!c.settings.isTrusted({command:"\\href",url:i}))return c.formatUnsupportedCmd("\\href");a=c.mode;return{type:"href",mode:a,href:i,body:ma(d)}},htmlBuilder:(a,b)=>W.makeAnchor(a.href,[],ca.buildExpression(a.body,b,!1),b),mathmlBuilder:(a,b)=>{b=aa.buildExpressionRow(a.body,b),ga(b,ra)||(b=new ra("mrow",[b])),b.setAttribute("href",a.href);return b}
```
Terser:
```js
handler:({parser:e},t)=>{const r=t[1],n=Lt(t[0],"url").url;return e.settings.isTrusted({command:"\\href",url:n})?{type:"href",mode:e.mode,href:n,body:st(r)}:e.formatUnsupportedCmd("\\href")},htmlBuilder:(e,t)=>{const r=mt(e.body,t,!1);return Ye.makeAnchor(e.href,[],r,t)},mathmlBuilder:(e,t)=>{let r=Nt(e.body,t);return r instanceof vt||(r=new vt("mrow",[r])),r.setAttribute("href",e.href),r}
```
Most `functions/*` modules look like this and are smaller than Terser's by 50–170 Brotli each
(046's map): plain functions over plain data, which is what the compiler is good at. The port
loses where it hand-builds what the language would give it: classes, arrays, `for-in`, typed
numbers.

## What this adds up to

| where the bytes go | evidence | lever |
|---|---|---|
| helper layer (`fa`/`oa`/`ga`/`ta`/`na`, `push`/`newArray`, `toStr`/`toNum`) | 121 helper calls, 86 `Array.prototype.push.call`, 115 `+member` coercions, 42 `+""` | port: write `JS.array(...)`, typed `number`/`string` fields, `for-in` where the language has it |
| redundant coercions the compiler could drop | 64 `+(a-b)`, 2 `0-1` | compiler, generic fold |
| Terser's remaining extraction from our artifact | −811: `unused` +241, `collapse_vars` +175, `evaluate` +162 | compiler (045) |
| internal field/method names | all 117 renamed: −581 | port typing (structure, not names) |

## Proof under Brotli, not raw

Every shape above, rewritten across the shipped ESM (katexlil 70f2f6b, 65586 Brotli) and scored
with `lilscript-codec`. Only the Brotli column counts; the raw column is why these looked like
levers. Files: `scripts/.proof.mjs` in the session, variants parsed with `node --check`.

| rewrite | sites | raw | gzip | **Brotli** | verdict |
|---|---:|---:|---:|---:|---|
| `Array.prototype.push.call(x,v)` → `x.push(v)` | 86 | −1806 | −101 | **−71** | what typed arrays would buy; the real port change (`JS.invoke`) was +696 because it hides the intrinsic from the array folds |
| `new RegExp("…")` → `/…/` | 44 | −706 | −71 | **−68** | compiler fold, gated on pristine builtins; needs the peephole lexer to stay certain about `/` |
| `ga(a,Y)` (helper) → `a instanceof Y` | 26 | +182 | +45 | **−60** | raw *bigger*, Brotli smaller: the helper's call shape repeats worse than the keyword |
| `!x\|\|(…)` → `x&&(…)` | 23 | −23 | −26 | **−38** | compiler spelling |
| `+x.y` coercions dropped | 93 | −93 | −40 | **−18** | what typed number fields would buy |
| `+""` coercions dropped | 41 | −123 | −54 | **−16** | what typed string fields would buy |
| `+(a-b)` → `(a-b)` | 61 | −61 | +6 | −3 | noise |
| `x!==void 0&&x!=null` → `x!=null` | 8 | −96 | −21 | 0 | noise |
| `fa(x,k)` (helper) → `x.hasOwnProperty(k)` | 35 | +420 | −7 | +2 | noise |
| `oa(x)` (helper) → `Object.keys(x)` | 20 | +180 | +10 | +24 | the helper is *better* |
| `while(!0)` → `for(;;)` | 9 | −18 | +2 | +52 | worse |
| `0-1` → `-1` | 4 | −4 | −4 | +60 | worse: four bytes moved 60, which is the noise floor of this artifact |
| all semantics-preserving rows together | | −1932 | −190 | **−173** | |
| everything, type simulations included | | −2148 | −290 | **−199** | |
| all 117 internal field/method names shortened | 1236 | −7737 | −921 | **−581** | names are not where the bytes are |
| symbols: inlined `"math","main","rel"` → 2-byte names (module alone) | 646 | −7291 | | −44 | the call spelling is not the module's loss |
| symbols: `,i)` → `,!0)` (module alone) | 600 | +364 | | +20 | |

What is proven: the visible shape differences are worth about 200 Brotli together, names about
600, and Terser's remaining passes 811 (`unused` 241, `collapse_vars` 175, `evaluate` 162). The
gap to Terser of the published graph is 2542. The rest is not localised: per-module attribution
through the compiler's map charges inlined and hoisted code to the module of its first token
(`functions/font` on our side carries buildCommon's `makeVList` positioning), and the two
ownership modes disagree by ±300 per module, so a module table cannot be used as a proof here.
Whole-artifact rewrites can, and they are what the table above is.

## Bottom-up, at the function boundary (2026-09-02, later)

`scripts/function-pairs.mjs` in katexlil collects every outermost function body in both
artifacts (a body is a unit wherever it sits), compresses each alone, each lane's bodies
concatenated, and the artifact with the bodies cut out (the glue), with `lilscript-codec`.

| measurement | LilScript | Terser (sources) | delta |
|---|---:|---:|---:|
| every function body compressed alone, summed (478 / 462 bodies) | 70113 | 71113 | **−1000** |
| all bodies concatenated | 35278 | 32431 | **+2847** |
| the glue (artifact minus bodies; Terser's includes the 9462-byte font table) | 21070 | 29416 | +1116 like for like |
| whole artifact, code only | 56236 | 52263 | +3973 |
| 78 content-matched pairs within 10 % raw of each other, in-context cost | 6377 | 3471 | ours costs 84 % more for the same code |
| matched bodies that cost ≤ 5 bytes in context (near-duplicates) | 1 | 22 | |
| candidate search off (`candidate_search = "off"`, level 13) | 59477 | | +3241: the search is a large net win, not the source of the variety |
| `candidate_search = "production"` (gated binary) | 56236 | | byte-identical to `always` |

Function by function we are already smaller. What we lose is *collective*: KaTeX's builders and
handlers are near-identical upstream and Terser keeps them near-identical, so Brotli copies each
next one for a few bytes (`isCharacterBox`: 87 raw, 3 Brotli in context; ours 91 raw, 77 Brotli).
Our versions of the same functions differ from each other because the port's transliteration
spells the same idiom several ways (`or3`/`or4` temporaries for `||`, `toStr` here and `+""`
there, `while` here and `for` there) and the compiler compiles each faithfully; turning the search
off makes it 3241 worse, so the search is not what creates the variety. Terser's own passes on our
artifact recover only 576 of the 2847 (compress-only), which says the variety is structural, not
a spelling a minifier reaches. The per-module attribution through the compiler's map is not usable
at this granularity (it charges inlined bodies to the module of their first token); content
matching by shared string literals is.

Language gap seen on the way: no `instanceof` on `JsValue`, so the port carries an `isPrototypeOf`
helper; spelled as `instanceof` it is −60 Brotli (raw +182).

## Follow-ups landed on the compiler (5ceeb59)

| change | proof before | pool A/B (katexlil / mobx / micromark / remark-gfm, Brotli) |
|---|---|---|
| `!a.b.c\|\|(…)` → `a.b.c&&(…)` (the fold took bare names only) | −38 on the ESM by rewrite | with the regex candidate: **+7 / +12 / −1 / −41** — neutral |
| `new RegExp("…","f")` → `/…/f`, a late codec-scored candidate | −68 by rewrite | (same run; 38 of 44 literals landed, compiler output −36, ESM +7) |
| `x=E1,x=x OP …` → `x=(E1)OP …`, declarator first link allowed | in context: `isCharacterBox` 77 vs Terser 3, `isDefined` chains; the peephole left them whole | **−79 / −7 / +14 / −31** (build I; 29 chains left where a link is not adjacent or reads the name) |

The neutral pair is the noise floor again (±60): two rewrites that were −106 alone on the ESM
add up to +7 on the compile, because the search re-decides around them. Kept: generic, prior art,
codec-scored where they can be wrong.

## Port idioms the language already has (katexlil 91ffa74)

| port change | sites | same compiler, shipped ESM Brotli |
|---|---:|---:|
| `Object.keys` counter loops → `for (string key in value)` (mathMLTree already used it) | 19 | **−165** (65507 → 65342), 17/17, Jest 1230/1230 |

The transliterator spelled upstream's `for…in` as `objectKeys` + a counter `while` in 19 places
and as the language's `for…in` in 2; one idiom, one spelling, is what the collective similarity
finding predicts, and it paid. The census of the port's other spellings: `JS.add` 422, `toStr`
290, `toNum` 499, `JS.strictEqual` 523, `JS.invoke` 1326, `undef()` 2387 — consistent, so the
next candidates are the shapes, not the helpers: the counter `while` (`int i = 0; while (i < n)
{…; i = i + 1;}`) where the language has `for (int i = 0; i < n; i++)`.
