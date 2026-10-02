// Differential harness for the core group (core, core/*, data, deferred, callbacks):
// node harness.mjs <path/to/jquery.esm.js> <out.json>
// Runs every case against official jquery@3.7.1 and the given build in one jsdom
// window and records the serialized result of each. compare.mjs diffs two runs.
import { createRequire } from "node:module"
import { writeFileSync } from "node:fs"
import { pathToFileURL } from "node:url"
const require = createRequire("/home/azureuser/jquerylil/test/compat.test.mjs")
const { JSDOM } = require("jsdom")

const [esmPath, outPath] = process.argv.slice(2)
const dom = new JSDOM("<!doctype html><html><head></head><body></body></html>", { pretendToBeVisual: true, runScripts: "dangerously", url: "http://example.test/page.html" })
globalThis.window = dom.window
globalThis.document = dom.window.document
const officialFactory = require("jquery")
const official = officialFactory.fn?.jquery ? officialFactory : officialFactory(dom.window)
const lil = (await import(pathToFileURL(esmPath).href)).jQuery
const W = dom.window
const D = W.document
const sleep = (ms) => new Promise((r) => setTimeout(r, ms * 4))

// ---------- serializer ----------
function ser($, v, depth = 0, seen = new Set()) {
  if (v === undefined) return "undef"
  if (v === null) return "null"
  const t = typeof v
  if (t === "number") return Object.is(v, -0) ? "n:-0" : "n:" + String(v)
  if (t === "string") return JSON.stringify(v.includes("\n    at ") ? v.split("\n")[0] + "\n[stack]" : v)
  if (t === "boolean") return String(v)
  if (t === "symbol") return "sym:" + String(v.description)
  if (t === "bigint") return "big:" + v
  if (v === $) return "$"
  if (v === $.fn) return "$.fn"
  if (v === W) return "window"
  if (v === D) return "document"
  if (t === "function") return "fn"
  if (seen.has(v)) return "[cycle]"
  if (depth > 6) return "[deep]"
  seen = new Set(seen); seen.add(v)
  if (v instanceof W.Node) {
    if (v.nodeType === 1) return "<" + v.outerHTML + ">"
    if (v.nodeType === 3) return "#text:" + JSON.stringify(v.data)
    if (v.nodeType === 8) return "#comment:" + JSON.stringify(v.data)
    if (v.nodeType === 9) return "#doc:" + (v.documentElement ? v.documentElement.nodeName : "")
    if (v.nodeType === 11) return "#frag[" + [...v.childNodes].map((n) => ser($, n, depth + 1, seen)).join(",") + "]"
    return "#node" + v.nodeType
  }
  if (v instanceof Error || v instanceof W.Error) return "Error(" + v.name + ":" + v.message + ")"
  if (v.jquery && typeof v.length === "number" && typeof v.pushStack === "function") {
    const els = []
    for (let i = 0; i < v.length; i++) els.push(ser($, v[i], depth + 1, seen))
    return "$[" + els.join(",") + "]"
  }
  if (typeof v.promise === "function" && typeof v.state === "function" && typeof v.then === "function") {
    return "promise(" + v.state() + (typeof v.resolve === "function" ? ",deferred" : "") + ")"
  }
  if (Array.isArray(v)) return "[" + v.map((x) => ser($, x, depth + 1, seen)).join(",") + "]"
  if (typeof v.length === "number" && typeof v.item === "function") return "list[" + [...v].map((x) => ser($, x, depth + 1, seen)).join(",") + "]"
  const keys = Object.keys(v)
  return "{" + keys.map((k) => JSON.stringify(k) + ":" + ser($, v[k], depth + 1, seen)).join(",") + "}"
}

// ---------- cases ----------
const cases = []
const C = (name, fn) => cases.push([name, fn])
const fixture = `<div id="main" class="m"><p id="p1" class="a">One<b>bold</b></p><p id="p2" class="a b" data-foo="bar" data-num="42" data-float="1.50" data-json='{"x":1}' data-arr="[1,2]" data-t="true" data-f="false" data-n="null" data-exp="1e3" data-foo-bar="dash" data-empty="">Two</p><span id="s1">S<!--c--></span><ul id="ul"><li>a</li><li>b</li><li>c</li></ul><input id="in" type="text" value="v"></div>`
const reset = () => { D.body.innerHTML = fixture }
const tryit = (f) => { try { return f() } catch (e) { return "threw:" + (e && e.name) } }

// core: construction
C("ctor empty forms", ($) => [$(), $(null), $(undefined), $(""), $(false), $(0), $(NaN)].map((j) => [j.length, j instanceof $, j.jquery]))
C("ctor html single", ($) => [$("<div>"), $("<div/>"), $("<div></div>"), $("<DIV>"), $("<img>"), $("<span/>")].map((j) => [j.length, j[0].nodeName, j[0].parentNode === null]))
C("ctor html multi", ($) => $("<p>hi</p><span>x</span>text"))
C("ctor html whitespace", ($) => [$("  <b>x</b>  "), $("\n<i>y</i>"), $("<b>x</b> trailing"), $("text <b>x</b>")].map((j) => [j.length, ser($, j)]))
C("ctor html attrs", ($) => { const j = $("<div>", { id: "made", "class": "c1", title: "t", text: "hello" }); return [j[0].outerHTML, j.length] })
C("ctor html attrs non-single", ($) => { const j = $("<div></div><p></p>", { id: "x" }); return [j.length, j[0].outerHTML, j[1].outerHTML] })
C("ctor html context doc", ($) => [$("<div>", D).length, $("<div>", $(D))[0].ownerDocument === D, $("<b>a</b>", D.body)[0].nodeName])
C("ctor id", ($) => { reset(); return [$("#p1"), $("#nope"), $("#p1").length, $("#p1")[0] === D.getElementById("p1")] })
C("ctor id with context", ($) => { reset(); return [$("#p1", D), $("#p1", D.getElementById("main")), $("#p1", $("#main")), $("#p1", D.getElementById("ul"))] })
C("ctor selector", ($) => { reset(); return [$("p"), $("p.a"), $("li", "#ul"), $("li", D.getElementById("ul")), $("li", $("#ul")), $("#main p").length, $("b", $("p"))] })
C("ctor selector invalid", ($) => { reset(); return tryit(() => $("p[").length) })
C("ctor dom", ($) => { reset(); const e = D.getElementById("p1"); const j = $(e); return [j.length, j[0] === e, $(D).length, $(D)[0] === D, $(W).length, $(W)[0] === W, $(D.createTextNode("t")).length] })
C("ctor array-like", ($) => { reset(); const ps = D.querySelectorAll("p"); return [$(ps), $([1, 2, 3]).length, $({ a: 1 }).length, $({ a: 1 })[0].a, $({ length: 2, 0: "x", 1: "y" }).length, $([]).length, $($("p")).length, $($("p"))[0] === $("p")[0]] })
C("ctor jquery clone", ($) => { reset(); const a = $("p"); const b = $(a); return [b !== a, b.length, b[0] === a[0], b.prevObject === undefined] })
C("ctor function", async ($) => { const out = []; const r = $(function (arg) { out.push(arg === $, this === D) }); out.push(r.length, r.jquery); await sleep(5); return out })
C("ctor new", ($) => tryit(() => { const j = new $("<div>"); return [j.length, j[0].nodeName] }))
C("ctor instanceof", ($) => { const j = $("<div>"); return [j instanceof $, j instanceof $.fn.init, $.fn.init.prototype === $.fn, $.prototype === $.fn, j.constructor === $] })

