// Differential harness for the dom group (manipulation, traversing, attributes, wrap, deprecated).
// usage: node diff.mjs <dist/jquery.esm.js> [out.json]
// Runs every case against official jquery@3.7.1 and the given LilScript build, each in its own
// JSDOM (same fixture), serializes return values + DOM + logs, and reports mismatching cases.
import { createRequire } from "node:module"
import { writeFileSync } from "node:fs"
import { pathToFileURL } from "node:url"
import { resolve } from "node:path"

const require = createRequire("/home/azureuser/jquerylil/package.json")
const { JSDOM } = require("jsdom")
const DIST = resolve(process.argv[2])
const OUT = process.argv[3]

const FIXTURE = `<div id="main" class="a b" data-k="v">
<p id="p1" class="x">Hello <b id="b1">bold</b> <i id="i1">it</i></p><p id="p2" class="x y">World</p><!-- cmt -->tn<ul id="list"><li id="li1" class="odd">1</li><li id="li2">2</li><li id="li3" class="odd">3</li><li id="li4">4</li></ul>
<form id="f"><input id="t1" type="text" name="t1" value="tv"><input id="c1" type="checkbox" name="c" value="c1" checked><input id="c2" type="checkbox" name="c" value="c2"><input id="c3" type="checkbox" name="c"><input id="r1" type="radio" name="r" value="r1"><input id="r2" type="radio" name="r" value="r2" checked><select id="s1"><option id="o1" value="o1">One</option><option id="o2" value="o2" selected>Two</option><option id="o3">Three </option></select><select id="s2" multiple><optgroup label="g" disabled><option value="a" selected>A</option></optgroup><option value="b" selected>B</option><option value="c" disabled selected>C</option><option value="d">D</option></select><select id="s3"></select><textarea id="ta">text
area</textarea><button id="btn" value="bv">Btn</button><input id="t2" type="text"></form>
<table id="tbl"><tbody><tr id="tr1"><td id="td1">c</td></tr></tbody></table><table id="tbl2"></table>
<a id="a1" href="#x" tabindex="3">link</a><a id="a2">nolink</a><a id="a3" href="#y">l3</a><span id="sp" contenteditable="true" tabindex="-1">sp</span>
<template id="tpl"><span class="in-tpl">t</span></template><svg id="svg"><circle id="circ" class="c1 c2"></circle></svg>
</div><div id="other"><span class="s">s1</span><span class="s" id="s2b">s2</span><div id="empty"></div></div>`

function makeDom() {
  const dom = new JSDOM("<!doctype html><html><head></head><body></body></html>", {
    pretendToBeVisual: true, runScripts: "dangerously", url: "http://localhost/"
  })
  dom.window.__log = []
  return dom
}

// ---- load libraries ----
delete globalThis.window; delete globalThis.document
const domO = makeDom()
const factory = require("jquery")
const $O = factory.fn?.jquery ? factory : factory(domO.window)
const domL = makeDom()
globalThis.window = domL.window
globalThis.document = domL.window.document
const $L = (await import(pathToFileURL(DIST).href + "?t=" + Date.now())).jQuery

// ---- serialization ----
function nodePath(n) {
  const parts = []
  let cur = n
  while (cur && cur.parentNode) {
    parts.unshift(Array.prototype.indexOf.call(cur.parentNode.childNodes, cur))
    cur = cur.parentNode
  }
  const root = cur ? (cur.nodeType === 9 ? "doc" : cur.nodeType === 11 ? "frag:" + fragHtml(cur) : "detached:" + (cur.outerHTML ?? cur.nodeValue)) : "?"
  return root + "/" + parts.join(".")
}
function fragHtml(f) { return Array.from(f.childNodes).map((c) => c.outerHTML ?? c.nodeValue).join("") }
function ser(x, env, depth = 0) {
  const { $, win } = env
  if (x === undefined) return { $u: 1 }
  if (x === null) return null
  if (typeof x === "number") return Number.isNaN(x) ? { $nan: 1 } : Object.is(x, -0) ? { $m0: 1 } : x
  if (typeof x === "string" || typeof x === "boolean") return x
  if (typeof x === "function") return { $fn: x === $ ? "jQuery" : (x.name && !/^[a-zA-Z_$]{1,3}$/.test(x.name) ? "named" : "anon"), len: x.length, guid: x.guid }
  if (typeof x === "symbol") return { $sym: String(x) }
  if (x === win) return { $win: 1 }
  if (x === win.document) return { $doc: 1 }
  if (x instanceof win.Node) return { $node: x.nodeName, path: nodePath(x), html: x.outerHTML ?? x.nodeValue }
  if (depth > 4) return { $deep: 1 }
  if (x instanceof $) {
    const out = { $jq: Array.from({ length: x.length }, (_, i) => ser(x[i], env, depth + 1)), length: x.length }
    if (x.prevObject && depth < 2) out.prev = ser(x.prevObject, env, depth + 1)
    return out
  }
  if (Array.isArray(x)) return x.map((v) => ser(v, env, depth + 1))
  if (x instanceof win.NodeList || x instanceof win.HTMLCollection) return { $list: Array.from(x).map((v) => ser(v, env, depth + 1)) }
  if (typeof x === "object") {
    const o = {}
    for (const k of Object.keys(x).sort()) o[k] = ser(x[k], env, depth + 1)
    return o
  }
  return String(x)
}

// ---- cases ----
const cases = []
function c(name, fn) { cases.push({ name, fn }) }
function js(a) { return a === undefined ? "undef" : JSON.stringify(a, (k, v) => (v === undefined ? "undef" : v)) }

// callback recorder: logs this-identity + args, returns ret(i, ...)
function rec(env, tag, ret) {
  return function (...args) {
    env.log.push([tag, ser(this, env), args.map((a) => ser(a, env)), arguments.length])
    return typeof ret === "function" ? ret.apply(this, args) : ret
  }
}

const sels = [undefined, null, "", "li", ".odd", "#li3", "p, li", "*", ":first", "li:eq(1)", "b"]
const sets = ["#li2", "li", "#b1", "#p1 b, #p1 i", "#nonexist", "#main", "#li1, #li4", "text"]
function pick($, env, s) {
  if (s === "text") return $("#main").contents().filter(function () { return this.nodeType === 3 })
  return $(s)
}

// traversal family
for (const m of ["parent", "parents", "next", "prev", "nextAll", "prevAll", "siblings", "children", "contents"]) {
  for (const s of sets) {
    for (const sel of [undefined, "li", ".odd", "p", "#li4", "", ":first"]) {
      c(`${m}(${s}|${sel})`, ($, env) => pick($, env, s)[m](sel))
    }
    c(`${m}(${s}|el)`, ($, env) => pick($, env, s)[m](env.doc.getElementById("li3")))
    c(`${m}(${s}|noarg)`, ($, env) => pick($, env, s)[m]())
  }
}
for (const m of ["parentsUntil", "nextUntil", "prevUntil"]) {
  for (const s of sets) {
    const args = [[], ["#main"], ["#li4"], ["#li1"], ["body"], [undefined, "li"], [null, ".odd"], ["#li4", ".odd"], ["ul", "div"], ["#nonexist"]]
    for (const a of args) c(`${m}(${s}|${js(a)})`, ($, env) => pick($, env, s)[m](...a))
    c(`${m}(${s}|el)`, ($, env) => pick($, env, s)[m](env.doc.getElementById("li4")))
    c(`${m}(${s}|jq)`, ($, env) => pick($, env, s)[m]($("#li4, #main")))
  }
}
c("contents-template", ($) => $("#tpl").contents())
c("contents-mixed", ($) => $("#main, #tpl, #p1").contents())
c("children-doc", ($, env) => $(env.doc).children())
c("parent-doc", ($, env) => $(env.doc).parent())
c("parents-frag", ($) => $("<div><p><b>x</b></p></div>").find("b").parents())
c("parent-frag", ($, env) => { const f = env.doc.createDocumentFragment(); const d = env.doc.createElement("div"); f.appendChild(d); return $(d).parent() })
c("siblings-detached", ($) => $("<i>").siblings())

