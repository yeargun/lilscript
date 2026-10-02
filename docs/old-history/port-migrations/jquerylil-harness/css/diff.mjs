// Differential harness for the css group (css.lil, css/**, dimensions.lil, offset.lil).
// usage: node diff.mjs <our dist/jquery.esm.js> [out.json]
// Runs every case on official jQuery 3.7.1 (jsdom window A) and on our build (jsdom window B),
// with identical fixtures and identical layout stubs, and compares serialized results + DOM.
import { createRequire } from "node:module"
import { pathToFileURL } from "node:url"
import { writeFileSync } from "node:fs"
import { resolve } from "node:path"

const req = createRequire("/home/azureuser/jquerylil/package.json")
const { JSDOM, VirtualConsole } = req("jsdom")
const officialFactory = req("jquery")

const distPath = resolve(process.argv[2])
const outPath = process.argv[3]

const STYLE = `
.box{width:100px;height:50px;padding:5px 6px;border:2px solid black;margin:3px 4px}
.bbox{box-sizing:border-box}
.hid{display:none}
.abs{position:absolute;top:10px;left:20px;width:30px;height:40px}
.pct{position:absolute;top:10%;left:5em;width:50%;margin-left:2em}
.fix{position:fixed;top:7px;left:8px}
.wrap{position:relative}
.em{font-size:10px;width:3em;height:2em;margin-left:1em}
li.x{display:none}
.nw{letter-spacing:normal;font-weight:normal}
`
const BODY = `<div id="box" class="box">box</div><div id="bb" class="box bbox">bb</div><div id="hid" class="box hid">hid</div><div id="wrap" class="wrap"><span id="sp">span</span><p id="p1" style="display:none">p1</p><div id="inner" class="box">inner<b id="deep">d</b></div></div><table id="tbl"><tbody><tr id="tr1"><td id="td1">td</td></tr></tbody></table><ul id="ul"><li id="li1">a</li><li id="li2" style="display:none">b</li><li id="li3" class="x">c</li></ul><div id="abs" class="abs">abs</div><div id="pct" class="pct">pct</div><div id="fix" class="fix">fix</div><div id="em" class="em">em</div><div id="nw" class="nw">nw</div><input id="inp" type="text" value="x"><span id="sp2" style="display:inline">s2</span>`
const HTML = `<!doctype html><html><head><style>${STYLE}</style></head><body>${BODY}</body></html>`

function makeWindow() {
  const vc = new VirtualConsole()
  const dom = new JSDOM(HTML, { pretendToBeVisual: true, virtualConsole: vc })
  return dom.window
}

const winA = makeWindow()
const official = officialFactory(winA)
const winB = makeWindow()
globalThis.window = winB
globalThis.document = winB.document
const lil = (await import(pathToFileURL(distPath).href + "?t=" + Date.now())).jQuery

// ---- serialization -------------------------------------------------------------------------
function nodeDesc(n) {
  if (!n) return String(n)
  if (n.nodeType === 9) return "[document]"
  if (n.nodeType === 3) return "#text:" + n.nodeValue
  if (n.nodeType === 8) return "#comment:" + n.nodeValue
  if (n.nodeType === 11) return "#fragment"
  let s = n.nodeName.toLowerCase()
  if (n.id) s += "#" + n.id
  const st = n.getAttribute && n.getAttribute("style")
  if (st != null) s += "[style=" + st + "]"
  return s
}
function ser(v, win, depth = 0) {
  if (v === undefined) return "<undefined>"
  if (v === null) return null
  const t = typeof v
  if (t === "number") {
    if (Number.isNaN(v)) return "<NaN>"
    if (Object.is(v, -0)) return "<-0>"
    if (!Number.isFinite(v)) return "<" + v + ">"
    return v
  }
  if (t === "string" || t === "boolean") return v
  if (t === "function") return "<function>"
  if (t === "symbol") return "<symbol>"
  if (v === win) return "<window>"
  if (v.nodeType) return "<node " + nodeDesc(v) + ">"
  if (depth > 4) return "<deep>"
  if (v.jquery) {
    const a = []
    for (let i = 0; i < v.length; i++) a.push(ser(v[i], win, depth + 1))
    return { jq: a }
  }
  if (Array.isArray(v)) return v.map((x) => ser(x, win, depth + 1))
  if (v instanceof win.Error || v instanceof Error) return { error: v.name }
  const o = {}
  for (const k of Object.keys(v).sort()) o[k] = ser(v[k], win, depth + 1)
  return o
}

// ---- layout stubs (identical on both windows) ---------------------------------------------
function stub(el, o) {
  const def = (k, v) => Object.defineProperty(el, k, { configurable: true, get: () => v })
  if ("ow" in o) def("offsetWidth", o.ow)
  if ("oh" in o) def("offsetHeight", o.oh)
  if ("op" in o) def("offsetParent", o.op)
  if ("sw" in o) def("scrollWidth", o.sw)
  if ("sh" in o) def("scrollHeight", o.sh)
  if ("cw" in o) def("clientWidth", o.cw)
  if ("ch" in o) def("clientHeight", o.ch)
  const rects = o.rects ?? 1
  el.getClientRects = () => (rects ? [{ top: o.top || 0 }] : [])
  el.getBoundingClientRect = () => ({ top: o.top || 0, left: o.left || 0, width: o.w ?? o.ow ?? 0, height: o.h ?? o.oh ?? 0, bottom: 0, right: 0 })
}

// ---- cases ----------------------------------------------------------------------------------
const cases = []
const C = (name, run) => cases.push({ name, run })
const g = (doc, id) => doc.getElementById(id)