// core: fn methods
C("fn basics", ($) => [$.fn.jquery, $.fn.length, $.fn.constructor === $, typeof $.fn.init])
C("fn toArray/get", ($) => { reset(); const j = $("li"); return [j.toArray(), j.get(), j.get(0), j.get(1), j.get(-1), j.get(-3), j.get(-4), j.get(5), j.get(null), j.get(undefined), j.get("1"), j.get("-1"), $().get(), $().get(0), Array.isArray(j.toArray())] })
C("fn pushStack/end", ($) => { reset(); const j = $("li"); const s = j.pushStack([D.body]); return [s.length, s[0] === D.body, s.prevObject === j, s.end() === j, j.end().length, j.end() instanceof $, $("li").eq(1).end().length, s.pushStack($("p")).length] })
C("fn each", ($) => { reset(); const out = []; const r = $("li").each(function (i, el) { out.push(i, this === el, el.textContent); if (i === 1) return false }); return [out, r.length, r.jquery] })
C("fn each nonfalse", ($) => { reset(); const out = []; $("li").each(function (i) { out.push(i); return 0 }); return out })
C("fn map", ($) => { reset(); const j = $("li").map(function (i, el) { return i === 1 ? null : i === 2 ? [el.textContent, "z"] : this.textContent }); return [j, j.length, j.prevObject.length, $().map(() => 1).length] })
C("fn map undefined", ($) => { reset(); return $("li").map((i) => (i === 0 ? undefined : [[i]])).get() })
C("fn slice", ($) => { reset(); const j = $("li"); return [j.slice(1), j.slice(-1), j.slice(0, 2), j.slice(), j.slice(5), j.slice(1, -1), j.slice("1"), j.slice(1).prevObject === j] })
C("fn first/last/eq", ($) => { reset(); const j = $("li"); return [j.first(), j.last(), j.eq(0), j.eq(1), j.eq(-1), j.eq(-3), j.eq(-4), j.eq(3), j.eq(), j.eq(null), j.eq("1"), j.eq("-1"), j.eq(1.5), j.eq(true), $().first().length, j.eq(1).prevObject === j] })
C("fn even/odd", ($) => { reset(); const j = $("li"); return [j.even(), j.odd(), $().even().length, j.even().prevObject === j] })
C("fn push/sort/splice", ($) => [$.fn.push === Array.prototype.push, $.fn.sort === Array.prototype.sort, $.fn.splice === Array.prototype.splice])
C("fn iterator", ($) => { reset(); return [[...$("li")].length, typeof $.fn[Symbol.iterator], $.fn[Symbol.iterator] === Array.prototype[Symbol.iterator]] })
C("fn extend", ($) => { $.fn.extend({ __tst() { return this.length } }); const r = $("<a><b>").__tst(); const has = typeof $.fn.__tst; delete $.fn.__tst; return [r, has, $.fn.extend === $.extend] })

// core: extend
C("extend shallow", ($) => { const t = { a: 1, o: { x: 1 } }; const r = $.extend(t, { b: 2, o: { y: 2 } }, null, undefined, { c: 3 }); return [r === t, r] })
C("extend deep", ($) => { const src = { o: { y: 2, arr: [1, { q: 1 }] }, arr: [3] }; const t = { a: 1, o: { x: 1, arr: { k: 1 } }, arr: [9, 8, 7] }; const r = $.extend(true, t, src); return [r, r.o.arr !== src.o.arr, r.o.arr[1] !== src.o.arr[1]] })
C("extend deep non-plain", ($) => { class K { constructor() { this.v = 1 } } const k = new K(); const r = $.extend(true, {}, { k, d: new Date(0), re: /x/, n: null, f: function () {} }); return [r.k === k, r.d instanceof Date, r.re instanceof RegExp, r.n, typeof r.f] })
C("extend deep replaces", ($) => { const r = $.extend(true, { a: 5, b: [1], c: { z: 1 }, d: "s" }, { a: { x: 1 }, b: { y: 1 }, c: [2], d: [1] }); return r })
C("extend undefined skip", ($) => $.extend({ a: 1, b: 2 }, { a: undefined, b: null, c: undefined }))
C("extend proto", ($) => { const src = JSON.parse('{"__proto__": {"polluted": 1}, "ok": 1}'); const r = $.extend(true, {}, src); return [r.ok, ({}).polluted, Object.keys(r)] })
C("extend self ref", ($) => { const t = {}; const r = $.extend(t, { self: t, x: 1 }); return [Object.keys(r), r.self] })
C("extend deep self ref", ($) => { const t = { a: {} }; const src = { a: t.a }; return tryit(() => Object.keys($.extend(true, t, src))) })
C("extend non-object target", ($) => [$.extend("str", { a: 1 }), $.extend(5, { a: 1 }), $.extend(true, "s", { a: 1 }), $.extend(null, { a: 1 }), $.extend(undefined, { a: 2 }), $.extend(false, { a: 3 }), typeof $.extend(function () {}, { a: 1 })])
C("extend into jQuery", ($) => { const r = $.extend({ __zz: 1 }); const ok = [r === $, $.__zz]; delete $.__zz; const r2 = $.extend(true, { __zy: { a: 1 } }); ok.push(r2 === $, $.__zy); delete $.__zy; return ok })
C("extend arrays", ($) => [$.extend([1, 2, 3], [4, 5]), $.extend(true, [], [[1], { a: 1 }]), $.extend({}, [7, 8]), $.extend(true, {}, { a: [1, [2]] })])
C("extend inherited", ($) => { const proto = { inh: 1 }; const src = Object.create(proto); src.own = 2; return $.extend({}, src) })
C("extend deep array into plain", ($) => { const t = { a: { x: 1 } }; $.extend(true, t, { a: [1] }); return t })
C("extend deep bool false", ($) => $.extend(false, { a: { b: 1 } }, { a: { c: 2 } }))
C("extend deep plain proto null", ($) => { const n = Object.create(null); n.q = { z: 1 }; const r = $.extend(true, {}, { n }); return [r.n !== n, Object.getPrototypeOf(r.n) === Object.prototype, r.n.q !== n.q] })
C("extend no args", ($) => { return [$.extend() === $, $.extend(true) === $] })