// closest
for (const s of sets) {
  for (const a of [["div"], ["li"], ["#main"], ["p, ul"], ["ul li"], [""], [undefined], ["li", "#list"], [":first"], ["li:eq(1)"], ["div", "body"], ["*"]]) {
    c(`closest(${s}|${js(a)})`, ($, env) => pick($, env, s).closest(...a))
  }
  c(`closest(${s}|el)`, ($, env) => pick($, env, s).closest(env.doc.getElementById("main")))
  c(`closest(${s}|jq)`, ($, env) => pick($, env, s).closest($("ul, #p1")))
  c(`closest(${s}|ctxel)`, ($, env) => pick($, env, s).closest("div", env.doc.getElementById("list")))
}

// index
for (const s of sets) {
  c(`index(${s})`, ($, env) => pick($, env, s).index())
  c(`index(${s}|li)`, ($, env) => pick($, env, s).index("li"))
  c(`index(${s}|el)`, ($, env) => pick($, env, s).index(env.doc.getElementById("li3")))
  c(`index(${s}|jq)`, ($, env) => pick($, env, s).index($("#li3, #li1")))
  c(`index(${s}|null)`, ($, env) => pick($, env, s).index(null))
}
c("index-detached", ($) => $("<i>").index())
c("index-emptyjq", ($) => $("li").index($()))

// add / addBack / has
for (const a of [["li"], ["#p1", "#main"], ["<p>n</p>"], [undefined], [null], [""], ["b", "#p1"]]) {
  c(`add(${js(a)})`, ($) => $("#li3, #p2").add(...a))
}
c("add-el", ($, env) => $("#li3").add(env.doc.getElementById("li1")))
c("add-arr", ($, env) => $("#li3").add([env.doc.getElementById("li2"), env.doc.getElementById("li1")]))
c("add-jq", ($) => $("#li3").add($("p")))
c("add-nodelist", ($, env) => $("#li3").add(env.doc.querySelectorAll("li")))
c("add-detached", ($) => $("<i>").add("<b>").add("li"))
for (const a of [[], ["li"], [".odd"], [undefined], [""]]) {
  c(`addBack(${js(a)})`, ($) => $("#list").find("li").addBack(...a))
  c(`addBack2(${js(a)})`, ($) => $("#li2").nextAll().addBack(...a))
}
c("addBack-noprev", ($) => $("li").addBack())
for (const a of ["b", "i", "li", "#b1", "", "*"]) c(`has(${a})`, ($) => $("#main, #p1, #p2, li").has(a))
c("has-el", ($, env) => $("p, div").has(env.doc.getElementById("b1")))
c("has-jq", ($) => $("p, div, ul").has($("#b1, #li2")))
c("has-none", ($) => $("p").has(undefined))

// find / filter / not / is / $.filter
for (const s of ["#main", "p", "#list, #p1", "#nonexist", "li"]) {
  for (const a of ["li", "b, i", "*", ".odd", "", "p b", "> li", ":first", "li:eq(2)", "#li3"]) {
    c(`find(${s}|${a})`, ($) => $(s).find(a))
  }
  c(`find(${s}|el)`, ($, env) => $(s).find(env.doc.getElementById("li2")))
  c(`find(${s}|jq)`, ($) => $(s).find($("li, b, #p2")))
  c(`find(${s}|undef)`, ($) => $(s).find(undefined))
  c(`find(${s}|arr)`, ($, env) => $(s).find([env.doc.getElementById("li2"), env.doc.getElementById("b1")]))
}
const fargs = [".odd", "li", "#li3", "", ":even", ":first", "li:last", "*", "p, li", "div > ul > li", ":not(.odd)"]
for (const m of ["filter", "not", "is"]) {
  for (const s of ["li", "#main *", "#nonexist", "#li1", "text"]) {
    for (const a of fargs) c(`${m}(${s}|${a})`, ($, env) => pick($, env, s)[m](a))
    c(`${m}(${s}|undef)`, ($, env) => pick($, env, s)[m](undefined))
    c(`${m}(${s}|null)`, ($, env) => pick($, env, s)[m](null))
    c(`${m}(${s}|noarg)`, ($, env) => pick($, env, s)[m]())
    c(`${m}(${s}|el)`, ($, env) => pick($, env, s)[m](env.doc.getElementById("li3")))
    c(`${m}(${s}|arr)`, ($, env) => pick($, env, s)[m]([env.doc.getElementById("li3"), env.doc.getElementById("li1")]))
    c(`${m}(${s}|jq)`, ($, env) => pick($, env, s)[m]($(".odd")))
    c(`${m}(${s}|fn)`, ($, env) => pick($, env, s)[m](rec(env, "f", function (i, el) { return i % 2 === 0 || this.id === "li4" })))
    c(`${m}(${s}|fnundef)`, ($, env) => pick($, env, s)[m](rec(env, "f", undefined)))
  }
}
c("$.filter", ($) => $.filter(".odd", $("li").get()))
c("$.filter-not", ($) => $.filter(".odd", $("li").get(), true))
c("$.filter-one", ($) => $.filter("li", [$("#li2")[0]]))
c("$.filter-one-not", ($) => $.filter("li", [$("#li2")[0]], true))
c("$.filter-text", ($) => $.filter("*", $("#main").contents().get()))
c("$.filter-one-text", ($) => $.filter("*", [$("#main").contents()[0]]))
c("filter-chain-end", ($) => $("li").filter(".odd").end())

