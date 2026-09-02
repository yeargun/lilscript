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