// core: statics
C("statics shape", ($) => [/^jQuery\d+$/.test($.expando), $.isReady, typeof $.guid, $.noop(), typeof $.support, typeof $.error])
C("error", ($) => [tryit(() => $.error("boom")), (() => { try { $.error("m") } catch (e) { return [e instanceof W.Error || e instanceof Error, e.message, e.constructor.name] } })()])
C("isPlainObject", ($) => { function F() {} return [{}, Object.create(null), new F(), [], null, undefined, "s", 1, true, W, D.body, Object.create({}), new Object(), Math, function () {}, new Date(), JSON, { constructor: 1 }, Object.create(Object.prototype), { a: 1 }, Object.assign(Object.create(null), { a: 1 })].map((v) => $.isPlainObject(v)) })
C("isEmptyObject", ($) => [{}, { a: 1 }, [], [1], Object.create({ a: 1 }), null, undefined, "", "ab", 5, Object.create(null)].map((v) => $.isEmptyObject(v)))
C("globalEval", ($) => { W.__ge = 0; $.globalEval("window.__ge = (window.__ge||0) + 1"); $.globalEval("window.__ge2 = 7", { nonce: "abc" }); $.globalEval("window.__ge3 = 8", undefined, D); const r = [W.__ge, W.__ge2, W.__ge3, D.head.childNodes.length]; delete W.__ge; delete W.__ge2; delete W.__ge3; return r })
C("globalEval returns", ($) => [$.globalEval("1+1"), typeof $.globalEval])
C("each array", ($) => { const out = []; const a = [1, 2, 3]; const r = $.each(a, function (i, v) { out.push(i, v, this === v || (typeof this === "object" && this.valueOf() === v)); if (v === 2) return false }); return [out, r === a] })
C("each object", ($) => { const out = []; const o = { a: 1, b: "x", c: null }; const r = $.each(o, function (k, v) { out.push(k, v) }); return [out, r === o] })
C("each arraylike", ($) => { const out = []; $.each({ length: 2, 0: "a", 1: "b", x: 9 }, (i, v) => { out.push(i, v) }); $.each({ length: 0, x: 9 }, (i, v) => { out.push(i, v) }); $.each({ length: -1, x: 9 }, (i, v) => { out.push(i, v) }); $.each({ length: "2", x: 9 }, (i, v) => { out.push(i, v) }); return out })
C("each function/window/string", ($) => { const out = []; const f = function () {}; f.a = 1; $.each(f, (k, v) => { out.push(k, v) }); $.each("ab", (k, v) => { out.push(k, v) }); return out })
C("each null", ($) => [tryit(() => $.each(null, () => {})), tryit(() => $.each(undefined, () => {}))])
C("each break on false only", ($) => { const out = []; $.each([1, 2, 3], (i, v) => { out.push(v); return v === 1 ? "" : v === 2 ? 0 : false }); return out })
C("each arguments", ($) => { const out = []; (function () { $.each(arguments, (i, v) => { out.push(i, v) }) })(4, 5); return out })
C("text", ($) => { reset(); return [$.text(D.getElementById("p1")), $.text([D.getElementById("p1"), D.getElementById("s1")]), $.text(D).length > 0, $.text(D.getElementById("s1").firstChild), $.text(D.getElementById("s1").lastChild), $.text(D.createDocumentFragment()), $.text([]), $.text({ 0: D.getElementById("p1"), length: 1 })] })
C("text cdata/comment", ($) => { const x = new W.DOMParser().parseFromString("<r><![CDATA[cd]]><!--cm--></r>", "text/xml"); return [$.text(x.documentElement.firstChild), $.text(x.documentElement.lastChild), $.text(x)] })
C("makeArray", ($) => { reset(); return [$.makeArray(null), $.makeArray(undefined), $.makeArray("str"), $.makeArray([1, 2]), $.makeArray({ length: 1, 0: "a" }), $.makeArray(5), $.makeArray(true), $.makeArray(W).length, $.makeArray(D.querySelectorAll("li")).length, $.makeArray($("li")).length, Array.isArray($.makeArray($("li"))), $.makeArray({ a: 1 }), $.makeArray(function () {}).length, $.makeArray(new String("ab")), $.makeArray("") ] })
C("makeArray results", ($) => { const obj = { length: 1, 0: "z" }; const r = $.makeArray([1, 2], obj); const r2 = $.makeArray("s", [0]); const r3 = $.makeArray(null, [9]); return [r === obj, r, r2, r3] })
C("makeArray arguments", ($) => (function () { return $.makeArray(arguments) })(1, "b"))
C("inArray", ($) => [$.inArray(2, [1, 2, 3]), $.inArray("2", [1, 2, 3]), $.inArray(4, [1, 2, 3]), $.inArray(1, null), $.inArray(1, undefined), $.inArray(1, [1, 2, 1], 1), $.inArray(1, [1, 2, 1], -1), $.inArray(1, [1, 2, 1], -2), $.inArray(1, [1, 2, 1], "1"), $.inArray(NaN, [NaN]), $.inArray("b", { length: 2, 0: "a", 1: "b" }), $.inArray(1, [1], NaN), $.inArray(1, [1], null)])
C("inArray string", ($) => tryit(() => $.inArray("b", "abc")))
C("isXMLDoc", ($) => { const x = new W.DOMParser().parseFromString("<r><c/></r>", "text/xml"); return [$.isXMLDoc(D), $.isXMLDoc(D.body), $.isXMLDoc(x), $.isXMLDoc(x.documentElement), $.isXMLDoc(x.documentElement.firstChild), $.isXMLDoc(null), $.isXMLDoc(undefined), $.isXMLDoc({}), $.isXMLDoc(D.createElementNS("http://www.w3.org/2000/svg", "svg"))] })
C("merge", ($) => { reset(); const a = [1, 2]; const r = $.merge(a, [3, 4]); const o = { length: 1, 0: "x" }; $.merge(o, ["y"]); const e = $.merge([], D.querySelectorAll("li")); const j = $.merge($("p"), $("li")); return [r === a, r, o, e.length, j.length, $.merge([1], { length: 2, 0: "a", 1: "b" }), $.merge([], []), $.merge([1], { length: "2", 0: 3, 1: 4 }), $.merge([1], "ab")] })
C("merge holes", ($) => { const r = $.merge([1], [, 2]); return [r.length, 1 in r, r] })
C("merge obj no length", ($) => { const o = {}; const r = tryit(() => $.merge(o, [1])); return [r === o ? "same" : r, o] })
C("grep", ($) => { const a = [1, 2, 3, 4, 5]; const out = []; const r1 = $.grep(a, (v, i) => { out.push(i); return v % 2 }); return [r1, $.grep(a, (v) => v > 2, true), $.grep(a, (v) => v > 2, false), $.grep(a, (v) => v > 2, "x"), $.grep(a, (v) => v > 2, 0), $.grep(a, (v) => v > 2, undefined), $.grep([], () => true), out, $.grep({ length: 2, 0: "a", 1: "b" }, (v) => v === "b")] })
C("map", ($) => { const out = []; return [$.map([1, 2, 3], (v, i) => v * 2), $.map([1, 2, 3], (v) => (v === 2 ? null : v === 3 ? undefined : v)), $.map([1, 2], (v) => [v, [v]]), $.map({ a: 1, b: 2 }, (v, k) => k + v), $.map({ length: 2, 0: "x", 1: "y" }, (v, i) => v + i), $.map([1], (v, i, arg) => typeof arg), $.map([1], (v, i, arg) => arg, "ARG"), $.map([], () => 1), $.map({ x: 1 }, function () { return this === W || this === undefined }), $.map([1, 2], (v, i, a) => { out.push(a); return v }, 0), out] })
C("map arraylike object", ($) => [$.map({ length: 0, x: 1 }, (v) => v), $.map({ length: 1, 0: "a", x: 1 }, (v) => v)])
C("type/toType via $.type", ($) => { if (!$.type) return "no $.type"; return [undefined, null, 1, "s", true, [], {}, function () {}, new Date(), /x/, new Error("e"), Symbol("s"), new Number(1), new String("s"), new Boolean(false), Object.create(null), W, 10n].map((v) => $.type(v)) })
C("isArrayLike via each(window)", ($) => { const out = []; const fake = { length: 3 }; $.each(fake, (k) => { out.push(k) }); return out })
C("makeArray fake arraylike", ($) => [$.makeArray({ length: 3, 2: "c" }), $.makeArray({ length: 2.5, 1: "b" }).length])
C("statics keys superset", ($) => Object.keys(official).filter((k) => !(k in $)).sort())
C("fn keys superset", ($) => Object.keys(official.fn).filter((k) => !(k in $.fn)).sort())