// ---- attributes ----
const attrTargets = ["#a1", "#c1", "#c2", "a", "#nonexist", "#svg", "#circ", "text", "#t1", "#s2"]
for (const s of attrTargets) {
  for (const n of ["id", "href", "checked", "CHECKED", "tabindex", "title", "type", "class", "multiple", "selected", "disabled", "value", "data-x"]) {
    c(`attr(${s}|${n})`, ($, env) => pick($, env, s).attr(n))
  }
}
for (const s of ["#a2", "a", "#c2", "#nonexist", "text", "#circ", "#t1"]) {
  const sets = [["title", "x"], ["title", 5], ["title", null], ["title", undefined], ["title", ""], ["checked", true], ["checked", false], ["checked", "checked"], ["disabled", "x"], ["type", "radio"], ["type", "checkbox"], ["value", "vv"], ["data-x", { a: 1 }], ["DATA-Y", "q"], ["title", false], ["title", true]]
  for (const a of sets) c(`attrset(${s}|${js(a)})`, ($, env) => [pick($, env, s).attr(...a), env.doc.body.innerHTML])
  c(`attrset(${s}|obj)`, ($, env) => [pick($, env, s).attr({ title: "t", "data-z": 3, checked: true, alt: null }), env.doc.body.innerHTML])
  c(`attrset(${s}|fn)`, ($, env) => [pick($, env, s).attr("title", rec(env, "a", function (i, old) { return "n" + i + old })), env.doc.body.innerHTML])
  c(`attrset(${s}|fnundef)`, ($, env) => [pick($, env, s).attr("title", rec(env, "a", undefined)), env.doc.body.innerHTML])
  c(`attrset(${s}|fnnull)`, ($, env) => [pick($, env, s).attr("id", rec(env, "a", null)), env.doc.body.innerHTML])
  c(`attrset(${s}|objfn)`, ($, env) => [pick($, env, s).attr({ title: rec(env, "o", (i) => "o" + i) }), env.doc.body.innerHTML])
  for (const a of [["title"], ["title id"], [undefined], [""], ["checked"], ["  title\thref "], [null]]) {
    c(`removeAttr(${s}|${js(a)})`, ($, env) => [pick($, env, s).removeAttr(...a), env.doc.body.innerHTML])
  }
}
c("attr-noargs", ($) => $("#a1").attr())
c("$.attr-get", ($, env) => [$.attr(env.doc.getElementById("a1"), "href"), $.attr(env.doc.getElementById("c1"), "checked"), $.attr(env.doc.getElementById("a1"), "zz")])
c("$.attr-set", ($, env) => [$.attr(env.doc.getElementById("a1"), "title", "q"), $.attr(env.doc.getElementById("a1"), "title", null), env.doc.body.innerHTML])
c("$.attr-text", ($, env) => [$.attr(env.doc.createTextNode("x"), "title"), $.attr(env.doc.createComment("x"), "title", "x")])
c("$.attr-doc", ($, env) => { try { return $.attr(env.doc, "title") } catch (e) { return { err: e.name } } })
c("$.removeAttr", ($, env) => [$.removeAttr(env.doc.getElementById("a1"), "href tabindex"), env.doc.body.innerHTML])
c("$.removeAttr-empty", ($, env) => [$.removeAttr(env.doc.getElementById("a1"), ""), $.removeAttr(env.doc.getElementById("a1")), $.removeAttr(env.doc.createTextNode("x"), "a")])
c("attrHooks-keys", ($) => Object.keys($.attrHooks))
c("attrHooks-type-set", ($, env) => typeof $.attrHooks.type.set)
c("attrHooks-custom", ($, env) => { $.attrHooks.foo = { get: (e, n) => "G" + n, set: (e, v, n) => { e.setAttribute(n, "S" + v); return true } }; const r = [$("#a1").attr("foo"), $("#a1").attr("foo", "x").attr("foo"), env.doc.getElementById("a1").getAttribute("foo")]; delete $.attrHooks.foo; return r })
c("attrHooks-custom-undef", ($, env) => { $.attrHooks.bar = { get: () => null, set: () => undefined }; const r = [$("#a1").attr("bar"), $("#a1").attr("bar", "x").attr("bar")]; delete $.attrHooks.bar; return r })
c("find.attr-bool", ($, env) => [$.find.attr(env.doc.getElementById("c1"), "checked"), $.find.attr(env.doc.getElementById("c2"), "checked"), $.expr.attrHandle.checked(env.doc.getElementById("c1"), "checked", false), $.expr.attrHandle.checked(env.doc.getElementById("c1"), "checked", true), $.expr.attrHandle.checked(env.doc.getElementById("c1"), "CHECKED", false)])
c("attrHandle-keys", ($) => Object.keys($.expr.attrHandle).sort())
c("attr-sel-bool", ($) => $("input[checked]"))
function xmlDoc(env) { const x = env.doc.implementation.createDocument(null, "r", null); const n = x.createElement("n"); n.setAttribute("checked", "q"); n.setAttribute("Title", "t"); n.appendChild(x.createTextNode("t")); x.documentElement.appendChild(n); return x }
c("attr-xml", ($, env) => { const x = xmlDoc(env); const n = x.documentElement.firstChild; return [$(n).attr("checked"), $(n).attr("Title"), $(n).attr("title"), $(n).attr("checked", false).attr("checked"), $(n).attr("Title", "u").attr("Title")] })

// prop
for (const s of ["#c1", "#c2", "#a1", "#a2", "#a3", "#t1", "#btn", "#sp", "#o2", "#o1", "#s1", "#nonexist", "text", "#li1", "#circ", "input"]) {
  for (const n of ["checked", "tabIndex", "tabindex", "class", "for", "className", "id", "selected", "value", "nodeName", "readonly", "readOnly", "maxlength", "contenteditable", "href", "nope", "selectedIndex", "disabled"]) {
    c(`prop(${s}|${n})`, ($, env) => pick($, env, s).prop(n))
  }
}
for (const s of ["#c2", "input", "#nonexist", "text", "#a1", "#o3"]) {
  for (const a of [["checked", true], ["checked", false], ["foo", 1], ["foo", undefined], ["foo", null], ["class", "k"], ["tabIndex", 4], ["tabindex", 5], ["selected", true], ["readonly", true]]) {
    c(`propset(${s}|${js(a)})`, ($, env) => { const r = pick($, env, s).prop(...a); return [r, env.doc.body.innerHTML, pick($, env, s).prop(a[0])] })
  }
  c(`propset(${s}|obj)`, ($, env) => [pick($, env, s).prop({ foo: "b", checked: true }), pick($, env, s).prop("foo"), env.doc.body.innerHTML])
  c(`propset(${s}|fn)`, ($, env) => [pick($, env, s).prop("title", rec(env, "p", (i, old) => "x" + i + old)), env.doc.body.innerHTML])
  for (const n of ["foo", "class", "for", undefined, "tabIndex"]) {
    c(`removeProp(${s}|${n})`, ($, env) => { const set = pick($, env, s); set.prop("foo", 3); set.each(function () { this.className2 = 1 }); const r = set.removeProp(n); return [r, set.prop("foo"), set.prop("className2"), env.doc.body.innerHTML] })
  }
}
c("$.prop", ($, env) => [$.prop(env.doc.getElementById("c1"), "checked"), $.prop(env.doc.getElementById("a1"), "tabIndex"), $.prop(env.doc.getElementById("c2"), "checked", true), $.prop(env.doc.createTextNode("x"), "checked"), $.prop(env.doc.getElementById("a1"), "class")])
c("propFix", ($) => $.propFix)
c("propHooks-keys", ($) => Object.keys($.propHooks))
c("propHooks-tabIndex", ($, env) => ["a1", "a2", "a3", "t1", "btn", "sp", "li1", "s1", "ta"].map((id) => $.propHooks.tabIndex.get(env.doc.getElementById(id))))
c("propHooks-custom", ($, env) => { $.propHooks.zz = { get: () => "G", set: (e, v) => { e.zz2 = v; return "r" } }; const r = [$("#a1").prop("zz"), $("#a1").prop("zz", 5).prop("zz2")]; delete $.propHooks.zz; return r })
c("prop-xml", ($, env) => { const x = xmlDoc(env); const n = x.documentElement.firstChild; return [$(n).prop("class"), $(n).prop("nodeName"), $(n).prop("for", 3).prop("for")] })

// classes
const classTargets = ["#main", "li", "#nonexist", "#circ", "text", "#p1, #p2, #b1", "#main, #main"]
const classArgs = [["q"], ["q  r\tq"], [" a "], [["q", "r"]], [[]], [""], [undefined], [null], [5], ["a b"], [["a", "x y"]], [{}]]
for (const s of classTargets) {
  for (const m of ["addClass", "removeClass", "toggleClass"]) {
    for (const a of classArgs) c(`${m}(${s}|${js(a)})`, ($, env) => [pick($, env, s)[m](...a), env.doc.body.innerHTML])
    c(`${m}(${s}|fn)`, ($, env) => [pick($, env, s)[m](rec(env, "k", function (i, cls, st) { return "f" + i + " x" })), env.doc.body.innerHTML])
    c(`${m}(${s}|fnarr)`, ($, env) => [pick($, env, s)[m](rec(env, "k", (i) => ["g" + i, "a"])), env.doc.body.innerHTML])
    c(`${m}(${s}|fnundef)`, ($, env) => [pick($, env, s)[m](rec(env, "k", undefined)), env.doc.body.innerHTML])
  }
  c(`removeClass(${s}|none)`, ($, env) => [pick($, env, s).removeClass(), env.doc.body.innerHTML])
  for (const a of [["x", true], ["x", false], ["a", true], ["a", false], [["a", "z"], true], [["a", "z"], false], ["a", 1], ["a", "yes"], ["a", undefined], [], [true], [false], [undefined], [null], [false, true], [true, false], [undefined, false], ["x y a"], [5]]) {
    c(`toggle2(${s}|${js(a)})`, ($, env) => [pick($, env, s).toggleClass(...a), env.doc.body.innerHTML])
  }
  c(`toggleClass(${s}|fnstate)`, ($, env) => [pick($, env, s).toggleClass(rec(env, "t", (i, cls, st) => "a z" + i), false), env.doc.body.innerHTML])
  c(`toggleClass(${s}|fnstate2)`, ($, env) => [pick($, env, s).toggleClass(rec(env, "t", (i, cls, st) => "a z" + i), true), env.doc.body.innerHTML])
  c(`toggleClass(${s}|whole-roundtrip)`, ($, env) => { const x = pick($, env, s); const r = [x.toggleClass().attr("class"), env.doc.body.innerHTML, x.toggleClass().attr("class"), x.toggleClass(false).attr("class"), x.toggleClass(true).attr("class"), x.toggleClass(false).toggleClass(false).attr("class")]; return [r, env.doc.body.innerHTML] })
  for (const a of ["a", "b", "x", "a b", "", " a", "c1", "q", undefined]) c(`hasClass(${s}|${a})`, ($, env) => pick($, env, s).hasClass(a))
}
c("hasClass-noarg", ($) => $("#main").hasClass())
c("addClass-empty-attr", ($, env) => [$("#li2").addClass("   ").attr("class"), $("#li2").attr("class", "  a   b ").addClass("b").attr("class"), $("#li2").removeClass("zz").attr("class")])
c("addClass-dup", ($, env) => [$("#li2").addClass("m m n").attr("class"), $("#li2").removeClass("m m").attr("class")])
c("class-doc-win", ($, env) => [$(env.doc).addClass("x").removeClass("x").toggleClass("x").hasClass("x"), $(env.win).addClass("x").hasClass("x"), $({}).addClass("x").hasClass("x")])