// support
C("support flags", ($) => [$.support.clearCloneStyle, $.support.boxSizingReliable(), $.support.pixelBoxStyles(), $.support.pixelPosition(), $.support.reliableMarginLeft(), $.support.scrollboxSize(), $.support.reliableTrDimensions(), typeof $.support.reliableTrDimensions])
C("cssNumber/cssProps/cssHooks keys", ($) => [Object.keys($.cssNumber).sort(), $.cssNumber.opacity, $.cssNumber.width, Object.keys($.cssProps), Object.keys($.cssHooks).sort(), Object.keys($.cssHooks.width).sort(), Object.keys($.cssHooks.margin).sort(), Object.keys($.cssHooks.padding).sort(), Object.keys($.cssHooks.borderWidth).sort(), Object.keys($.cssHooks.top).sort(), Object.keys($.cssHooks.opacity).sort(), typeof $.offset.setOffset])

// .css getters
for (const prop of ["width", "height", "color", "paddingLeft", "padding-left", "borderTopWidth", "marginLeft", "margin-right", "display", "position", "top", "left", "opacity", "zIndex", "fontWeight", "letterSpacing", "boxSizing", "float", "cssFloat", "--foo", "nonexistentProp", "backgroundColor"]) {
  for (const id of ["box", "bb", "hid", "abs", "pct", "fix", "em", "nw", "sp", "tr1", "inner"]) {
    C(`css get ${prop} #${id}`, ($, win, doc) => [$("#" + id).css(prop), $.css(g(doc, id), prop), $.css(g(doc, id), prop, true), $.css(g(doc, id), prop, ""), $.css(g(doc, id), prop, false)])
  }
}
C("css get on empty set", ($) => [$([]).css("width"), $("#nope").css("color"), $().css(["width"])])
C("css get array", ($) => $("#box").css(["width", "height", "color", "padding-left", "marginTop"]))
C("css get array empty", ($) => $("#box").css([]))
C("css get on text node", ($, win, doc) => { const t = doc.createTextNode("x"); return [$(t).css("color"), $.style(t, "color"), $.style(t, "color", "red")] })
C("css get on comment", ($, win, doc) => { const t = doc.createComment("x"); return [$.style(t, "color"), $.style(t, "color", "red")] })
C("css get on window", ($, win) => { try { return $(win).css("width") } catch (e) { return { threw: e.name } } })
C("css get detached", ($, win, doc) => { const d = doc.createElement("div"); d.style.width = "12px"; d.style.color = "red"; return [$(d).css("width"), $(d).css("color"), $(d).css("display"), $(d).css("height")] })
C("css get detached empty", ($, win, doc) => { const d = doc.createElement("div"); return [$(d).css("width"), $(d).css("margin-left"), $(d).css("opacity")] })