// core/parseHTML
C("parseHTML basic", ($) => [$.parseHTML("<div>a</div><b>x</b>"), $.parseHTML("hello"), $.parseHTML("<br>"), $.parseHTML("<br/>"), $.parseHTML(""), $.parseHTML(null), $.parseHTML(undefined), $.parseHTML(5), $.parseHTML({}), $.parseHTML(" <i>a</i> ")])
C("parseHTML scripts", ($) => [$.parseHTML("<div>a</div><script>window.__ph=1</script>"), $.parseHTML("<div>a</div><script>window.__ph=1</script>", true), $.parseHTML("<div>a</div><script>window.__ph=1</script>", D, true), $.parseHTML("<div>a</div><script>window.__ph=1</script>", D, false), $.parseHTML("<div><script>x</script></div>"), $.parseHTML("<script>x</script>"), W.__ph])
C("parseHTML context", ($) => { const r = $.parseHTML("<p>x</p>", D); const r2 = $.parseHTML("<p>x</p>"); return [r[0].ownerDocument === D, r2[0].ownerDocument === D, r2[0].ownerDocument.body !== undefined, $.parseHTML("<a href='rel'>x</a>")[0].href, $.parseHTML("<a href='rel'>x</a>", D)[0].href, $.parseHTML("<p>x</p>", D.body)[0].nodeName] })
C("parseHTML tables", ($) => [$.parseHTML("<tr><td>1</td></tr>"), $.parseHTML("<td>1</td>"), $.parseHTML("<option>o</option>"), $.parseHTML("<thead></thead>"), $.parseHTML("<col>")])
C("parseHTML img", ($) => [$.parseHTML("<img src='x.png' onerror='window.__bad=1'>").length, W.__bad])

// core/parseXML
C("parseXML", ($) => { const x = $.parseXML("<root><a>1</a></root>"); return [x.documentElement.nodeName, x.getElementsByTagName("a")[0].textContent, $.isXMLDoc(x), $.parseXML(""), $.parseXML(null), $.parseXML(undefined), $.parseXML(5), tryit(() => $.parseXML("<root>")), (() => { try { $.parseXML("<a></b>") } catch (e) { return e.message.slice(0, 13) } })()] })

// core/ready
C("ready", async ($) => { const out = []; const r = $(D).ready(function (arg) { out.push("ready", arg === $, this === D) }); out.push(r.length, $.isReady, $.readyWait, typeof $.ready.then, typeof $.ready); await sleep(5); $.ready.then(function (arg) { out.push("then", arg === $) }); await sleep(5); return out })
C("ready ret", ($) => { const j = $("<div>"); return j.ready(() => {}) === j })
C("ready exception", async ($) => { const out = []; const saved = $.readyException; $.readyException = (e) => out.push("rex", e.message); $(() => { throw new Error("rx") }); await sleep(20); $.readyException = saved; return out })
C("readyException default", async ($) => { const out = []; const onerr = (ev) => { out.push("uncaught", ev.error && ev.error.message); ev.preventDefault() }; W.addEventListener("error", onerr); const savedST = W.setTimeout; let fnSeen = null; W.setTimeout = function (fn, ms) { fnSeen = fn; return 0 }; try { $.readyException(new Error("zz")) } finally { W.setTimeout = savedST } W.removeEventListener("error", onerr); return [typeof fnSeen, tryit(() => fnSeen())] })
C("ready manual", ($) => { const s = $.readyWait; const isr = $.isReady; $.ready(); $.ready(true); const r = [$.isReady, $.readyWait]; $.readyWait = s; return [r, isr] })

// core/access through public users
C("access attr/prop", ($) => { reset(); const p = $("p"); const out = []; out.push(p.attr("id"), p.attr({ "data-a": "1", title: "t" }).length, p.attr("title"), p.attr("title", (i, v) => v + i).attr("title"), $().attr("id"), p.prop("id"), $("#p1").attr("title", null).attr("title")); return out })
C("access css/text/html", ($) => { reset(); const p = $("p"); return [p.text(), p.text("x").length, p.text(), p.html(), $().text(), $().html(), p.css("display"), p.css({ color: "red" }).css("color"), p.css(["color"]), p.text((i, t) => t + i).text()] })
C("access data bulk", ($) => { reset(); return [$("p").data(), $().data(), $("p").data("foo")] })