// val
for (const s of ["#t1", "#t2", "#c1", "#c3", "#r2", "#s1", "#s2", "#s3", "#ta", "#btn", "#o1", "#o3", "#nonexist", "#main", "text", "#p1", "input", "select", "option"]) {
  c(`val(${s})`, ($, env) => pick($, env, s).val())
}
for (const s of ["#t1", "#c1", "#c3", "input[type=checkbox]", "#r1", "input[type=radio]", "#s1", "#s2", "#s3", "#ta", "#btn", "#o1", "#nonexist", "text", "#p1", "input, select, textarea"]) {
  for (const a of ["x", 5, 0, null, undefined, "", ["c1", "c2"], ["b", "d"], ["o1"], ["o2", "c"], [null, 5, "r1"], [], true, "o3", "Three ", "Three", "c", { a: 1 }]) {
    c(`valset(${s}|${js(a)})`, ($, env) => { const x = pick($, env, s); const r = x.val(a); return [r, x.map(function () { return [this.value, this.checked, this.selectedIndex] }).get(), env.doc.body.innerHTML, x.val()] })
  }
  c(`valset(${s}|fn)`, ($, env) => { const x = pick($, env, s); const r = x.val(rec(env, "v", function (i, v) { return "f" + i + v })); return [r, x.map(function () { return this.value }).get(), x.val()] })
  c(`valset(${s}|fnarr)`, ($, env) => { const x = pick($, env, s); const r = x.val(rec(env, "v", (i, v) => ["c1", "b", "o1"])); return [r, x.map(function () { return [this.value, this.checked, this.selected] }).get(), x.val()] })
  c(`valset(${s}|fnnull)`, ($, env) => { const x = pick($, env, s); const r = x.val(rec(env, "v", () => null)); return [r, x.map(function () { return this.value }).get()] })
  c(`valset(${s}|fnnum)`, ($, env) => { const x = pick($, env, s); const r = x.val(rec(env, "v", (i) => i * 3)); return [r, x.map(function () { return this.value }).get()] })
}
c("val-select-none", ($, env) => { $("#s1")[0].selectedIndex = -1; return [$("#s1").val(), $("#s2").val([]).val(), $("#s2")[0].selectedIndex] })
c("val-select-disabled-parent", ($) => { $("#s2 optgroup").prop("disabled", false); return $("#s2").val() })
c("val-select-one-disabled", ($) => { $("#o2").prop("disabled", true); return $("#s1").val() })
c("val-option-text", ($) => [$("#o3").val(), $("<option> a  b </option>").val(), $("<option value=''>x</option>").val()])
c("valHooks-keys", ($) => Object.keys($.valHooks))
c("valHooks-direct", ($, env) => [$.valHooks.option.get(env.doc.getElementById("o3")), $.valHooks.select.get(env.doc.getElementById("s2")), $.valHooks.select.set(env.doc.getElementById("s2"), ["d"]), $.valHooks.checkbox.set(env.doc.getElementById("c3"), ["on"]), $.valHooks.radio.set(env.doc.getElementById("c3"), "on"), typeof $.valHooks.checkbox.get, $.valHooks.select.get(env.doc.getElementById("s1"))])
c("valHooks-custom", ($, env) => { $.valHooks.text = { get: (e) => "G" + e.value, set: (e, v) => { e.value = "S" + v; return true } }; const r = [$("#t1").val(), $("#t1").val("q").val(), env.doc.getElementById("t1").value]; delete $.valHooks.text; return r })
c("valHooks-custom-undef", ($, env) => { $.valHooks.text = { get: () => undefined, set: () => undefined }; const r = [$("#t1").val(), $("#t1").val("q").val()]; delete $.valHooks.text; return r })
c("val-crlf", ($, env) => { env.doc.getElementById("t2").value = "a\r\nb\rc"; return [$("#t2").val(), $("#ta").val().length] })
c("val-numstring", ($) => [$("#t1").val(1.5).val(), $("#t1").val(-0).val(), $("#t1").val(NaN).val(), $("#t1").val([1, null, "x"]).val()])