// .css setters
const setVals = [10, 0, -5, 1.5, "20px", "3em", "", null, undefined, NaN, "+=10", "-=5", "+=2em", "-=1.5px", "+=0", "auto", "50%", " 12px ", "10px !important", true, "calc(10px + 1em)", "+=5%"]
for (const prop of ["width", "height", "opacity", "zIndex", "paddingLeft", "padding", "margin", "marginTop", "borderWidth", "top", "left", "lineHeight", "backgroundColor", "background-image", "--custom", "fillOpacity", "gridArea", "fontWeight"]) {
  for (const v of setVals) {
    C(`css set ${prop}=${String(v)}`, ($, win, doc) => {
      const r = $("#box").css(prop, v)
      return [r && r.jquery ? "chain" : ser(r, win), $("#box").attr("style"), $("#box").css(prop)]
    })
  }
}
C("css set on multiple, fn", ($) => { const log = []; const r = $("#box, #bb, #inner").css("width", function (i, v) { log.push([i, v, this.id]); return i * 10 + 5 }); return [r.length, log, $("#box").attr("style"), $("#bb").attr("style"), $("#inner").attr("style")] })
C("css set fn returns undefined", ($) => { const r = $("#box").css("width", function () { }); return [$("#box").attr("style"), r.length] })
C("css set fn returns string rel", ($) => { $("#box").css("width", () => "+=7"); return $("#box").attr("style") })
C("css set object", ($) => { const r = $("#box").css({ width: 10, height: "20px", color: "red", opacity: 0.3, "margin-left": 7, "--v": " 1 " }); return [r.length, $("#box").attr("style")] })
C("css set object with fn", ($) => { $("#box, #bb").css({ width: (i, v) => i + v, height: (i) => i * 3 }); return [$("#box").attr("style"), $("#bb").attr("style")] })
C("css set undefined value chain", ($, win) => { const r = $("#box").css("width", undefined); return [r && r.jquery ? r.length : ser(r, win), $("#box").attr("style")] })
C("css set empty set", ($) => { const r = $([]).css("width", 5); return [r.length, r.jquery] })
C("css set on text node set", ($, win, doc) => { const t = doc.createTextNode("x"); const r = $(t).css("color", "red"); return r.length })
C("css custom prop get trim", ($) => { $("#box")[0].style.setProperty("--foo", "  bar  "); return [$("#box").css("--foo"), $("#box").css("--missing"), $.css($("#box")[0], "--foo")] })
C("css custom prop empty", ($) => { $("#box")[0].style.setProperty("--e", "   "); return [$("#box").css("--e")] })
C("css custom prop set number", ($) => { $("#box").css("--n", 5); return [$("#box").attr("style"), $("#box").css("--n")] })
C("cssProps mapping", ($) => { $.cssProps.myColor = "color"; $("#box").css("myColor", "blue"); const r = [$("#box").attr("style"), $("#box").css("myColor")]; delete $.cssProps.myColor; return r })
C("cssProps empty string", ($) => { $.cssProps.foo = ""; const r = $("#box").css("foo"); delete $.cssProps.foo; return r })
C("vendor prefixed names", ($) => ["userSelect", "appearance", "textSizeAdjust", "transform", "transition", "boxFlex", "cssFloat", "notAThing", "WebkitTransform", "MozFoo"].map((p) => { $("#box").css(p, "none"); return [p, $("#box").css(p), $("#box").attr("style")] }))
C("vendor cache consistency", ($) => [$("#box").css("userSelect"), $("#box").css("userSelect"), $("#bb").css("userSelect")])
C("custom hooks args", ($) => {
  const log = []
  $.cssHooks.fooBar = { get(e, c, x) { log.push(["get", e.id, c, x === undefined ? "U" : x === null ? "N" : x]); return "G" }, set(e, v, x) { log.push(["set", e.id, v, x === undefined ? "U" : x === null ? "N" : x]); return v === "skip" ? undefined : v + "!" } }
  const r = [$("#box").css("fooBar"), $.css($("#box")[0], "fooBar"), $.css($("#box")[0], "fooBar", "border"), $.style($("#box")[0], "fooBar"), $.style($("#box")[0], "fooBar", "v1"), $.style($("#box")[0], "fooBar", "skip", "x"), $("#box").css("fooBar", "v2").length, $("#box").css("fooBar", 12).length]
  delete $.cssHooks.fooBar
  return [r, log, $("#box").attr("style")]
})
C("custom hook get returns undefined", ($) => { $.cssHooks.color = { get() { return undefined } }; const r = [$("#box").css("color"), $.style($("#box")[0], "color")]; delete $.cssHooks.color; return r })
C("custom hook set only", ($) => { $.cssHooks.width2 = { set: (e, v) => v }; const r = [$("#box").css("width2"), $.style($("#box")[0], "width2")]; delete $.cssHooks.width2; return r })
C("custom hook on final name", ($) => { const log = []; $.cssHooks["--q"] = { get: (e, c) => (log.push(c), "Q") }; const r = [$("#box").css("--q")]; delete $.cssHooks["--q"]; return [r, log] })
C("hook non-object truthy", ($) => { $.cssHooks.zz = "str"; let r; try { r = [$("#box").css("zz"), $.style($("#box")[0], "zz", "1")] } catch (e) { r = { threw: e.name } }; delete $.cssHooks.zz; return r })
C("jQuery.style direct", ($, win, doc) => { const e = g(doc, "box"); return [$.style(e, "width"), $.style(e, "width", 15), e.getAttribute("style"), $.style(e, "width", "+=5"), e.getAttribute("style"), $.style(e, "opacity", "0.5"), $.style(e, "opacity"), $.style(null, "width"), $.style(undefined, "width", 1), $.style({}, "width", 1), $.style({ nodeType: 1 }, "width")] })
C("jQuery.style numeric px", ($, win, doc) => { const e = g(doc, "box"); return ["width", "zIndex", "lineHeight", "flexGrow", "order", "aspectRatio", "scale", "strokeOpacity", "top", "fontSize", "columnCount", "zoom"].map((p) => { $.style(e, p, 2); return [p, $.style(e, p)] }) })
C("jQuery.style string rel with unit", ($, win, doc) => { const e = g(doc, "em"); return [$.style(e, "width", "+=1em"), e.getAttribute("style"), $.style(e, "marginLeft", "-=2px"), e.getAttribute("style"), $.style(e, "zIndex", "+=2"), e.getAttribute("style"), $.style(e, "opacity", "-=0.25"), e.getAttribute("style")] })
C("jQuery.css direct numeric", ($, win, doc) => { const e = g(doc, "box"); return [$.css(e, "width", true), $.css(e, "color", true), $.css(e, "color", ""), $.css(e, "width", "padding"), $.css(e, "width", "border"), $.css(e, "width", "margin"), $.css(e, "width", "content"), $.css(e, "opacity", ""), $.css(e, "zIndex", ""), $.css(e, "zIndex", true)] })
C("jQuery.css with styles arg", ($, win, doc) => { const e = g(doc, "box"); const s = win.getComputedStyle(e); return [$.css(e, "width", false, s), $.css(e, "paddingTop", true, s), $.css(e, "color", undefined, s)] })
C("css normal transform", ($, win, doc) => { const e = g(doc, "nw"); return [$.css(e, "letterSpacing"), $.css(e, "fontWeight"), $.css(e, "letterSpacing", true), $.css(e, "fontWeight", "")] })
C("clearCloneStyle background", ($) => { $("#box").css("backgroundColor", "red"); const c = $("#box").clone(); c.css("backgroundColor", ""); return [$("#box").attr("style"), c.attr("style"), $("#box").css("background-image", "").attr("style")] })