// camelCase
C("camelCase", ($) => { if (!$.camelCase) return "none"; return ["a-b", "-ms-transform", "-moz-x", "a-b-c", "a--b", "a-1", "A-B", "foo-bar-", "ms-x"].map((s) => $.camelCase(s)) })

// data
C("data static", ($) => { reset(); const e = D.getElementById("p1"); const out = []; out.push($.hasData(e), $.data(e, "k", 1), $.data(e, "k"), $.hasData(e), $.data(e, { a: 2, "b-c": 3 }), $.data(e), $.data(e, "bC"), $.data(e, "b-c"), $.data(e, "missing"), $.data(e, "k", undefined), $.data(e, "k"), $.data(e, 5), $.data(e, null), $.data(e, "")); $.removeData(e, "k"); out.push($.data(e)); $.removeData(e, "a bC"); out.push($.data(e), $.hasData(e)); $.removeData(e); out.push($.hasData(e), $.data(e)); return out })
C("data removeData array", ($) => { const o = { }; $.data(o, { aB: 1, "c-d": 2, e: 3, "f g": 4 }); $.removeData(o, ["a-b", "cD"]); const r1 = $.data(o); $.removeData(o, "f g"); const r2 = $.data(o); $.removeData(o, "e"); return [r1, r2, $.data(o), $.hasData(o), Object.keys(o)] })
C("data plain obj", ($) => { const o = { x: 1 }; $.data(o, "k", "v"); const keys = Object.keys(o); const r = [keys, $.data(o, "k"), $.hasData(o), JSON.stringify(o)]; $.removeData(o); r.push($.hasData(o), Object.getOwnPropertyNames(o).length); return r })
C("data text node", ($) => { const t = D.createTextNode("t"); return [$.data(t, "k", 1), $.data(t, "k"), $.hasData(t), $.data(t), $(t).data("k", 2).data("k"), $(t).data()] })
C("data document/window", ($) => { const r = [$.data(D, "k", 1), $.data(D, "k"), $.hasData(D), $(W).data("w", 3).data("w"), $.hasData(W)]; $.removeData(D); $.removeData(W); return r })
C("_data", ($) => { reset(); const e = D.getElementById("p1"); $._data(e, "pk", 1); const r = [$._data(e, "pk"), $._data(e).pk, $.data(e, "pk"), $.hasData(e)]; $._removeData(e, "pk"); r.push($._data(e, "pk"), $.hasData(e)); return r })
C("fn.data attrs", ($) => { reset(); const p = $("#p2"); return [p.data(), p.data("foo"), p.data("num"), p.data("float"), p.data("json"), p.data("arr"), p.data("t"), p.data("f"), p.data("n"), p.data("exp"), p.data("fooBar"), p.data("foo-bar"), p.data("empty"), p.data("nope"), typeof p.data("num")] })
C("fn.data attrs lazy", ($) => { reset(); const p = $("#p2"); const a = p.data("num"); p.attr("data-num", "7"); const b = p.data("num"); const c = $("#p2").data(); return [a, b, c.num] })
C("fn.data set/get", ($) => { reset(); const p = $("p"); const out = []; out.push(p.data("k", 1) === p, p.data("k"), $("#p2").data("k"), p.data({ a: 1, "b-c": 2 }).length, $("#p2").data("bC"), $("#p1").data()); out.push(p.data("k", undefined) === p, p.data("k")); out.push(p.data("k", null).data("k")); return out })
C("fn.data empty set", ($) => [$().data(), $().data("x"), $().data("x", 1).length, $().data({ a: 1 }).length, $().removeData("x").length])
C("fn.data key types", ($) => { reset(); const p = $("#p1"); return [p.data(5, "five").data(5), p.data("5"), p.data(null), tryit(() => p.data(true))] })
C("fn.removeData", ($) => { reset(); const p = $("#p2"); p.data("foo"); p.data("x", 1); const r = p.removeData("x"); const a = [r === p, p.data("x"), p.data("foo")]; p.removeData("foo"); a.push(p.data("foo")); p.removeData(); a.push(p.data("x"), $.hasData(p[0]), p.data("num")); return a })
C("fn.data camel", ($) => { reset(); const p = $("#p1"); p.data("some-key", 1); p.data("someOther", 2); return [p.data("someKey"), p.data("some-other"), p.data(), p.removeData("someKey").data()] })
C("fn.data obj", ($) => { const o = {}; const j = $(o); j.data("a", 1); return [j.data("a"), j.data(), Object.keys(o)] })
C("fn.data getter array key", ($) => { reset(); const p = $("#p1"); p.data("a", 1); return [p.data(["a"]), p.data({})] })