// ---- manipulation ----
const insArgs = {
  html: () => "<b class='n'>n</b>",
  text: () => "plain & <text",
  multi: () => "<i>1</i><i>2</i>",
  zero: () => 0,
  num: () => 7,
  empty: () => "",
  undef: () => undefined,
  nul: () => null,
  el: (env) => env.doc.getElementById("li4"),
  arr: (env) => [env.doc.getElementById("li4"), "<u>u</u>", env.doc.getElementById("b1")],
  nested: (env) => [[env.doc.getElementById("li1")], ["<s>s</s>", ["t"]]],
  jq: (env, $) => $(".s"),
  newel: (env) => env.doc.createElement("em"),
  frag: (env) => { const f = env.doc.createDocumentFragment(); f.appendChild(env.doc.createElement("q")); f.appendChild(env.doc.createTextNode("ft")); return f },
  tr: () => "<tr><td>new</td></tr>",
  td: () => "<td>td</td>",
  option: () => "<option value='z'>Z</option>",
  col: () => "<col>",
  thead: () => "<thead><tr><th>h</th></tr></thead>",
  script: () => "<script>window.__log.push('ran')<\/script>",
  scripttpl: () => "<script type='text/template'>window.__log.push('no')<\/script>",
  scriptmod: () => "<div><script type='module'>window.__log.push('mod')<\/script></div>",
  scriptsrc: () => "<script src='http://localhost/x.js' nonce='abc'><\/script>",
  scriptsrcmod: () => "<script src='http://localhost/m.js' type='module'><\/script>",
  scriptnomod: () => "<script src='http://localhost/n.js' nomodule><\/script>",
  cdata: () => "<script><![CDATA[window.__log.push('cdata')]]><\/script>",
  checked: () => "<input type='checkbox' checked='checked'>",
  comment: () => "<!-- c -->",
  whitespace: () => "  <b>ws</b>  ",
  textnode: (env) => env.doc.createTextNode("tn2"),
  fnstr: (env) => rec(env, "m", function (i, h) { return "<i>f" + i + "</i>" }),
  fnel: (env) => rec(env, "m", function (i, h) { return env.doc.createElement("kbd") }),
  fnundef: (env) => rec(env, "m", undefined),
  table: () => "<table><tr><td>x</td></tr></table>",
  xhtml: () => "<div/><span/>",
}
const manipTargets = ["#p1", "li", "#nonexist", "#tbl", "#tbl2", "#s3", "text", "#empty", "#li2, #p2"]
for (const m of ["append", "prepend", "before", "after", "replaceWith"]) {
  for (const t of manipTargets) {
    for (const [k, mk] of Object.entries(insArgs)) {
      c(`${m}(${t}|${k})`, ($, env) => {
        env.win._evalUrl_log = []
        const orig = $._evalUrl
        $._evalUrl = function (url, opts, doc) { env.log.push(["evalUrl", url, ser(opts, env), doc === env.doc, this === $, arguments.length]) }
        try {
          const r = pick($, env, t)[m](mk(env, $))
          return [r, env.doc.body.innerHTML, env.win.__log.slice()]
        } finally { $._evalUrl = orig }
      })
    }
    c(`${m}(${t}|multiargs)`, ($, env) => [pick($, env, t)[m]("<a1>", $("#li1"), ["x", 3], env.doc.createElement("hr")), env.doc.body.innerHTML])
    c(`${m}(${t}|noargs)`, ($, env) => [pick($, env, t)[m](), env.doc.body.innerHTML])
  }
}
for (const m of ["appendTo", "prependTo", "insertBefore", "insertAfter", "replaceAll"]) {
  for (const src of ["<b>n</b>", "#li4", ".s", "#nonexist", "<i>1</i><i>2</i>"]) {
    for (const tgt of ["#p1", "li", "#nonexist", ".odd", "#empty"]) {
      c(`${m}(${src}|${tgt})`, ($, env) => { const r = $(src)[m](tgt); return [r, env.doc.body.innerHTML] })
    }
    c(`${m}(${src}|el)`, ($, env) => [$(src)[m](env.doc.getElementById("li2")), env.doc.body.innerHTML])
    c(`${m}(${src}|jq)`, ($, env) => [$(src)[m]($("#p2, #li3")), env.doc.body.innerHTML])
    c(`${m}(${src}|html)`, ($, env) => [$(src)[m]("<div id='nn'></div>"), env.doc.body.innerHTML])
  }
}
c("appendTo-events-clone", ($, env) => { $(".s").on("click", function () { env.log.push(["clk", this.textContent]) }).data("k", 1); const r = $(".s").appendTo("li"); $("li .s").trigger("click"); return [r, $("li .s").map(function () { return $(this).data("k") }).get(), env.doc.body.innerHTML] })
c("append-fn-multi", ($, env) => { const r = $("li").append(rec(env, "af", function (i, h) { return "<i>" + i + h + "</i>" })); return [r, env.doc.body.innerHTML] })
c("append-checkclone", ($, env) => [$("li").append("<input type='checkbox' checked='checked'>").find("input").map(function () { return this.checked }).get(), env.doc.body.innerHTML])
c("append-script-multi", ($, env) => { $("li").append("<script>window.__log.push('m')<\/script>"); return [env.win.__log.slice(), env.doc.body.innerHTML] })
c("append-script-moved", ($, env) => { $("#p1").append("<script>window.__log.push('once')<\/script>"); $("#p2").append($("#p1 script")); $("#p2 script").appendTo("#li1"); return env.win.__log.slice() })
c("append-script-detached", ($, env) => { const d = $("<div>"); d.append("<script>window.__log.push('det')<\/script>"); const r1 = env.win.__log.slice(); $("#p1").append(d); return [r1, env.win.__log.slice()] })
c("append-script-clone", ($, env) => { $("#p1").append("<script>window.__log.push('c1')<\/script>"); const cl = $("#p1 script").clone(); $("#p2").append(cl); const d = $("<div><script>window.__log.push('c2')<\/script></div>"); const cl2 = d.clone(); $("#p2").append(cl2); $("#li1").append(d); return env.win.__log.slice() })
c("text-get", ($, env) => [$("#main").text(), $("li").text(), $("#nonexist").text(), $(env.doc).text().length, pick($, env, "text").text(), $("#main").contents().text()])
for (const t of ["#p1", "li", "#nonexist", "text", "#main, #li1"]) {
  for (const v of ["x<b>", 5, 0, "", null, true, { a: 1 }]) c(`text(${t}|${js(v)})`, ($, env) => [pick($, env, t).text(v), env.doc.body.innerHTML])
  c(`text(${t}|undef)`, ($, env) => [pick($, env, t).text(undefined), env.doc.body.innerHTML])
  c(`text(${t}|fn)`, ($, env) => [pick($, env, t).text(rec(env, "tx", function (i, old) { return "T" + i + old.length })), env.doc.body.innerHTML])
  c(`text(${t}|fnundef)`, ($, env) => [pick($, env, t).text(rec(env, "tx", undefined)), env.doc.body.innerHTML])
  c(`html-get(${t})`, ($, env) => pick($, env, t).html())
  for (const v of ["<b>h</b>", "plain", "<script>window.__log.push('h')<\/script><i>s</i>", "<td>x</td>", "<tr><td>y</td></tr>", "<option>o</option>", "<link rel='x'>", "<style>b{}</style>", 5, 0, "", null, undefined, true, "<div/><p/>", " <b>lead</b>"]) {
    c(`html(${t}|${js(v)})`, ($, env) => [pick($, env, t).html(v), env.doc.body.innerHTML, env.win.__log.slice()])
  }
  c(`html(${t}|el)`, ($, env) => [pick($, env, t).html(env.doc.getElementById("li4")), env.doc.body.innerHTML])
  c(`html(${t}|jq)`, ($, env) => [pick($, env, t).html($(".s")), env.doc.body.innerHTML])
  c(`html(${t}|fn)`, ($, env) => [pick($, env, t).html(rec(env, "h", function (i, old) { return "<u>" + i + old.length + "</u>" })), env.doc.body.innerHTML])
  c(`html(${t}|fnundef)`, ($, env) => [pick($, env, t).html(rec(env, "h", undefined)), env.doc.body.innerHTML])
  c(`empty(${t})`, ($, env) => [pick($, env, t).empty(), env.doc.body.innerHTML])
  for (const sel of [undefined, "", ".odd", "li", "#p1", "*"]) {
    c(`remove(${t}|${sel})`, ($, env) => { const x = pick($, env, t); x.find("*").addBack().data("q", 1).on("click", () => env.log.push("c")); const r = x.remove(sel); x.trigger("click"); return [r, env.doc.body.innerHTML, x.map(function () { return $.hasData(this) }).get()] })
    c(`detach(${t}|${sel})`, ($, env) => { const x = pick($, env, t); x.data("q", 1).on("click", () => env.log.push("c")); const r = x.detach(sel); x.trigger("click"); return [r, env.doc.body.innerHTML, x.map(function () { return $.hasData(this) }).get()] })
  }
}
c("html-select-option", ($, env) => [$("#s3").html("<option>a</option><option selected>b</option>"), $("#s3").val(), env.doc.body.innerHTML])
c("html-cleans-data", ($, env) => { $("#p1 b").data("x", 1).on("click", () => env.log.push("b")); const b = $("#p1 b")[0]; $("#p1").html("new"); return [$.hasData(b), $._data ? $._data(b, "events") : null] })
c("html-xml", ($, env) => { const x = xmlDoc(env); return [$(x.documentElement).html(), $(x.documentElement).text()] })
c("empty-cleans", ($, env) => { $("#p1 b").data("x", 1); const b = $("#p1 b")[0]; $("#p1").empty(); return [$.hasData(b), env.doc.body.innerHTML] })
c("empty-doc", ($, env) => { const x = $("<div><b>1</b></div>"); return [x.empty().html(), $({}).empty().length, $([env.doc.createTextNode("x")]).empty().length] })
const cloneSetup = ($, env) => {
  $("#p1").data("k", { a: 1 }).on("click.ns", function (e) { env.log.push(["p1", e.namespace, this.id]) })
  $("#b1").data("kb", 2).on("mouseover", function () { env.log.push(["b1", this.id]) })
  $("#c2").prop("checked", true)
  $("#ta").val("changed")
}
for (const args of [[], [true], [false], [true, false], [true, true], [false, true], [undefined, true], [1], [null, null], [0, 1]]) {
  c(`clone(${js(args)})`, ($, env) => {
    cloneSetup($, env)
    const cl = $("#p1, #c2, #ta, #s2, #tpl").clone(...args)
    $("#other").append(cl)
    cl.trigger("click"); cl.find("b").trigger("mouseover")
    return [cl, cl.map(function () { return [$.hasData(this), $(this).data("k"), this.checked, this.value, this.defaultValue] }).get(), cl.find("b").data("kb"), env.doc.body.innerHTML]
  })
}
c("$.clone", ($, env) => { cloneSetup($, env); const a = $.clone(env.doc.getElementById("p1")); const b = $.clone(env.doc.getElementById("p1"), true); const d = $.clone(env.doc.getElementById("p1"), true, true); return [a, b, d, $.hasData(a), $(b).data("k"), $(d).find("b").data("kb"), $.clone(env.doc.createTextNode("tx")), $.clone(env.doc.getElementById("c2")).checked] })
c("$.clone-script", ($, env) => { $("#p1").append("<script>window.__log.push('cs')<\/script>"); const s = $.clone($("#p1")[0]); $("#p2").append(s); return env.win.__log.slice() })
c("$.cleanData", ($, env) => { $("#p1, #b1").data("x", 1).on("click", () => env.log.push("c")); const r = $.cleanData($("#p1, #b1").get()); $("#p1").trigger("click"); return [r, $.hasData($("#p1")[0]), $.hasData($("#b1")[0])] })
c("$.cleanData-special", ($, env) => { $("#p1").on("mouseenter focusin", () => env.log.push("m")); $.cleanData([$("#p1")[0], undefined, $("#b1")[0]]); $("#p1").trigger("mouseenter").trigger("focusin"); return env.log.slice() })
c("$.cleanData-arraylike", ($, env) => { $("#p1").data("x", 1); return [$.cleanData({ 0: $("#p1")[0], length: 1 }), $.hasData($("#p1")[0]), $.cleanData([]), $.cleanData([env.doc.createTextNode("x")])] })
c("$.htmlPrefilter", ($) => [$.htmlPrefilter("<div/>"), $.htmlPrefilter(5), $.htmlPrefilter()])
c("htmlPrefilter-override", ($, env) => { const o = $.htmlPrefilter; $.htmlPrefilter = (h) => h.replace("x", "Y"); try { $("#p1").html("<b>x</b>"); $("#p2").append("<i>x</i>"); return env.doc.body.innerHTML } finally { $.htmlPrefilter = o } })
c("replaceWith-self", ($, env) => [$("li").replaceWith($("#li2")), env.doc.body.innerHTML])
c("replaceWith-fn", ($, env) => [$("li").replaceWith(rec(env, "rw", function (i) { return "<b>" + i + this.id + "</b>" })), env.doc.body.innerHTML])
c("replaceWith-events", ($, env) => { $("#li2").on("click", () => env.log.push("li2")); const old = $("#li2")[0]; $("#li2").replaceWith("<li id='n'>n</li>"); return [$.hasData(old), env.doc.body.innerHTML] })
c("domManip-evalUrl-real", ($, env) => { const capt = []; $.ajaxPrefilter("script", function (s, orig, xhr) { if (s.url.indexOf("zz.js") >= 0) { capt.push([s.url, s.type, s.dataType, s.cache, s.async, s.global, typeof s.dataFilter, Object.keys(s.converters || {}).includes("text script")]); throw new Error("stop") } }); let t = null; try { $("#p1").append("<script src='http://localhost/zz.js' nonce='nn'><\/script>") } catch (e) { t = e.message } return [capt, t] })
c("_evalUrl-direct", ($, env) => { const capt = []; $.ajaxPrefilter(function (s, orig, xhr) { if (s.url.indexOf("ev.js") >= 0) { capt.push([s.url, s.type, s.dataType, s.cache, s.async, s.global, Object.keys(orig).sort(), orig.converters && typeof orig.converters["text script"], orig.dataFilter && orig.dataFilter.length]); if (orig.dataFilter) orig.dataFilter("window.__log.push('df')"); if (orig.dataFilter) orig.dataFilter(55); throw new Error("stop2") } }); let t = null; try { $._evalUrl("http://localhost/ev.js", { nonce: "n" }, env.doc) } catch (e) { t = e.message } return [capt, t, env.win.__log.slice()] })