// cssHooks direct
C("expand hooks", ($) => [$.cssHooks.margin.expand("1px 2px 3px 4px"), $.cssHooks.margin.expand("1px"), $.cssHooks.padding.expand("1px 2px"), $.cssHooks.borderWidth.expand("1px 2px 3px"), $.cssHooks.padding.expand(5), $.cssHooks.margin.expand(0), $.cssHooks.margin.expand(""), $.cssHooks.margin.expand("0 5px"), $.cssHooks.margin.expand("a  b"), $.cssHooks.margin.expand(null), $.cssHooks.margin.expand(undefined), $.cssHooks.margin.expand(["1", "2"]), $.cssHooks.margin.expand(" "), typeof $.cssHooks.margin.set, typeof $.cssHooks.padding.set, typeof $.cssHooks.borderWidth.set])
C("expand hooks set", ($, win, doc) => { const e = g(doc, "box"); const s = $.cssHooks.padding.set; return [s(e, "10px"), s(e, "-10px"), s(e, "5em", 2), s(e, "abc"), s(e, 7), s(e, 3, 5), s(e, "3px", "1"), s(e, "", 0), s(e, "+=4")] })
C("opacity hook", ($, win, doc) => [$.cssHooks.opacity.get(g(doc, "box"), true), $.cssHooks.opacity.get(g(doc, "box"), false), $("#box").css("opacity"), $("#box").css("opacity", 0.4).css("opacity")])
C("marginLeft hook", ($, win, doc) => { const e = g(doc, "pct"); stub(e, { ow: 50, left: 30 }); return [$("#pct").css("marginLeft"), $("#box").css("marginLeft"), $.cssHooks.marginLeft.get(e, true), $.cssHooks.marginLeft.get(g(doc, "box"), false)] })
C("marginLeft hook zero", ($, win, doc) => { const e = g(doc, "sp"); stub(e, { left: 13 }); return [$("#sp").css("marginLeft"), $("#sp").css("margin-left")] })
C("width hook direct", ($, win, doc) => { const h = $.cssHooks.width; const e = g(doc, "box"); return [h.get(e, false), h.get(e, true), h.get(e, true, "padding"), h.get(e, true, "border"), h.get(e, true, "margin"), h.get(e, true, true), h.set(e, "10px"), h.set(e, 10), h.set(e, "10px", "padding"), h.set(e, "10em", "border"), h.set(e, "-100px", "margin"), h.set(e, 5, "content"), e.getAttribute("style")] })
C("height hook hidden swap", ($, win, doc) => { const e = g(doc, "hid"); const r = [$.cssHooks.height.get(e, true), e.getAttribute("style"), $("#hid").css("height"), $("#hid").height(), $("#hid").outerHeight(true), e.getAttribute("style")]; return r })
C("height hook table display swap", ($, win, doc) => { const e = g(doc, "td1"); stub(e, { rects: 1, w: 0 }); const r = [$.cssHooks.height.get(e, true), $("#td1").height(), $("#td1").css("display")]; return r })

// dimensions
const dimFns = ["width", "height", "innerWidth", "innerHeight", "outerWidth", "outerHeight"]
for (const id of ["box", "bb", "hid", "abs", "pct", "em", "sp", "tr1", "td1", "inner", "inp", "p1"]) {
  C(`dims get #${id}`, ($) => dimFns.map((f) => [f, $("#" + id)[f](), $("#" + id)[f](true), $("#" + id)[f](false)]))
}
C("dims get stubbed", ($, win, doc) => { for (const id of ["box", "bb", "hid", "tr1"]) stub(g(doc, id), { ow: 120, oh: 70, w: 120, h: 70 }); return ["box", "bb", "hid", "tr1"].map((id) => dimFns.map((f) => [f, $("#" + id)[f](), $("#" + id)[f](true)])) })
C("dims get stubbed no rects", ($, win, doc) => { for (const id of ["box", "bb"]) stub(g(doc, id), { ow: 120, oh: 70, rects: 0 }); return ["box", "bb"].map((id) => dimFns.map((f) => [f, $("#" + id)[f]()])) })
const dimSetVals = [10, "20px", "3em", 0, -5, "50%", "", null, "auto", "+=10", "-=3px"]
for (const f of dimFns) {
  for (const v of dimSetVals) {
    for (const id of ["box", "bb", "abs"]) {
      C(`dims set ${f}(${String(v)}) #${id}`, ($, win) => { const r = $("#" + id)[f](v); return [r && r.jquery ? "chain" : ser(r, win), $("#" + id).attr("style"), $("#" + id)[f]()] })
    }
  }
  C(`dims set ${f} margin-bool`, ($, win) => { const a = $("#box")[f](30, true); const s1 = $("#box").attr("style"); const b = $("#bb")[f](30, false); return [a && a.jquery ? "chain" : ser(a, win), s1, b && b.jquery ? "chain" : ser(b, win), $("#bb").attr("style")] })
  C(`dims set ${f} fn`, ($) => { const log = []; $("#box, #bb")[f](function (i, v) { log.push([i, v, this.id]); return v + 1 }); return [log, $("#box").attr("style"), $("#bb").attr("style")] })
  C(`dims ${f} undefined arg`, ($, win) => { const r = $("#box")[f](undefined); return r && r.jquery ? ["chain", r.length] : ser(r, win) })
  C(`dims ${f} bool only`, ($, win) => [ser($("#box")[f](true), win), ser($("#box")[f](false), win)])
  C(`dims ${f} empty`, ($, win) => [ser($([])[f](), win), $([])[f](5).length])
  C(`dims ${f} window`, ($, win) => { stub(win.document.documentElement, { cw: 800, ch: 600 }); return [$(win)[f](), $(win)[f](true), $(win)[f](10) && "set"] })
  C(`dims ${f} document`, ($, win, doc) => { stub(doc.documentElement, { cw: 800, ch: 600, sw: 900, sh: 1200, ow: 810, oh: 610 }); stub(doc.body, { sw: 950, sh: 1100, ow: 700, oh: 1300 }); return [$(doc)[f](), $(doc)[f](true)] })
  C(`dims ${f} document zero`, ($, win, doc) => [$(doc)[f](), $(doc)[f](false)])
}
C("dims scrollbox abs border-box", ($, win, doc) => { const e = g(doc, "abs"); e.style.boxSizing = "border-box"; stub(e, { ow: 44, oh: 55 }); const r = []; for (const f of dimFns) { $("#abs")[f](25); r.push([f, e.getAttribute("style")]) } return r })
C("dims inline zero", ($, win, doc) => { const e = g(doc, "sp2"); stub(e, { ow: 33, oh: 11 }); return dimFns.map((f) => $("#sp2")[f]()) })