// callbacks
function cbRun($, flags) {
  const out = []
  const cb = $.Callbacks(flags)
  const f1 = function (x) { out.push("f1:" + x + ":" + (this && this.tag)); }
  const f2 = function (x) { out.push("f2:" + x); return x !== "stop" ? undefined : false }
  const f3 = function (x) { out.push("f3:" + x) }
  out.push(cb.add(f1) === cb)
  cb.add(f2, [f3, [f1]], "notfn", null, { length: 1, 0: f3 })
  out.push(cb.has(f1), cb.has(f3), cb.has(), cb.has(function () {}))
  out.push(cb.fire("a") === cb, cb.fired())
  cb.fireWith({ tag: "T" }, ["b"])
  cb.fire("stop")
  cb.add(function (x) { out.push("late:" + x) })
  out.push(cb.remove(f1) === cb, cb.has(f1))
  cb.fire("c")
  out.push(cb.locked(), cb.disabled())
  cb.lock(); out.push(cb.locked(), cb.disabled())
  cb.fire("d"); cb.add(function (x) { out.push("afterlock:" + x) })
  cb.empty(); cb.add(f3); cb.fire("e")
  cb.disable(); out.push(cb.disabled(), cb.locked(), cb.fired()); cb.fire("f"); cb.add(f1); out.push(cb.has(f1), cb.has())
  return out
}
for (const flags of [undefined, "", "once", "memory", "unique", "stopOnFalse", "once memory", "memory unique", "once stopOnFalse", "memory stopOnFalse", "  once   memory  ", "bogus"]) {
  C("callbacks " + JSON.stringify(flags), ($) => cbRun($, flags))
}
C("callbacks object opts", ($) => [cbRun($, { once: true }), cbRun($, { memory: true, unique: true }), cbRun($, { stopOnFalse: 1 }), cbRun($, null)])
C("callbacks reentrant", ($) => { const out = []; const cb = $.Callbacks(); cb.add(function f(x) { out.push("a" + x); if (x < 2) cb.fire(x + 1) }, (x) => out.push("b" + x)); cb.fire(0); return out })
C("callbacks remove during fire", ($) => { const out = []; const cb = $.Callbacks(); const b = () => out.push("b"); const c = () => out.push("c"); cb.add(() => { out.push("a"); cb.remove(b) }, b, c); cb.fire(); cb.fire(); return out })
C("callbacks remove self during fire", ($) => { const out = []; const cb = $.Callbacks(); const a = () => { out.push("a"); cb.remove(a) }; cb.add(a, () => out.push("b"), () => out.push("c")); cb.fire(); cb.fire(); return out })
C("callbacks add during fire", ($) => { const out = []; const cb = $.Callbacks(); cb.add(() => { out.push("a"); cb.add(() => out.push("added")) }); cb.fire(); return out })
C("callbacks memory add during fire", ($) => { const out = []; const cb = $.Callbacks("memory"); cb.add((x) => { out.push("a" + x); cb.add((y) => out.push("m" + y)) }); cb.fire(1); cb.add((z) => out.push("late" + z)); return out })
C("callbacks unique dupes", ($) => { const out = []; const cb = $.Callbacks("unique"); const f = () => out.push("f"); cb.add(f, f, [f]); cb.fire(); const cb2 = $.Callbacks(); cb2.add(f, f); cb2.fire(); cb2.remove(f); cb2.fire(); return out })
C("callbacks fireWith args", ($) => { const out = []; const cb = $.Callbacks(); cb.add(function () { out.push([].slice.call(arguments), this === W || this === undefined ? "g" : this) }); cb.fireWith(); cb.fireWith(null); cb.fireWith({ c: 1 }, "str"); cb.fireWith({ c: 2 }, { length: 2, 0: "x", 1: "y" }); cb.fireWith(undefined, [1, 2]); (function () { cb.fireWith(this, arguments) })(7, 8); cb.fire(1, 2, 3); return out })
C("callbacks lock memory", ($) => { const out = []; const cb = $.Callbacks("memory"); cb.add((x) => out.push("a" + x)); cb.fire(1); cb.lock(); cb.add((x) => out.push("b" + x)); cb.fire(2); out.push(cb.disabled(), cb.locked()); return out })
C("callbacks lock before fire", ($) => { const out = []; const cb = $.Callbacks("memory"); cb.add((x) => out.push("a" + x)); cb.lock(); cb.fire(1); cb.add((x) => out.push("b" + x)); out.push(cb.disabled(), cb.locked(), cb.has()); return out })
C("callbacks empty during fire", ($) => { const out = []; const cb = $.Callbacks(); cb.add(() => { out.push("a"); cb.empty() }, () => out.push("b")); cb.fire(); cb.fire(); return out })
C("callbacks disabled has", ($) => { const cb = $.Callbacks(); cb.disable(); return [cb.has(), cb.has(() => {}), cb.disabled(), cb.fired()] })
C("callbacks once fired has", ($) => { const cb = $.Callbacks("once"); cb.add(() => {}); cb.fire(); return [cb.has(), cb.disabled(), cb.locked(), cb.fired()] })
C("callbacks stopOnFalse memory", ($) => { const out = []; const cb = $.Callbacks("memory stopOnFalse"); cb.add(() => false); cb.fire(); cb.add(() => out.push("late")); return out })
C("callbacks method lengths", ($) => { const cb = $.Callbacks(); return Object.keys(cb) })
C("callbacks returns", ($) => { const cb = $.Callbacks(); return [cb.empty() === cb, cb.lock() === cb, cb.disable() === cb, cb.fireWith() === cb, cb.remove() === cb, cb.add() === cb] })