// wrap
const wrapArgs = {
  html: () => "<div class='w'><span class='in'></span></div>",
  simple: () => "<section/>",
  el: (env) => env.doc.getElementById("empty"),
  jq: (env, $) => $(".s"),
  sel: () => "#empty",
  fn: (env) => rec(env, "w", function (i) { return "<em class='f" + i + "'></em>" }),
  fnel: (env) => rec(env, "w", function (i) { return env.doc.createElement("kbd") }),
  fnundef: (env) => rec(env, "w", undefined),
  none: () => undefined,
  deep: () => "<div><p><b></b></p><i></i></div>",
  text: () => "text",
}
for (const m of ["wrapAll", "wrapInner", "wrap"]) {
  for (const t of ["#li2", "li", "#nonexist", "#p1 b, #p1 i", "text", "#empty", "#b1"]) {
    for (const [k, mk] of Object.entries(wrapArgs)) {
      c(`${m}(${t}|${k})`, ($, env) => [pick($, env, t)[m](mk(env, $)), env.doc.body.innerHTML])
    }
  }
}
for (const t of ["#b1", "li", "#nonexist", "#p1 *", "#main", "text", "#p1"]) {
  for (const a of [[], ["p"], ["#list"], ["div"], [undefined], ["body"]]) c(`unwrap(${t}|${js(a)})`, ($, env) => [pick($, env, t).unwrap(...a), env.doc.body.innerHTML])
}
c("unwrap-body", ($, env) => [$("#main").unwrap(), env.doc.body.innerHTML])
c("wrapAll-detached", ($) => [$("<b>x</b>").wrapAll("<div>").parent(), $("<b>x</b>").wrap("<p>").parent()])