// offset / position / offsetParent / scroll
C("offset get no rects", ($) => [$("#box").offset(), $("#abs").offset(), $([]).offset(), $("#nope").position()])
C("offset get stubbed", ($, win, doc) => { stub(g(doc, "abs"), { top: 15.5, left: 22, ow: 30, oh: 40 }); return [$("#abs").offset(), $("#abs").position()] })
C("offset get with page offset", ($, win, doc) => { Object.defineProperty(win, "pageYOffset", { configurable: true, get: () => 100 }); Object.defineProperty(win, "pageXOffset", { configurable: true, get: () => 7 }); stub(g(doc, "abs"), { top: 5, left: 6 }); const r = [$("#abs").offset()]; delete win.pageYOffset; delete win.pageXOffset; return r })
C("offset set object", ($, win, doc) => { const r = $("#box").offset({ top: 10, left: 20 }); return [r.length, $("#box").attr("style")] })
C("offset set partial", ($) => { $("#abs").offset({ top: 5 }); const a = $("#abs").attr("style"); $("#pct").offset({ left: "7" }); return [a, $("#pct").attr("style")] })
C("offset set null fields", ($) => { $("#abs").offset({ top: null, left: undefined }); return $("#abs").attr("style") })
C("offset set stubbed", ($, win, doc) => { stub(g(doc, "abs"), { top: 30, left: 40 }); $("#abs").offset({ top: 100, left: 200 }); return $("#abs").attr("style") })
C("offset set fixed/auto", ($, win, doc) => { const e = g(doc, "fix"); e.style.top = "auto"; stub(e, { top: 3, left: 4 }); $("#fix").offset({ top: 50, left: 60 }); return $("#fix").attr("style") })
C("offset set fn", ($, win, doc) => { const log = []; stub(g(doc, "box"), { top: 3, left: 4 }); $("#box, #abs").offset(function (i, c) { log.push([i, c, this.id]); return { top: i * 10, left: 5 } }); return [log, $("#box").attr("style"), $("#abs").attr("style")] })
C("offset set fn returns coords mutation", ($, win, doc) => { const log = []; $("#abs").offset(function (i, c) { c.top = 99; log.push(c); return c }); return [log, $("#abs").attr("style")] })
C("offset set using", ($, win, doc) => { const log = []; $("#abs").offset({ top: 1, left: 2, using(p) { log.push([this.id, p, Object.keys(p)]) } }); $("#box").offset({ top: 3, using(p) { log.push([this.id, p, Object.keys(p)]) } }); return [log, $("#abs").attr("style"), $("#box").attr("style")] })
C("offset set undefined", ($) => { const r = $("#box").offset(undefined); return [r.jquery, r.length] })
C("offset set string", ($) => { let r; try { r = $("#box").offset("x"); r = [r.length, $("#box").attr("style")] } catch (e) { r = { threw: e.name } } return r })
C("offset set on empty", ($) => { const r = $([]).offset({ top: 1 }); return [r.length] })
C("offset each index", ($, win, doc) => { $("#box, #bb, #abs").offset({ top: 11, left: 12 }); return [$("#box").attr("style"), $("#bb").attr("style"), $("#abs").attr("style")] })
C("setOffset direct", ($, win, doc) => { const e = g(doc, "abs"); $.offset.setOffset(e, { top: 9, left: 8 }, 0); const a = e.getAttribute("style"); $.offset.setOffset(e, (i, c) => ({ top: i + 1 }), 3); return [a, e.getAttribute("style")] })
C("offset window/document", ($, win, doc) => { let r = []; for (const x of [win, doc]) { try { r.push(ser($(x).offset(), win)) } catch (e) { r.push({ threw: e.name }) } } return r })
C("position static", ($, win, doc) => { stub(g(doc, "box"), { top: 40, left: 50 }); stub(doc.body, { top: 0, left: 0 }); return [$("#box").position()] })
C("position fixed", ($, win, doc) => { stub(g(doc, "fix"), { top: 7, left: 8 }); return [$("#fix").position()] })
C("position in relative parent", ($, win, doc) => { const w = g(doc, "wrap"); const i = g(doc, "inner"); stub(w, { top: 100, left: 10 }); stub(i, { top: 130, left: 25, op: w }); return [$("#inner").position(), $("#inner").offsetParent().map((_, e) => e.id).get()] })
C("position offsetParent body static", ($, win, doc) => { const i = g(doc, "box"); stub(i, { top: 30, left: 25, op: doc.body }); stub(doc.body, { top: 1, left: 1 }); return [$("#box").position()] })
C("position offsetParent body relative", ($, win, doc) => { doc.body.style.position = "relative"; doc.body.style.borderTopWidth = "3px"; const i = g(doc, "box"); stub(i, { top: 30, left: 25, op: doc.body }); stub(doc.body, { top: 1, left: 1 }); const r = [$("#box").position()]; doc.body.removeAttribute("style"); return r })
C("position offsetParent is elem", ($, win, doc) => { const i = g(doc, "box"); stub(i, { top: 30, left: 25, op: i }); return [$("#box").position()] })
C("position no offsetParent", ($, win, doc) => { const i = g(doc, "box"); stub(i, { top: 30, left: 25, op: null }); return [$("#box").position()] })
C("position margins", ($, win, doc) => { const i = g(doc, "pct"); stub(i, { top: 30, left: 25, op: null }); return [$("#pct").position(), $("#em").position()] })
C("offsetParent", ($, win, doc) => { const w = g(doc, "wrap"); stub(g(doc, "inner"), { op: w }); stub(g(doc, "deep"), { op: g(doc, "inner") }); stub(g(doc, "sp"), { op: doc.body }); const f = (sel) => $(sel).offsetParent().map((_, e) => e.id || e.nodeName).get(); return [f("#inner"), f("#deep"), f("#sp"), f("#box"), f([]), f("#inner, #deep, #box")] })
C("offsetParent static chain", ($, win, doc) => { const w = g(doc, "wrap"); w.style.position = "static"; stub(w, { op: doc.body }); stub(g(doc, "inner"), { op: w }); return $("#inner").offsetParent().map((_, e) => e.id || e.nodeName).get() })
C("scrollTop/Left elements", ($, win, doc) => { const e = g(doc, "box"); const r = [$("#box").scrollTop(), $("#box").scrollLeft(), $("#box").scrollTop(15).length, e.scrollTop, $("#box").scrollLeft("7").length, e.scrollLeft, $([]).scrollTop(), $([]).scrollTop(3).length, $("#box").scrollTop(undefined).jquery]; return r })
C("scrollTop fn", ($) => { const log = []; $("#box, #bb").scrollTop(function (i, v) { log.push([i, v]); return i + 1 }); return log })
C("scroll window", ($, win) => { const log = []; win.scrollTo = (x, y) => log.push([x, y]); Object.defineProperty(win, "pageXOffset", { configurable: true, get: () => 3 }); Object.defineProperty(win, "pageYOffset", { configurable: true, get: () => 4 }); const r = [$(win).scrollTop(), $(win).scrollLeft(), $(win).scrollTop(40).length, $(win).scrollLeft("9").length]; delete win.pageXOffset; delete win.pageYOffset; delete win.scrollTo; return [r, log] })
C("scroll document", ($, win, doc) => { const log = []; win.scrollTo = (x, y) => log.push([x, y]); const r = [$(doc).scrollTop(), $(doc).scrollLeft(), $(doc).scrollTop(40).length]; delete win.scrollTo; return [r, log] })
C("top/left hooks", ($, win, doc) => { stub(g(doc, "pct"), { top: 22, left: 33, op: null }); return [$("#pct").css("top"), $("#pct").css("left"), $("#abs").css("top"), $("#box").css("top"), $.cssHooks.top.get(g(doc, "pct"), false), $.cssHooks.left.get(g(doc, "abs"), true)] })