// deferred
C("deferred sync basics", ($) => { const out = []; const d = $.Deferred(); out.push(d.state()); d.done((a, b) => out.push("done", a, b)).fail(() => out.push("fail")).progress((p) => out.push("prog", p)).always((a) => out.push("always", a)); d.notify(1); d.notifyWith({ c: 1 }, [2]); d.resolve(3, 4); d.reject(5); d.notify(6); out.push(d.state()); d.done((a) => out.push("late", a)); return out })
C("deferred reject", ($) => { const out = []; const d = $.Deferred(); d.fail(function (e) { out.push("fail", e, this === d.promise() ? "p" : this === d ? "d" : this) }).always(() => out.push("always")); d.reject("x"); d.resolve("y"); out.push(d.state()); return out })
C("deferred resolveWith ctx", ($) => { const out = []; const d = $.Deferred(); const ctx = { k: 1 }; d.done(function (a) { out.push(this === ctx, a) }); d.resolveWith(ctx, ["v"]); const d2 = $.Deferred(); d2.done(function () { out.push(this === d2.promise(), this === d2) }); d2.resolve(); const d3 = $.Deferred(); d3.done(function () { out.push(this === d3) }); d3.resolve.call({}); return out })
C("deferred fn init", ($) => { const out = []; const d = $.Deferred(function (x) { out.push(this === x, typeof x.resolve); x.resolve(1) }); out.push(d.state()); return out })
C("deferred promise", ($) => { const d = $.Deferred(); const p = d.promise(); const o = { a: 1 }; const r = d.promise(o); return [p === d.promise(), typeof p.resolve, typeof p.then, r === o, typeof o.done, o.state(), Object.keys(p).sort(), Object.keys(d).sort(), d.promise(null) === p] })
C("deferred then async", async ($) => { const out = []; const d = $.Deferred(); const p = d.then((v) => { out.push("t1", v); return v * 2 }).then((v) => { out.push("t2", v); throw new Error("E") }).then(null, (e) => { out.push("c", e.message); return "rec" }).catch((e) => out.push("nope")).then((v) => out.push("t3", v)); d.resolve(5); out.push("sync", p.state()); await sleep(30); out.push(p.state()); return out })
C("deferred then values", async ($) => { const out = []; const d = $.Deferred(); d.then(null, null, (p) => { out.push("p", p); return p + 1 }).progress((x) => out.push("pp", x)); d.notify(1); d.then((v) => { const n = $.Deferred(); setTimeout(() => n.resolve("inner" + v), 1); return n }).done((x) => out.push("adopt", x)); d.then(() => ({ then(res) { res("thenable") } })).done((x) => out.push("thenable", x)); d.then(() => Promise.resolve("native")).done((x) => out.push("native", x)); d.resolve("R"); await sleep(40); return out })
C("deferred then self", async ($) => { const out = []; const d = $.Deferred(); let p; const saved = $.Deferred.exceptionHook; $.Deferred.exceptionHook = (e) => out.push("hook", e.name, e.message); p = d.then(() => p); p.fail((e) => out.push("fail", e.name, e.message)); d.resolve(); await sleep(30); $.Deferred.exceptionHook = saved; return out })
C("deferred then reject passthrough", async ($) => { const out = []; const d = $.Deferred(); d.then((v) => out.push("no")).then(null, (e) => { out.push("r", e); return $.Deferred().reject("again") }).fail((e) => out.push("f", e)); d.reject("x"); await sleep(30); return out })
C("deferred exceptionHook", async ($) => { const out = []; const saved = $.Deferred.exceptionHook; $.Deferred.exceptionHook = (e, s) => out.push("hook", e.message, s === undefined ? "u" : typeof s); $.Deferred().resolve(1).then(() => { throw new TypeError("te") }).fail((e) => out.push("fail", e.message)); await sleep(30); $.Deferred.exceptionHook = saved; return out })
C("deferred exceptionHook default", async ($) => { const out = []; const saved = W.console.warn; W.console.warn = (...a) => out.push(a.map((x) => (typeof x === "string" ? x : typeof x))); $.Deferred().resolve(1).then(() => { throw new TypeError("te") }); $.Deferred().resolve(1).then(() => { throw "notanerror" }); $.Deferred().resolve(1).then(() => { const e = new Error("plain"); throw e }); await sleep(30); W.console.warn = saved; return [out, typeof $.Deferred.exceptionHook] })
C("deferred errorhooks", async ($) => { const out = []; $.Deferred.getErrorHook = () => "EH"; const saved = $.Deferred.exceptionHook; $.Deferred.exceptionHook = (e, s) => out.push("hook", s); $.Deferred().resolve().then(() => { throw new Error("q") }); await sleep(20); delete $.Deferred.getErrorHook; $.Deferred.getStackHook = () => "SH"; $.Deferred().resolve().then(() => { throw new Error("q") }); await sleep(20); delete $.Deferred.getStackHook; $.Deferred.exceptionHook = saved; return out })
C("deferred pipe", ($) => { const out = []; const d = $.Deferred(); const p = d.pipe((v) => v + 1, (e) => "err" + e, (n) => n * 10); p.done((v) => out.push("done", v)).fail((e) => out.push("fail", e)).progress((n) => out.push("prog", n)); d.notify(2); d.resolve(1); const d2 = $.Deferred(); d2.pipe(null, (e) => $.Deferred().resolve("fixed" + e)).done((v) => out.push("d2", v)); d2.reject("x"); const d3 = $.Deferred(); d3.pipe().fail((a, b) => out.push("d3", a, b)); d3.reject(1, 2); return out })
C("deferred pipe ctx", ($) => { const out = []; const d = $.Deferred(); const ctx = { c: 1 }; d.pipe((v) => v).done(function (v) { out.push(this === ctx, v) }); d.resolveWith(ctx, [5]); return out })
C("deferred always/catch return", ($) => { const d = $.Deferred(); const p = d.promise(); return [d.always(() => {}) === d, p.always(() => {}) === p, typeof d.catch(() => {}).then, d.done() === d, p.done() === p] })
C("deferred state after", async ($) => { const d = $.Deferred(); const r = d.resolve(); const d2 = $.Deferred(); d2.reject(); return [r === d, d.state(), d2.state(), d.resolve() === d, d.notify() === d] })
C("deferred progress after resolve", ($) => { const out = []; const d = $.Deferred(); d.notify("a"); d.progress((x) => out.push("p", x)); d.resolve(); d.notify("b"); d.progress((x) => out.push("late", x)); return out })
C("deferred then ordering", async ($) => { const out = []; const d = $.Deferred(); d.then(() => out.push(1)); d.done(() => out.push(2)); d.resolve(); out.push(3); d.then(() => out.push(4)); await sleep(20); return out })
C("deferred then this", async ($) => { const out = []; const d = $.Deferred(); const ctx = { z: 1 }; d.then(function () { out.push(this === ctx); return 1 }).done(function (v) { out.push(this === ctx, this === undefined || this === W ? "g" : typeof this, v) }); d.resolveWith(ctx, [0]); d.then(function () { return this }); await sleep(20); return out })
C("deferred then identity ctx", async ($) => { const out = []; const d = $.Deferred(); const ctx = { z: 1 }; d.then().done(function (a, b) { out.push(this === ctx, a, b) }); d.resolveWith(ctx, [1, 2]); const e = $.Deferred(); e.then().fail(function (a, b) { out.push("f", this === ctx, a, b) }); e.rejectWith(ctx, [3, 4]); await sleep(20); return out })
C("deferred notify then", async ($) => { const out = []; const d = $.Deferred(); d.then(null, null, (v) => $.Deferred().resolve("pv" + v)).progress((x) => out.push("p", x)).done((x) => out.push("d", x)); d.notify(1); d.notify(2); await sleep(20); d.resolve("end"); await sleep(20); return out })
C("when none", async ($) => { const out = []; const p = $.when(); p.done(function () { out.push("d", arguments.length, this === W ? "w" : typeof this) }); out.push(p.state()); await sleep(5); return out })
C("when value", async ($) => { const out = []; const p = $.when(5); out.push(p.state()); p.done((v) => out.push("d", v)); await sleep(10); out.push(p.state()); const p2 = $.when("s", "t"); p2.done((a, b) => out.push("d2", a, b)); await sleep(5); return out })
C("when deferreds", async ($) => { const out = []; const a = $.Deferred(); const b = $.Deferred(); const w = $.when(a, b, 7); w.done(function (x, y, z) { out.push("d", x, y, z, Array.isArray(this) ? this.length : typeof this) }).progress((...v) => out.push("prog", v)); a.notify("n"); b.resolve(1, 2); out.push(w.state()); a.resolve("A"); out.push(w.state()); await sleep(5); return out })
C("when reject", async ($) => { const out = []; const a = $.Deferred(); const b = $.Deferred(); $.when(a, b).fail((e) => out.push("f", e)).done(() => out.push("no")); b.reject("B"); a.resolve(); await sleep(5); return out })
C("when single deferred", async ($) => { const out = []; const a = $.Deferred(); const p = $.when(a); out.push(p === a.promise(), p.state()); p.done((x, y) => out.push("d", x, y)); a.resolve(1, 2); await sleep(10); out.push(p.state()); return out })
C("when single resolved", async ($) => { const out = []; const a = $.Deferred().resolve(9); const p = $.when(a); out.push(p.state()); p.done((x) => out.push("d", x)); await sleep(10); return out })
C("when thenable", async ($) => { const out = []; $.when(Promise.resolve(3)).done((x) => out.push("n", x)); $.when({ then(r) { r(4) } }).done((x) => out.push("t", x)); $.when(Promise.reject(5)).fail((x) => out.push("r", x)); $.when(Promise.resolve(1), $.Deferred().resolve(2)).done((a, b) => out.push("m", a, b)); await sleep(20); return out })
C("when throws", async ($) => { const out = []; const bad = { get then() { throw new Error("getter") } }; const r = tryit(() => { $.when(bad).fail((e) => out.push("f", e.message)); return "ok" }); await sleep(10); return [r, out] })
C("when promise obj", async ($) => { const out = []; const d = $.Deferred(); const p = $.when(d.promise(), null, undefined); p.done((...a) => out.push(a)); d.resolve("x"); await sleep(10); return out })
C("when array", async ($) => { const out = []; $.when([1, 2]).done((a) => out.push(a)); await sleep(10); return out })