// ---- deprecated ----
c("proxy", ($, env) => {
  const obj = { name: "o", f: function (...a) { return [this && this.name, a.length, ...a] } }
  const p1 = $.proxy(obj.f, obj)
  const p2 = $.proxy(obj, "f")
  const p3 = $.proxy(obj.f, obj, 1, 2)
  const p4 = $.proxy(obj.f, null, "x")
  const p5 = $.proxy(obj.f)
  const g = [p1.guid === obj.f.guid, typeof p1.guid, p2.guid === p1.guid, p3.guid === p1.guid]
  return [p1(3), p2(4, 5), p3(6), p4.call({ name: "ctx" }, 7), p5.call({ name: "c5" }), g, p1.length, typeof p1, $.proxy(5), $.proxy(obj, "nope"), $.proxy(), $.proxy(obj, 5), p3.call({ name: "z" })]
})
c("proxy-guid-reuse", ($) => { const f = function () { return 1 }; f.guid = 77; const p = $.proxy(f, {}); return [p.guid, f.guid, p()] })
c("proxy-guid-seq", ($) => { const a = $.proxy(function () {}, {}); const b = $.proxy(function () {}, {}); return [b.guid - a.guid, typeof $.guid] })
c("proxy-events", ($, env) => { const o = { n: "o", h: function (e) { env.log.push([this.n, e.type]) } }; $("#p1").on("click", $.proxy(o.h, o)); $("#p1").off("click", $.proxy(o.h, o)); $("#p1").trigger("click"); $("#p1").on("click", $.proxy(o, "h")).trigger("click"); return env.log.slice() })
c("holdReady", ($, env) => { const w = $.readyWait; $.holdReady(true); const a = $.readyWait; $.holdReady(true); const b = $.readyWait; $.holdReady(false); const d = $.readyWait; const r = $.holdReady(false); return [a - w, b - w, d - w, $.readyWait - w, r, $.isReady] })
c("isArray", ($) => [$.isArray([]), $.isArray({ length: 0 }), $.isArray(), $.isArray("a"), $.isArray === Array.isArray])
c("parseJSON", ($) => { const out = []; for (const v of ['{"a":[1,2]}', "3", "null", '"s"', " [1] "]) out.push($.parseJSON(v)); for (const v of ["", "{a:1}", undefined]) { try { out.push($.parseJSON(v)) } catch (e) { out.push({ err: e.name }) } } out.push($.parseJSON === JSON.parse); out.push($.parseJSON(5)); out.push($.parseJSON(null)); return out })
c("nodeName", ($, env) => [$.nodeName(env.doc.getElementById("p1"), "P"), $.nodeName(env.doc.getElementById("p1"), "p"), $.nodeName(env.doc.getElementById("p1"), "div"), $.nodeName({}, "x"), $.nodeName(env.doc.getElementById("circ"), "CIRCLE")])
c("isFunction", ($) => [$.isFunction(() => 1), $.isFunction(function () {}), $.isFunction({}), $.isFunction(), $.isFunction(class {}), $.isFunction(async () => 1)])
c("isWindow", ($, env) => [$.isWindow(env.win), $.isWindow({}), $.isWindow(null), $.isWindow(), $.isWindow({ window: 1 })])
c("camelCase", ($) => ["a-b", "-ms-transform", "--custom-prop", "a-b-c", "abc", "", "-webkit-x", "a--b"].map((s) => $.camelCase(s)))
c("type", ($) => [1, "s", true, null, undefined, [], {}, () => 1, new Date(0), /x/, new Error("e"), Symbol("q"), new Number(1), new String("s"), Object.create(null)].map((v) => $.type(v)))
c("now", ($) => [typeof $.now(), Math.abs($.now() - Date.now()) < 1000, $.now === Date.now])
c("isNumeric", ($) => [1, -1, 0, "1", "1.5", "-1e5", " 2 ", "0x10", "", " ", "a", NaN, Infinity, -Infinity, "Infinity", null, undefined, true, [], [1], {}, new Number(3), "1e", ".5", "5.", 1e308 * 10, "1e400", Number.MAX_VALUE, "  ", "\t1\n", "0b1", "0o7"].map((v) => $.isNumeric(v)))
c("trim", ($) => ["  a  ", " x﻿", "", null, undefined, 5, " a b ", "\n\t z \r", { toString: () => " o " }, false, 0, "x"].map((v) => $.trim(v)))
c("deprecated-keys", ($) => ["proxy", "holdReady", "isArray", "parseJSON", "nodeName", "isFunction", "isWindow", "camelCase", "type", "now", "isNumeric", "trim"].map((k) => [k, typeof $[k], $[k] && $[k].length]))

// method surface: arity and presence
c("fn-surface", ($) => ["detach", "remove", "text", "append", "prepend", "before", "after", "empty", "clone", "html", "replaceWith", "appendTo", "prependTo", "insertBefore", "insertAfter", "replaceAll", "has", "closest", "index", "add", "addBack", "parent", "parents", "parentsUntil", "next", "prev", "nextAll", "prevAll", "nextUntil", "prevUntil", "siblings", "children", "contents", "find", "filter", "not", "is", "attr", "removeAttr", "prop", "removeProp", "addClass", "removeClass", "toggleClass", "hasClass", "val", "wrapAll", "wrapInner", "wrap", "unwrap"].map((k) => [k, typeof $.fn[k], $.fn[k] && $.fn[k].length]))
c("static-surface", ($) => ["htmlPrefilter", "clone", "cleanData", "filter", "attr", "removeAttr", "prop", "_evalUrl"].map((k) => [k, typeof $[k], $[k] && $[k].length]))
c("chain-end", ($) => [$("#main").find("li").parent().end().end(), $("li").eq(1).nextAll().prevAll().end()])
c("clone-this-map", ($) => $("li").clone().map(function (i) { return this.id + i }).get())


// ---- extra edge cases ----
c("propHooks-get-undef", ($, env) => { $.propHooks.q1 = { get: () => undefined }; const r = [$("#a1").prop("q1")]; env.doc.getElementById("a1").q1 = 5; r.push($("#a1").prop("q1")); delete $.propHooks.q1; return r })
c("attrHooks-get-undef", ($, env) => { $.attrHooks.q2 = { get: () => undefined }; const r = [$("#a1").attr("q2")]; env.doc.getElementById("a1").setAttribute("q2", "v"); r.push($("#a1").attr("q2")); delete $.attrHooks.q2; return r })
c("attrHooks-set-undef", ($, env) => { $.attrHooks.q3 = { set: () => undefined }; const r = [$("#a1").attr("q3", "x").attr("q3")]; delete $.attrHooks.q3; return r })
c("attr-num-name", ($, env) => { try { return [$("#a1").attr(5), $("#a1").attr(5, "v").attr("5")] } catch (e) { return { err: e.name } } })
c("removeAttr-num", ($, env) => { try { return [$("#a1").attr("5", "q").removeAttr(5).attr("5")] } catch (e) { return { err: e.name } } })
c("attr-doc-set", ($, env) => [$(env.doc).attr("title", "T").attr("title"), $(env.win).attr("q", "1").attr("q"), env.doc.title])
c("attr-obj", ($) => { try { return [$({ a: 1 }).attr("a"), $({ a: 1 }).attr("b", 2)] } catch (e) { return { err: e.name } } })
c("prop-obj", ($) => [$({ a: 1 }).prop("a"), $({ a: 1 }).prop("b", 2).prop("b"), $([{ for: 3 }]).prop("for")])
c("val-plainobj", ($) => { try { return $({ value: "v" }).val() } catch (e) { return { err: e.name } } })
c("val-doc", ($, env) => { try { return $(env.doc).val() } catch (e) { return { err: e.name } } })
c("valHooks-identity", ($) => [$.valHooks.radio === $.valHooks.checkbox, typeof $.valHooks.radio.set, Object.keys($.valHooks.radio), Object.keys($.valHooks.select)])
c("index-falsy", ($, env) => [$("li").index(""), $("li").index(0), $("li").index(false), $("#li3").index(undefined)])
c("filter-falsy", ($) => [$("li").filter(0), $("li").filter(false), $("li").not(0), $("li").not(""), $("li").is(0), $("li").is("")])
c("closest-array", ($, env) => [$("#b1").closest([env.doc.getElementById("p1")]), $("#b1").closest(":first"), $("#b1").closest([":first"])])
c("has-context", ($) => [$("#main, #other").has(".s"), $("ul").has("li:first")])
c("html-xml-bad", ($, env) => { const x = xmlDoc(env); const n = x.documentElement.firstChild; let r; try { r = $(n).html("<br>") } catch (e) { r = { err: e.name } } return [r, n.childNodes.length, new env.win.XMLSerializer().serializeToString(x)] })
c("html-text-set", ($, env) => [$("#main").contents().html("<b>q</b>"), env.doc.body.innerHTML])
c("html-emptyobj", ($) => [$({}).html(), $([]).html(), $([{}]).html("x").length])
c("text-emptyobj", ($) => { try { return [$([]).text(), $([]).text("x").length] } catch (e) { return { err: e.name } } })
c("toggleClass-stored", ($, env) => { const x = $("#p1"); x.toggleClass(); x.addClass("z"); x.toggleClass(); const a = x.attr("class"); x.toggleClass(); return [a, x.attr("class"), $._data(x[0], "__className__")] })
c("toggleClass-false-noattr", ($, env) => { const x = $("#li2"); return [x.toggleClass(false).attr("class"), x.toggleClass().attr("class"), x.toggleClass(true).attr("class")] })
c("addClass-svg-fn", ($, env) => [$("#circ").addClass(function (i, c) { return c + "-x" }).attr("class")])
c("removeClass-needle-dollar", ($, env) => [$("#li2").attr("class", "a$& b $1 c").removeClass("$&").removeClass("$1").attr("class")])
c("val-select-hooks-override", ($, env) => { const o = $.valHooks.option; $.valHooks.option = { get: (e) => "O" + e.text }; const r = [$("#s1").val(), $("#s1").val("OOne").val()]; $.valHooks.option = o; return r })
c("val-radio-arr", ($, env) => [$("input[type=radio]").val(["r1"]).map(function () { return this.checked }).get(), $("#r1").val()])
c("val-set-hookreturn", ($, env) => [$("#s2").val(["b", "d"]).val(), $("#c3").val(["on"]).prop("checked"), $("#c3").val("xx").val()])
c("prop-tabindex-attr", ($, env) => [$("#li1").attr("tabindex", "0").prop("tabIndex"), $("#li2").attr("tabindex", "x").prop("tabIndex"), $("#li3").attr("tabindex", "-2").prop("tabIndex")])
c("attr-bool-props", ($, env) => [$("#c2").attr("checked", "checked").prop("checked"), $("#c1").removeAttr("checked").prop("checked"), $("#s2").attr("multiple"), $("#t1").attr("readonly", true).attr("readonly"), $("#t1").prop("readOnly")])
c("attr-type-radio", ($, env) => { const i = $("<input value='v'>"); i.attr("type", "radio"); return [i.attr("type"), i.val(), i[0].value] })
c("wrap-text-fn-args", ($, env) => [$("li").wrap(function () { env.log.push(["w", arguments.length, typeof arguments[0]]); return "<div/>" }), env.doc.body.innerHTML])
c("wrapInner-fn-args", ($, env) => [$("li").wrapInner(function () { env.log.push(["wi", arguments.length, typeof arguments[0], this.id]); return "<b/>" }), env.doc.body.innerHTML])
c("wrapAll-fn-args", ($, env) => [$("li").wrapAll(function () { env.log.push(["wa", arguments.length, this.id]); return "<div class='wa'/>" }), env.doc.body.innerHTML])
c("proxy-args-this", ($) => { const f = function () { return [this === undefined ? "u" : typeof this, arguments.length] }; return [$.proxy(f, null)(), $.proxy(f)(1, 2), $.proxy(f, undefined, 1)(2)] })
c("clone-events-deep-noevents", ($, env) => { $("#p1").on("click", () => env.log.push("p")); const c1 = $("#p1").clone(true, false); c1.appendTo("#other").trigger("click"); return env.log.slice() })
c("detach-reinsert-events", ($, env) => { $("#li2").on("click", () => env.log.push("li2")).data("d", 7); const x = $("#li2").detach(); x.appendTo("#p1"); x.trigger("click"); return [x.data("d"), env.log.slice()] })
c("empty-remove-chain", ($, env) => [$("#list").empty().append("<li>n</li>").children().remove().end().length, env.doc.body.innerHTML])
c("append-to-doc-frag", ($, env) => { const f = env.doc.createDocumentFragment(); $(f).append("<b>1</b>", "<i>2</i>"); return [f.childNodes.length, $(f).children()] })
c("prepend-table-tr", ($, env) => [$("#tbl").prepend("<tr><td>p</td></tr>"), $("#tbl2").append($("<tr><td>q</td></tr>")), env.doc.body.innerHTML])
c("after-detached", ($, env) => { const d = $("<div>"); return [d.after("<p>").length, d.before("<p>").length, d.replaceWith("<p>").length] })
c("insert-multi-target-order", ($, env) => { const r = $("<b class='m'>1</b><i class='m'>2</i>").appendTo("li"); return [r.length, r.map(function () { return this.parentNode.id + this.nodeName }).get()] })
c("appendTo-string-html-target", ($, env) => { const r = $("<b>z</b>").appendTo($("<div>")); return [r.length, r.parent()] })