// show / hide / toggle
const vis = ($, doc) => ["box", "hid", "sp", "p1", "li1", "li2", "li3", "tr1", "td1", "tbl", "ul", "inp", "sp2"].map((id) => [id, g(doc, id).getAttribute("style")])
C("hide all", ($, win, doc) => { const r = $("#box, #hid, #sp, #p1, #li1, #li2, #li3, #tr1, #tbl, #inp, #sp2").hide(); return [r.length, vis($, doc)] })
C("show all", ($, win, doc) => { const r = $("#box, #hid, #sp, #p1, #li1, #li2, #li3, #tr1, #tbl, #inp, #sp2").show(); return [r.length, vis($, doc)] })
C("hide then show", ($, win, doc) => { $("#box, #sp, #li1, #tr1, #sp2").hide().show(); return vis($, doc) })
C("hide then show custom display", ($, win, doc) => { $("#box").css("display", "flex"); $("#box").hide(); const a = g(doc, "box").getAttribute("style"); $("#box").show(); return [a, g(doc, "box").getAttribute("style")] })
C("show twice / hide twice", ($, win, doc) => { $("#p1").show().show(); $("#li1").hide().hide().show(); return vis($, doc) })
C("toggle", ($, win, doc) => { const r = [$("#box, #hid, #p1, #li3").toggle().length]; const a = vis($, doc); $("#box, #hid, #p1, #li3").toggle(); return [r, a, vis($, doc)] })
C("toggle bool", ($, win, doc) => { $("#box").toggle(false); const a = vis($, doc); $("#box, #p1").toggle(true); return [a, vis($, doc)] })
C("toggle non-bool args", ($, win, doc) => { $("#box").toggle(0); $("#box").stop(true, true); const a = vis($, doc); $("#p1").toggle("x"); $("#p1").stop(true, true); return [a, vis($, doc)] })
C("show detached", ($, win, doc) => { const d = doc.createElement("div"); d.style.display = "none"; $(d).show(); const s = doc.createElement("span"); $(s).hide(); const s2 = $(s).show(); return [d.getAttribute("style"), s.getAttribute("style"), s2.length] })
C("show text node in set", ($, win, doc) => { const t = doc.createTextNode("x"); const r = $([t, g(doc, "p1")]).show(); return [r.length, vis($, doc)] })
C("show new tags default display", ($, win, doc) => { const out = []; for (const tag of ["div", "span", "li", "table", "tr", "td", "section", "custom-el", "a", "img"]) { const e = doc.createElement(tag); e.className = "hid"; doc.body.appendChild(e); $(e).show(); out.push([tag, e.getAttribute("style")]) } return out })
C("show hid via stylesheet li.x", ($, win, doc) => { $("#li3").show(); return vis($, doc) })
C("hidden visible selectors", ($, win, doc) => { stub(g(doc, "box"), { ow: 5, oh: 5 }); stub(g(doc, "sp"), { ow: 0, oh: 3 }); stub(g(doc, "li1"), { ow: 0, oh: 0, rects: 1 }); stub(g(doc, "li2"), { ow: 0, oh: 0, rects: 0 }); return [$("#box").is(":visible"), $("#box").is(":hidden"), $("#sp").is(":visible"), $("#li1").is(":visible"), $("#li2").is(":hidden"), $("#hid").is(":hidden"), $("div:visible").map((_, e) => e.id).get(), $("li:hidden").map((_, e) => e.id).get(), $("#ul").children(":visible").length, typeof $.expr.pseudos.hidden, typeof $.expr.pseudos.visible] })
C("hidden pseudo fns direct", ($, win, doc) => { stub(g(doc, "box"), { ow: 1 }); return [$.expr.pseudos.visible(g(doc, "box")), $.expr.pseudos.hidden(g(doc, "box")), $.expr.pseudos.visible(g(doc, "hid")), $.expr.pseudos.hidden(g(doc, "hid"))] })
C("filters :hidden detached", ($, win, doc) => { const d = doc.createElement("div"); return [$(d).is(":hidden"), $(d).is(":visible")] })