// extra coverage for rewritten paths
C("callbacks options copied", ($) => { const out = []; const opts = { memory: false }; const cb = $.Callbacks(opts); opts.memory = true; cb.fire(1); cb.add((x) => out.push("late", x)); const o2 = { once: true }; const cb2 = $.Callbacks(o2); delete o2.once; cb2.add((x) => out.push("o", x)); cb2.fire(1); cb2.fire(2); return out })
C("holdReady/readyWait", ($) => { const r0 = $.readyWait; const out = [$.isReady]; if ($.holdReady) { $.holdReady(true); out.push($.readyWait); $.holdReady(false); out.push($.readyWait, $.isReady) } $.readyWait = 1; $.ready(true); out.push($.readyWait, $.isReady); $.ready(true); out.push($.readyWait); $.ready(); out.push($.readyWait); $.readyWait = r0; return out })
C("ctor context variants", ($) => { reset(); return [$("p", {}).length, $("#p1", $(D)).length, $("<p>", $("<div>"))[0].nodeName, $("<p>", { text: "t", "data-x": "1" })[0].outerHTML, $("<b>", { css: { color: "red" } })[0].style.color, $("li", "ul").length, $("li", null).length, $("li", undefined).length, $("li", 0).length] })
C("ctor html attrs method", ($) => { const out = []; const j = $("<a>", { click: function () { out.push("clicked") }, html: "<i>x</i>" }); j.trigger("click"); return [out, j[0].outerHTML] })
C("parseHTML bool ctx", ($) => [$.parseHTML("<p>a</p><script>1</script>", true).length, $.parseHTML("<p>a</p><script>1</script>", false).length, $.parseHTML("<p>a</p><script>1</script>", false, true).length, $.parseHTML("<p>a</p><script>1</script>", null, true).length, $.parseHTML("<p>a</p><script>1</script>", undefined, false).length])
C("globalEval doc", ($) => { const doc = D.implementation.createHTMLDocument(""); let threw = tryit(() => $.globalEval("1", null, doc)); return [threw, doc.head.childNodes.length] })
C("deferred then undefined handlers", async ($) => { const out = []; const d = $.Deferred(); d.then(undefined, undefined, undefined).done((v) => out.push("d", v)); d.then("notfn").done((v) => out.push("s", v)); d.resolve(4); await sleep(20); return out })
C("deferred catch return", async ($) => { const out = []; const d = $.Deferred(); d.catch((e) => { out.push("c", e); return "r" }).done((v) => out.push("d", v)); d.reject("E"); await sleep(20); return out })
C("deferred always args", ($) => { const out = []; const d = $.Deferred(); d.always([(a) => out.push("x", a), [(a) => out.push("y", a)]], (a) => out.push("z", a)); d.reject(1); return out })
C("deferred pipe returns", ($) => { const d = $.Deferred(); const p = d.pipe(); return [typeof p.then, typeof p.resolve, p.state()] })
C("deferred notifyWith ctx", ($) => { const out = []; const d = $.Deferred(); const ctx = { q: 1 }; d.progress(function (v) { out.push(this === ctx, v) }); d.notifyWith(ctx, [3]); d.notify.call(d, 4); return out })
C("when mixed args", async ($) => { const out = []; const d = $.Deferred(); $.when(1, d, "x").done(function () { out.push([].slice.call(arguments)) }); d.resolve("a", "b"); await sleep(10); return out })
C("isPlainObject cross", ($) => { const other = new W.Object(); return [$.isPlainObject(other), $.isPlainObject(Object.create(Object.create(null))), $.isPlainObject({ constructor: Object }), $.isPlainObject(new (class {})())] })
C("each returns and this", ($) => { const out = []; $.each([1], function () { out.push(typeof this) }); $.each({ a: "s" }, function () { out.push(typeof this) }); return out })
C("merge returns", ($) => { const a = { length: 0 }; const r = $.merge(a, { length: 1, 0: "x" }); return [r === a, a] })
C("grep array-like", ($) => $.grep({ length: 3, 0: 1, 1: 2, 2: 3 }, (v, i) => i !== 1))
C("text nested", ($) => { reset(); return [$.text($("#main").get()), $.text(D.body)] })
C("data events private", ($) => { reset(); const e = D.getElementById("p1"); $(e).on("click", () => {}); const r = [$.hasData(e), typeof $._data(e, "events"), Object.keys($._data(e, "events") || {})]; $(e).off("click"); r.push($.hasData(e)); return r })
C("data removeData whitespace", ($) => { const o = {}; $.data(o, { "a b": 1, a: 2, b: 3 }); $.removeData(o, "a b"); return $.data(o) })
C("data key camel/dash", ($) => { const o = {}; $.data(o, "foo-bar", 1); $.data(o, "-ms-x", 2); $.data(o, "a-b-c", 3); return [$.data(o), $.data(o, "fooBar"), $.data(o, "foo-bar"), $.data(o, "msX")] })

const results = []
for (const [name, fn] of cases) {
  const row = { name }
  for (const [label, $] of [["official", official], ["lil", lil]]) {
    reset()
    let v
    try { v = await fn($) } catch (e) { v = "THREW:" + (e && e.name) + (e && e.name === "TypeError" ? "" : ":" + (e && e.message)) }
    row[label] = ser($, v)
  }
  row.match = row.official === row.lil
  results.push(row)
}
writeFileSync(outPath, JSON.stringify(results, null, 1))
const bad = results.filter((r) => !r.match)
console.log(`cases ${results.length}, mismatches ${bad.length}`)
for (const r of bad) console.log("MISMATCH " + r.name + "\n  official: " + r.official.slice(0, 400) + "\n  lil:      " + r.lil.slice(0, 400))
process.exit(0)