// ---- support-flag branches ----
function withSupport($, key, val, fn) { const old = $.support[key]; $.support[key] = val; try { return fn() } finally { $.support[key] = old } }
c("support-keys", ($) => ["checkClone", "noCloneChecked", "option", "checkOn", "optSelected", "radioValue"].map((k) => [k, $.support[k]]))
c("checkClone-false-multi", ($, env) => withSupport($, "checkClone", false, () => { const r = $("li").append("<input type='checkbox' checked='checked'>", "<b>x</b>"); return [r, env.doc.body.innerHTML, $("li input").map(function () { return this.checked }).get()] }))
c("checkClone-false-fn", ($, env) => withSupport($, "checkClone", false, () => [$("li").prepend(rec(env, "cc", (i, h) => "<i>" + i + "</i>")), env.doc.body.innerHTML]))
c("noCloneChecked-false", ($, env) => withSupport($, "noCloneChecked", false, () => { $("#c2").prop("checked", true); $("#c1").prop("checked", false); $("#ta").val("zz"); const cl = $("#f").clone(); return [cl.find("input").map(function () { return this.checked }).get(), cl.find("textarea")[0].defaultValue, cl.find("textarea")[0].value] }))
c("noCloneChecked-false-frag", ($, env) => withSupport($, "noCloneChecked", false, () => { const f = env.doc.createDocumentFragment(); const i = env.doc.createElement("input"); i.type = "checkbox"; f.appendChild(i); i.checked = true; const cl = $.clone(f); return [cl.firstChild.checked, $.clone(i).checked] }))
c("radioValue-false", ($, env) => withSupport($, "radioValue", false, () => { const i = $("<input value='keep'>"); const r = i.attr("type", "radio"); const j = $("<input>"); j.attr("type", "radio"); const k = $("<div>").attr("type", "radio"); return [r.attr("type"), i[0].value, j[0].value, k.attr("type"), $("#c1").attr("type", "radio").val()] }))
c("contents-iframe", ($, env) => { const f = env.doc.createElement("iframe"); env.doc.getElementById("empty").appendChild(f); const r = $(f).contents(); return [r.length, r[0] === f.contentDocument, $("#empty").contents().length] })
c("contents-object", ($, env) => { const o = env.doc.createElement("object"); o.appendChild(env.doc.createElement("param")); env.doc.getElementById("empty").appendChild(o); return $(o).contents() })
c("text-frag", ($, env) => { const f = env.doc.createDocumentFragment(); f.appendChild(env.doc.createElement("b")).textContent = "q"; return [$(f).text(), $(f).text("z").text(), $(env.doc).text("nope").length] })
c("closest-jq-context", ($, env) => [$("#b1").closest("div", $("#main")), $("#b1").closest("p", "#main")])
c("filter-jq-positional", ($) => [$("li").filter("li:first"), $("li").is("li:first"), $("li").is(":first"), $("#li2").is(":first")])
c("add-order", ($) => [$("#li4").add("#li1").add("#p1"), $("#li4").add("#li1").add("#p1").prevObject])
c("parents-multi-order", ($) => [$("b, li").parents(), $("li").prevAll(), $("li").nextAll(".odd"), $("li").siblings("#li1, #li4")])
c("next-prev-text", ($) => [$("#main").contents().next(), $("#main").contents().prev("p")])
// ---- runner ----
function runAll($, dom) {
  const win = dom.window, doc = win.document
  const results = {}
  for (const { name, fn } of cases) {
    doc.head.innerHTML = ""
    doc.body.innerHTML = FIXTURE
    win.__log = []
    const env = { $, win, doc, log: [] }
    let value
    try { value = ser(fn($, env), env) } catch (e) { value = { $throw: (e && e.name) || String(e) } }
    let after
    try { after = doc.body.innerHTML } catch (e) { after = "?" }
    results[name] = JSON.stringify([value, env.log, after, win.__log])
  }
  return results
}
const names = new Set()
for (const k of cases) { if (names.has(k.name)) throw new Error("dup case " + k.name); names.add(k.name) }
const rO = runAll($O, domO)
const rL = runAll($L, domL)
const mism = []
for (const { name } of cases) if (rO[name] !== rL[name]) mism.push(name)
console.log(`cases ${cases.length} mismatches ${mism.length}`)
if (OUT) writeFileSync(OUT, JSON.stringify({ cases: cases.length, mismatches: mism, official: Object.fromEntries(mism.map((n) => [n, rO[n]])), lil: Object.fromEntries(mism.map((n) => [n, rL[n]])), all: rL }, null, 1))
if (process.env.SHOW) for (const n of mism.slice(0, +process.env.SHOW)) console.log("--", n, "\n  O:", rO[n].slice(0, 600), "\n  L:", rL[n].slice(0, 600))
process.exit(0)