// animate (adjustCSS via tweens)
const animCases = [{ width: "+=10" }, { width: "-=10px" }, { width: 30 }, { width: "2em" }, { opacity: 0.5 }, { opacity: "-=0.5" }, { left: "50%" }, { marginLeft: "+=3em" }, { height: "toggle" }, { padding: 10 }, { margin: "5px" }, { zIndex: "+=3" }, { width: "+=1.5em" }, { lineHeight: 2 }, { fontSize: "+=2" }, { borderWidth: "3px" }, { width: "+=5%" }, { top: "-=4" }]
for (const props of animCases) {
  for (const id of ["box", "em", "abs", "hid"]) {
    C(`animate ${JSON.stringify(props)} #${id}`, ($, win, doc) => { const log = []; $("#" + id).animate(props, { duration: 100, step(now, fx) { log.push([fx.prop, fx.unit, typeof fx.start, typeof fx.end, Math.round(fx.start * 1000) / 1000, Math.round(fx.end * 1000) / 1000]) } }); $("#" + id).stop(true, true); return [log, g(doc, id).getAttribute("style")] })
  }
}
C("fx tween start/end", ($, win, doc) => { const out = []; $("#em").animate({ width: "+=2em", height: "10px", marginLeft: "-=1", opacity: 0.3 }, { duration: 1000, start(anim) { for (const t of anim.tweens) out.push([t.prop, t.unit, Math.round(t.start * 100) / 100, Math.round(t.end * 100) / 100]) } }); $("#em").stop(true, true); return [out, g(doc, "em").getAttribute("style")] })
C("slide/fade toggles", ($, win, doc) => { $("#box").slideUp(0); $("#li1").fadeOut(0); $("#p1").slideDown(0); $("#box").stop(true, true); $("#li1").stop(true, true); $("#p1").stop(true, true); return vis($, doc) })

// swap directly observed through hidden dims
C("swap restores style", ($, win, doc) => { const e = g(doc, "hid"); e.style.position = "static"; e.style.visibility = "visible"; const w = $("#hid").width(); return [w, e.getAttribute("style")] })

// extra edge cases
const T = (fn) => { try { return fn() } catch (e) { return { threw: e && e.name } } }
C("css rel on auto width", ($, win, doc) => { const d = doc.createElement("div"); doc.body.appendChild(d); $(d).css("width", "+=10"); const a = d.getAttribute("style"); $(d).css("opacity", "+=0.25"); $(d).css("zIndex", "-=2"); return [a, d.getAttribute("style")] })
C("css shorthand get", ($) => [$("#box").css("margin"), $("#box").css("padding"), $("#box").css("border"), $("#box").css("borderWidth"), $("#box").css("background")])
C("css no args / odd args", ($, win) => [T(() => ser($("#box").css(), win)), T(() => ser($("#box").css(null), win)), T(() => ser($("#box").css(undefined), win)), T(() => $("#box").css({}).length), T(() => ser($("#box").css(["width"], 5), win)), T(() => { $("#box").css("width", {}); return $("#box").attr("style") }), T(() => ser($("#box").css(5), win)), T(() => ser($("#box").css("width", "10px", "extra"), win))])
C("css set with extra args via style", ($, win, doc) => { const e = g(doc, "box"); return [T(() => $.style(e, "padding", 5, "margin")), e.getAttribute("style"), T(() => $.style(e, "width", 50, "padding")), e.getAttribute("style"), T(() => $.style(e, "width", "5em", "border")), e.getAttribute("style")] })
C("css numeric extra variants", ($, win, doc) => { const e = g(doc, "bb"); return [$.css(e, "height", "padding"), $.css(e, "height", "border"), $.css(e, "height", "margin"), $.css(e, "height", "content"), $.css(e, "height", true), $.css(e, "height", false), $.css(e, "height", 0), $.css(e, "height", null), $.css(e, "height", "xyz")] })
C("user expand hook", ($) => { $.cssHooks.foo = { expand: (v) => ({ fooA: v, fooB: v }) }; const r = [$("#box").css("foo"), Object.keys($.cssHooks.foo)]; delete $.cssHooks.foo; return r })
C("svg dims", ($, win, doc) => { doc.body.insertAdjacentHTML("beforeend", '<svg id="svg" width="40" height="30"><rect id="rect" width="10" height="10" style="width:10px"/></svg>'); return ["svg", "rect"].map((id) => [id, T(() => $("#" + id).width()), T(() => $("#" + id).outerWidth(true)), T(() => $("#" + id).css("width")), T(() => ser($("#" + id).offset(), win)), T(() => ser($("#" + id).position(), win)), T(() => $("#" + id).innerHeight())]) })
C("svg set dims", ($, win, doc) => { doc.body.insertAdjacentHTML("beforeend", '<svg id="svg" width="40" height="30"><rect id="rect" width="10" height="10"/></svg>'); $("#rect").width(20); $("#rect").innerHeight(15); $("#rect").css("opacity", 0.5).hide(); const a = g(doc, "rect").getAttribute("style"); $("#rect").show(); return [a, g(doc, "rect").getAttribute("style")] })
C("dims fn with margin", ($) => { const log = []; $("#box").outerHeight(function (i, v) { log.push(v); return 40 }, true); $("#bb").innerWidth(function (i, v) { log.push(v); return "5em" }); return [log, $("#box").attr("style"), $("#bb").attr("style")] })
C("dims on hidden ancestors", ($, win, doc) => { doc.body.insertAdjacentHTML("beforeend", '<div id="outer" style="display:none"><div id="in2" class="box">x</div></div>'); return [$("#in2").width(), $("#in2").outerWidth(true), $("#in2").css("width"), $("#in2").is(":hidden")] })
C("position with static docElem parent", ($, win, doc) => { const e = g(doc, "box"); stub(e, { top: 30, left: 25, op: doc.documentElement }); stub(doc.documentElement, { top: 2, left: 3 }); return [$("#box").position()] })
C("offset chain of offsetParents", ($, win, doc) => { const w = g(doc, "wrap"), i = g(doc, "inner"), d = g(doc, "deep"); w.style.position = "static"; i.style.position = "absolute"; stub(i, { op: w, top: 5, left: 6 }); stub(d, { op: i, top: 9, left: 11 }); stub(w, { op: doc.body }); return [$("#deep").offsetParent().map((_, e) => e.id).get(), $("#deep").position(), $("#inner").offsetParent().map((_, e) => e.id || e.nodeName).get()] })
C("show hide toggle chains", ($, win, doc) => { const r = [$("#box").hide().show().hide().length, $("#li1, #li2").toggle().toggle(false).toggle(true).length]; return [r, vis($, doc)] })
C("hide stores olddisplay then show", ($, win, doc) => { $("#sp2").hide(); const a = $("#sp2").attr("style"); $("#sp2").css("display", "none"); $("#sp2").show(); return [a, $("#sp2").attr("style")] })
C("hidden visible on detached/text", ($, win, doc) => { const t = doc.createTextNode("x"); return [T(() => $(t).is(":visible")), T(() => $([g(doc, "box"), t]).filter(":hidden").length)] })
C("scroll on text nodes and fns", ($, win, doc) => { const t = doc.createTextNode("x"); return [T(() => ser($(t).scrollTop(), win)), T(() => $(t).scrollTop(5).length), T(() => t.scrollTop)] })
C("offset on text/detached", ($, win, doc) => { const t = doc.createTextNode("x"); const d = doc.createElement("div"); return [T(() => ser($(t).offset(), win)), T(() => ser($(d).offset(), win)), T(() => ser($(d).position(), win)), T(() => ser($(t).position(), win))] })
C("animate relative unit conversions", ($, win, doc) => { const out = []; for (const [p, v] of [["width", "+=1em"], ["height", "-=1em"], ["marginLeft", "2em"], ["paddingTop", "+=3%"], ["opacity", "+=0.1"], ["left", "10em"]]) { $("#em").animate({ [p]: v }, { duration: 1000, start(anim) { for (const t of anim.tweens) out.push([t.prop, t.unit, Math.round(t.start * 1000) / 1000, Math.round(t.end * 1000) / 1000]) } }); $("#em").stop(true, true) } return [out, g(doc, "em").getAttribute("style")] })
C("support function identity", ($) => [typeof $.support.boxSizingReliable, $.support.boxSizingReliable === $.support.boxSizingReliable, $.support.reliableTrDimensions(), $.support.reliableTrDimensions()])

// ---- runner ---------------------------------------------------------------------------------
function resetWindow(win) {
  const doc = win.document
  doc.body.innerHTML = BODY
  doc.body.removeAttribute("style")
  doc.documentElement.removeAttribute("style")
  for (const k of ["offsetWidth", "offsetHeight", "offsetParent", "scrollWidth", "scrollHeight", "clientWidth", "clientHeight", "getClientRects", "getBoundingClientRect"]) {
    delete doc.body[k]
    delete doc.documentElement[k]
  }
}
function runCase(c, $, win) {
  resetWindow(win)
  $.fx.off = false
  let result
  try {
    result = ser(c.run($, win, win.document), win)
  } catch (e) {
    result = { threw: e && e.name, msg: String(e && e.message).slice(0, 60) }
  }
  try { $("*").stop(true, true) } catch (e) { }
  const html = win.document.body.innerHTML
  return JSON.stringify({ result, html })
}

const results = {}
let mismatches = []
for (const c of cases) {
  const a = runCase(c, official, winA)
  const b = runCase(c, lil, winB)
  results[c.name] = { official: a, ours: b }
  if (a !== b) mismatches.push(c.name)
}
console.log(`cases ${cases.length} mismatches ${mismatches.length}`)
for (const m of mismatches) {
  console.log("MISMATCH " + m)
  if (process.env.VERBOSE) { console.log("  official " + results[m].official.slice(0, 600)); console.log("  ours     " + results[m].ours.slice(0, 600)) }
}
if (outPath) writeFileSync(outPath, JSON.stringify({ cases: cases.length, mismatches, results }, null, 1))
process.exit(0)
