// Scenarios for the event group. Each is [name, (env) => result]; env = { $, w, d, log, H, id, ser }.
export const scenarios = []
const S = (name, fn) => scenarios.push([name, fn])
const g = (env, id) => env.d.getElementById(id)

// handleObj digest of $._data(el, "events"); handlers named through `names` (Map fn->name)
function dumpEvents(env, el, names = new Map()) {
  const events = env.$._data(el, "events")
  if (!events) return events === undefined ? "<none>" : events
  const out = {}
  for (const type of Object.keys(events)) {
    const list = events[type]
    out[type] = {
      delegateCount: list.delegateCount,
      handlers: list.map((h) => ({
        keys: Object.keys(h).join(","),
        type: h.type,
        origType: h.origType,
        data: env.ser(h.data),
        handler: names.get(h.handler) || (typeof h.handler === "function" ? "<other-fn>" : env.ser(h.handler)),
        guidIsHandlerGuid: h.guid === (h.handler && h.handler.guid),
        guidType: typeof h.guid,
        selector: env.ser(h.selector),
        needsContext: env.ser(h.needsContext),
        namespace: env.ser(h.namespace),
      })),
    }
  }
  return out
}

// ---------------------------------------------------------------- on(): argument forms
const onForms = {
  "types,fn": ($el, f) => $el.on("click", f),
  "types,null,fn": ($el, f) => $el.on("click", null, f),
  "types,undef,fn": ($el, f) => $el.on("click", undefined, f),
  "types,data,fn": ($el, f) => $el.on("click", { d: 1 }, f),
  "types,numdata,fn": ($el, f) => $el.on("click", 5, f),
  "types,zerodata,fn": ($el, f) => $el.on("click", 0, f),
  "types,sel,fn": ($el, f) => $el.on("click", "span", f),
  "types,emptysel,fn": ($el, f) => $el.on("click", "", f),
  "types,sel,data,fn": ($el, f) => $el.on("click", "span", { d: 2 }, f),
  "types,null,data,fn": ($el, f) => $el.on("click", null, { d: 3 }, f),
  "types,sel,null,fn": ($el, f) => $el.on("click", "span", null, f),
  "types,undef,undef,fn": ($el, f) => $el.on("click", undefined, undefined, f),
  "types,false": ($el) => $el.on("click", false),
  "types,sel,false": ($el) => $el.on("click", "span", false),
  "types,null,false": ($el) => $el.on("click", null, false),
  "types only": ($el) => $el.on("click"),
  "types,null": ($el) => $el.on("click", null),
  "types,sel,undef": ($el) => $el.on("click", "span", undefined),
  "types,data(fn),undef": ($el, f) => $el.on("click", f, undefined),
  "types,sel,data,false": ($el) => $el.on("click", "span", { d: 4 }, false),
  "types,data,0": ($el) => $el.on("click", { d: 5 }, 0),
  "map": ($el, f, f2) => $el.on({ click: f, mouseover: f2 }),
  "map,sel": ($el, f, f2) => $el.on({ click: f, mouseover: f2 }, "span"),
  "map,data": ($el, f) => $el.on({ click: f }, { d: 6 }),
  "map,sel,data": ($el, f) => $el.on({ click: f }, "span", { d: 7 }),
  "map,null,data": ($el, f) => $el.on({ click: f }, null, { d: 8 }),
  "map,undef,data": ($el, f) => $el.on({ click: f }, undefined, { d: 9 }),
  "map,num,data": ($el, f) => $el.on({ click: f }, 3, { d: 10 }),
  "map false": ($el) => $el.on({ click: false }),
  "multi types": ($el, f) => $el.on("click dblclick", f),
  "multi types ws": ($el, f) => $el.on("  click \t dblclick  ", f),
  "namespaced": ($el, f) => $el.on("click.b.a", f),
  "ns only": ($el, f) => $el.on(".ns", f),
  "empty types": ($el, f) => $el.on("", f),
  "null types": ($el, f) => $el.on(null, f),
  "undef types": ($el, f) => $el.on(undefined, f),
}
for (const method of ["on", "one"]) {
  for (const [form, bind] of Object.entries(onForms)) {
    S(`${method}(${form}) bind+trigger`, (env) => {
      const { $, H } = env
      const f = H("f", "rf"), f2 = H("f2")
      const $p = $("#p1")
      const on = $p[method]
      const bindWith = (...a) => on.apply($p, a)
      // route the form through the chosen method
      const r = bind({ on: bindWith }, f, f2)
      const same = r === $p
      const before = dumpEvents(env, g(env, "p1"), new Map([[f, "f"], [f2, "f2"]]))
      const t1 = $("#s1").trigger("click")
      const t2 = $("#p1").trigger("click")
      const t3 = $("#p1").trigger("mouseover")
      const t4 = $("#s1").triggerHandler("click")
      const after = dumpEvents(env, g(env, "p1"), new Map([[f, "f"], [f2, "f2"]]))
      return { same, before, after, t1: t1 === undefined ? "u" : t1.length, t4, global: Object.keys($.event.global).sort() }
    })
  }
}

// ---------------------------------------------------------------- off(): forms
const offForms = {
  "off()": ($el) => $el.off(),
  "off(click)": ($el) => $el.off("click"),
  "off(click,f)": ($el, f) => $el.off("click", f),
  "off(click,f2)": ($el, f, f2) => $el.off("click", f2),
  "off(click,span)": ($el) => $el.off("click", "span"),
  "off(click,**)": ($el) => $el.off("click", "**"),
  "off(click,span,f)": ($el, f) => $el.off("click", "span", f),
  "off(click,b,f)": ($el, f) => $el.off("click", "b", f),
  "off(click,null,f)": ($el, f) => $el.off("click", null, f),
  "off(click,undef,f)": ($el, f) => $el.off("click", undefined, f),
  "off(click,false)": ($el) => $el.off("click", false),
  "off(click,span,false)": ($el) => $el.off("click", "span", false),
  "off(.a)": ($el) => $el.off(".a"),
  "off(.b.a)": ($el) => $el.off(".b.a"),
  "off(.c)": ($el) => $el.off(".c"),
  "off(click.a)": ($el) => $el.off("click.a"),
  "off(click.a.b)": ($el) => $el.off("click.a.b"),
  "off(click.b)": ($el) => $el.off("click.b"),
  "off(mouseover click)": ($el) => $el.off("mouseover click"),
  "off(map)": ($el, f, f2) => $el.off({ click: f, mouseover: f2 }),
  "off(map,span)": ($el, f) => $el.off({ click: f }, "span"),
  "off(null)": ($el) => $el.off(null),
  "off('')": ($el) => $el.off(""),
  "off(undefined,span)": ($el) => $el.off(undefined, "span"),
  "off(click,fn-unknown)": ($el) => $el.off("click", function () {}),
  "off(mouseenter)": ($el) => $el.off("mouseenter"),
  "off(focus)": ($el) => $el.off("focus"),
}
for (const [form, unbind] of Object.entries(offForms)) {
  S(`off form ${form}`, (env) => {
    const { $, H } = env
    const f = H("f"), f2 = H("f2"), f3 = H("f3"), f4 = H("f4"), f5 = H("f5")
    const $p = $("#p1")
    $p.on("click", f).on("click.a", f2).on("click.a.b", f3).on("click", "span", f).on("click.a", "span", f4)
    $p.on("mouseover.a", f2).on("click", "b", f5).on("click", false).on("mouseenter", f).on("focus", f5)
    const names = new Map([[f, "f"], [f2, "f2"], [f3, "f3"], [f4, "f4"], [f5, "f5"]])
    let ret
    try { ret = unbind($p, f, f2) === $p } catch (e) { ret = "throw:" + e.name }
    const after = dumpEvents(env, g(env, "p1"), names)
    $("#s1").trigger("click")
    $("#p1").trigger("mouseover")
    return { ret, after, hasData: $.hasData(g(env, "p1")) }
  })
}

S("off(event) inside handler", (env) => {
  const { $, log } = env
  const $p = $("#p1")
  function h(e) { log.push("h:" + e.handleObj.origType + ":" + e.handleObj.namespace); $(this).off(e) }
  $p.on("click.x.y", h).on("click", h).on("click", "span", h)
  $("#s1").trigger("click"); $("#s1").trigger("click"); $p.trigger("click")
  return dumpEvents(env, g(env, "p1"), new Map([[h, "h"]]))
})
S("off(event) returns this", (env) => {
  const { $ } = env
  let r
  const $o = $("#other")
  $("#p1").on("click", function (e) { r = $o.off(e) === $o })
  $("#p1").trigger("click")
  return r
})
S("off on empty set / text node", (env) => {
  const { $, d } = env
  const t = d.createTextNode("x")
  const a = $().off("click")
  const b = $(t).on("click", () => {}).off("click")
  return [a.length, b.length, $.hasData(t)]
})

// ---------------------------------------------------------------- one()
S("one fires once, identity", (env) => {
  const { $, H } = env
  const f = H("f", 7)
  $("#p1").one("click", f)
  const d1 = dumpEvents(env, g(env, "p1"), new Map([[f, "f"]]))
  const r1 = $("#p1").triggerHandler("click")
  const r2 = $("#p1").triggerHandler("click")
  return { d1, r1, r2, guid: typeof f.guid, d2: dumpEvents(env, g(env, "p1")) }
})
S("one delegated fires once per selector", (env) => {
  const { $, H } = env
  const f = H("f")
  $("#mid").one("click", "p", f)
  $("#s1").trigger("click"); $("#b1").trigger("click"); $("#s1").trigger("click")
  return dumpEvents(env, g(env, "mid"))
})
S("one on multiple elements", (env) => {
  const { $, H } = env
  const f = H("f")
  $("p").one("click", f)
  $("#p1").trigger("click"); $("#p2").trigger("click"); $("#p1").trigger("click"); $("#p2").trigger("click")
  return [dumpEvents(env, g(env, "p1")), dumpEvents(env, g(env, "p2"))]
})
S("one + on same fn: one removes both", (env) => {
  const { $, H } = env
  const f = H("f")
  $("#p1").on("click", f).one("click", f)
  $("#p1").trigger("click"); $("#p1").trigger("click")
  return dumpEvents(env, g(env, "p1"))
})
S("one then off(fn) before firing", (env) => {
  const { $, H } = env
  const f = H("f")
  $("#p1").one("click", f).off("click", f)
  $("#p1").trigger("click")
  return dumpEvents(env, g(env, "p1"))
})
S("one with map and data", (env) => {
  const { $, H } = env
  const f = H("f"), f2 = H("f2")
  $("#p1").one({ click: f, mouseover: f2 }, { d: 1 })
  $("#p1").trigger("click").trigger("mouseover").trigger("click").trigger("mouseover")
  return dumpEvents(env, g(env, "p1"))
})
S("one mouseenter: related inside does not consume", (env) => {
  const { $, w, H } = env
  const f = H("f")
  $("#p1").one("mouseenter", f)
  const s1 = g(env, "s1"), p1 = g(env, "p1"), mid = g(env, "mid")
  s1.dispatchEvent(new w.MouseEvent("mouseover", { bubbles: true, relatedTarget: p1 }))
  s1.dispatchEvent(new w.MouseEvent("mouseover", { bubbles: true, relatedTarget: mid }))
  s1.dispatchEvent(new w.MouseEvent("mouseover", { bubbles: true, relatedTarget: mid }))
  return dumpEvents(env, p1)
})
S("one with false", (env) => {
  const { $ } = env
  $("#p1").one("click", false)
  const e1 = $.Event("click"); $("#p1").trigger(e1)
  const e2 = $.Event("click"); $("#p1").trigger(e2)
  return [e1.isDefaultPrevented(), e2.isDefaultPrevented(), dumpEvents(env, g(env, "p1"))]
})
S("one handler guid shared with original", (env) => {
  const { $ } = env
  const f = function () {}
  f.guid = 77
  $("#p1").one("click", f)
  const h = $._data(g(env, "p1"), "events").click[0]
  return [h.guid, h.handler.guid, f.guid, h.handler === f]
})

// ---------------------------------------------------------------- trigger / triggerHandler
const dataForms = {
  none: [],
  undef: [undefined],
  null: [null],
  array: [[1, "two", { t: 3 }]],
  emptyArray: [[]],
  scalar: ["x"],
  zero: [0],
  object: [{ a: 1 }],
  false: [false],
  arrayLike: [{ 0: "a", 1: "b", length: 2 }],
}
for (const [form, data] of Object.entries(dataForms)) {
  S(`trigger data ${form}`, (env) => {
    const { $, H } = env
    $("#p1").on("custom", H("p", 1)).on("custom", H("p2"))
    $("#mid").on("custom", H("mid", 2))
    const r = $("#s1").trigger("custom", ...data)
    const rh = $("#p1").triggerHandler("custom", ...data)
    return [r.length, rh]
  })
}
S("trigger bubbling order and window", (env) => {
  const { $, w, d, H } = env
  for (const sel of ["#s1", "#p1", "#mid", "#outer", "body", "html"]) $(sel).on("click", H(sel))
  $(d).on("click", H("doc")); $(w).on("click", H("win"))
  $("#s1").trigger("click")
  $(d).trigger("click")
  $(w).trigger("click")
  return true
})
S("trigger stopPropagation / stopImmediate / return false", (env) => {
  const { $, H } = env
  $("#s1").on("click", H("s1a", (e) => e.stopImmediatePropagation())).on("click", H("s1b"))
  $("#p1").on("click", H("p1a", (e) => e.stopPropagation())).on("click", H("p1b"))
  $("#mid").on("click", H("mid"))
  $("#b1").on("click", H("b1", () => false))
  $("#p2").on("click", H("p2"))
  const e1 = $.Event("click"); $("#s1").trigger(e1)
  const e2 = $.Event("click"); $("#b1").trigger(e2)
  const e3 = $.Event("click"); $("#p1").trigger(e3)
  const st = (e) => [e.isDefaultPrevented(), e.isPropagationStopped(), e.isImmediatePropagationStopped(), e.result === undefined ? "<u>" : e.result]
  return [st(e1), st(e2), st(e3)]
})
S("trigger namespaces", (env) => {
  const { $, H } = env
  const $p = $("#p1")
  $p.on("click", H("plain")).on("click.a", H("a")).on("click.a.b", H("ab")).on("click.b", H("b")).on("click.c.a", H("ca"))
  for (const t of ["click", "click.a", "click.b", "click.a.b", "click.b.a", "click.c", "click.x", "click.a.c"]) {
    env.log.push("--" + t)
    $p.trigger(t)
  }
  return true
})
S("trigger event object fields", (env) => {
  const { $ } = env
  let seen
  $("#p1").on("click.n1", function (e) {
    seen = {
      isTrigger: e.isTrigger, ns: e.namespace, rns: String(e.rnamespace), target: env.id(e.target), ct: env.id(e.currentTarget),
      dt: env.id(e.delegateTarget), hoType: e.handleObj.type, hoOrig: e.handleObj.origType, hoNs: e.handleObj.namespace,
      data: e.data, result: e.result, type: e.type, isSim: e.isSimulated, orig: e.originalEvent === undefined,
      isDP: e.isDefaultPrevented(), ts: typeof e.timeStamp,
    }
  })
  $("#s1").trigger("click.n1")
  const a = seen
  $("#p1").triggerHandler("click.n1")
  return [a, seen]
})
S("trigger reuses event object, result reset", (env) => {
  const { $ } = env
  $("#p1").on("custom", () => 5)
  $("#mid").on("custom", () => undefined)
  const e = $.Event("custom")
  $("#p1").trigger(e)
  const r1 = e.result
  $("#mid").trigger(e)
  return [r1, e.result === undefined, e.type, e.target && e.target.id]
})
S("trigger with preset target", (env) => {
  const { $ } = env
  let t
  $("#p1").on("custom", (e) => { t = env.id(e.target) })
  const e = $.Event("custom", { target: g(env, "other") })
  $("#p1").trigger(e)
  return t
})
S("trigger plain object with type", (env) => {
  const { $, H } = env
  $("#p1").on("custom", H("h")).on("custom.ns", H("hns"))
  $("#p1").trigger({ type: "custom", foo: 1 })
  $("#p1").trigger({ type: "custom.ns" })
  $("#p1").trigger({ type: "custom", namespace: "ns" })
  let foo
  $("#p1").on("custom2", (e) => { foo = [e.foo, e.type, e.bar] })
  $("#p1").trigger({ type: "custom2", foo: 9, bar: undefined })
  return foo
})
S("trigger $.Event with props", (env) => {
  const { $ } = env
  let got
  $("#p1").on("custom", (e) => { got = [e.type, e.foo, e.which, e.isDefaultPrevented()] })
  $("#p1").trigger($.Event("custom", { foo: 3, which: 2 }))
  return got
})
S("trigger on plain object", (env) => {
  const { $, H } = env
  const o = { name: "o" }
  $(o).on("custom", H("h", 4))
  const r = $(o).triggerHandler("custom", ["x"])
  $(o).trigger("custom")
  const keys = Object.keys(o).filter((k) => !/^jQuery\d+$/.test(k))
  return [r, keys, typeof $._data(o).events]
})
S("trigger plain object native method + onfoo", (env) => {
  const { $, log } = env
  const o = { name: "o", custom() { log.push("native custom") }, oncustom() { log.push("oncustom"); return false } }
  $(o).on("custom", () => log.push("jq"))
  const e = $.Event("custom")
  $(o).trigger(e)
  return [e.isDefaultPrevented(), e.result]
})
S("trigger on text node / comment", (env) => {
  const { $, d, H } = env
  const t = d.createTextNode("x"), c = d.createComment("c")
  g(env, "p1").appendChild(t); g(env, "p1").appendChild(c)
  $("#p1").on("click", H("p"))
  $(t).trigger("click"); $(c).trigger("click")
  return [$(t).triggerHandler("click"), $.event.trigger("click", null, t)]
})
S("trigger on detached element", (env) => {
  const { $, d, H } = env
  const div = d.createElement("div"), span = d.createElement("span")
  div.appendChild(span)
  $(div).on("click", H("div")); $(d).on("click", H("doc"))
  $(span).trigger("click")
  return true
})
S("trigger onfoo inline handler", (env) => {
  const { $, log } = env
  const p = g(env, "p1")
  p.onclick = function (e) { log.push(["onclick", this.id, e.type, typeof e.isTrigger]); return false }
  g(env, "mid").onclick = function () { log.push("mid onclick") }
  $("#p1").on("click", () => log.push("jq"))
  const e = $.Event("click")
  $("#s1").trigger(e)
  return [e.isDefaultPrevented(), e.result]
})
S("trigger native default: checkbox", (env) => {
  const { $, log } = env
  const cb = g(env, "cb")
  $("#cb").on("click", function () { log.push(["h", this.checked]) })
  $("#cb").trigger("click")
  const a = cb.checked
  $("#cb").trigger("click")
  const b = cb.checked
  $("#cb").on("click", () => false)
  $("#cb").trigger("click")
  return [a, b, cb.checked, dumpEvents(env, cb)]
})
S("trigger native default: checkbox without handlers", (env) => {
  const { $ } = env
  const cb = g(env, "cb")
  $("#cb").trigger("click")
  const a = cb.checked
  $("#rd").trigger("click")
  return [a, g(env, "rd").checked, $.hasData(cb)]
})
S("triggerHandler click on checkbox", (env) => {
  const { $, log } = env
  $("#cb").on("click", function () { log.push(["h", this.checked]); return "r" })
  return [$("#cb").triggerHandler("click"), g(env, "cb").checked]
})
S("trigger click on anchor does not navigate", (env) => {
  const { $, w, log } = env
  g(env, "a1").addEventListener("click", () => log.push("native listener"))
  $("#a1").on("click", () => log.push("jq"))
  $("#a1").trigger("click")
  return w.location.hash
})
S("trigger click on button native", (env) => {
  const { $, log } = env
  g(env, "btn2").addEventListener("click", (e) => log.push(["native", e.isTrusted]))
  $("#btn2").on("click", (e) => log.push(["jq", typeof e.isTrigger]))
  $("#btn2").trigger("click")
  return true
})
S("trigger focus / blur", (env) => {
  const { $, d, log } = env
  $("#txt").on("focus", (e) => log.push(["focus", e.type, d.activeElement && d.activeElement.id]))
  $("#txt").on("blur", (e) => log.push(["blur", e.type]))
  $("#mid").on("focusin", (e) => log.push(["focusin", e.type, e.target.id]))
  $("#mid").on("focusout", (e) => log.push(["focusout", e.type, e.target.id]))
  $("#txt").trigger("focus")
  const a = d.activeElement && d.activeElement.id
  $("#txt").trigger("blur")
  const b = d.activeElement && d.activeElement.id
  $("#fi").trigger("focus")
  return [a, b, d.activeElement && d.activeElement.id]
})
S("triggerHandler focus does not focus", (env) => {
  const { $, d, log } = env
  $("#txt").on("focus", (e) => { log.push(["focus", e.type]); return 3 })
  const r = $("#txt").triggerHandler("focus")
  return [r, d.activeElement && d.activeElement.id]
})
S("native focus with jq focusin/focus handlers", (env) => {
  const { $, d, log } = env
  $("#txt").on("focus", (e) => log.push(["focus", e.type, e.isTrigger]))
  $(d).on("focusin", (e) => log.push(["doc focusin", e.target.id, e.type]))
  $("#outer").on("focusin", "input", (e) => log.push(["deleg focusin", e.target.id, e.currentTarget.id]))
  g(env, "txt").focus()
  g(env, "fi").focus()
  return d.activeElement.id
})
S("focusin setup/teardown counters", (env) => {
  const { $, d } = env
  const f = () => {}
  $("#p1").on("focusin", f); $("#p2").on("focusin", f)
  const a = $._data(d, "focusin")
  $("#p1").off("focusin")
  const b = $._data(d, "focusin")
  $("#p2").off("focusin")
  const c = $._data(d, "focusin")
  $("#p1").on("focusout", f)
  return [a, b, c, $._data(d, "focusout")]
})
S("trigger focusin directly", (env) => {
  const { $, log } = env
  $("#txt").on("focusin", (e) => log.push(["fi", e.type]))
  $("#mid").on("focusin", (e) => log.push(["mid fi", e.type]))
  $("#txt").trigger("focusin")
  return true
})
S("trigger with rfocusMorph guard", (env) => {
  const { $, log } = env
  $("#txt").on("focus", () => { log.push("focus"); $("#txt").trigger("focusin") })
  $("#txt").on("focusin", () => log.push("focusin"))
  $("#txt").trigger("focus")
  return true
})
S("trigger colon type", (env) => {
  const { $, log } = env
  const p = g(env, "p1")
  p["my:evt"] = () => log.push("native colon")
  p["onmy:evt"] = () => log.push("on colon")
  $("#p1").on("my:evt", () => log.push("jq colon"))
  $("#p1").trigger("my:evt")
  return true
})
S("trigger load noBubble", (env) => {
  const { $, w, H } = env
  $("#p1").on("load", H("p")); $("#mid").on("load", H("mid")); $(w).on("load", H("win"))
  $("#p1").trigger("load")
  return true
})
S("trigger on multiple elements", (env) => {
  const { $, H } = env
  $("p").on("custom", H("p"))
  const r = $("p").trigger("custom", ["d"])
  const rh = $("p").triggerHandler("custom")
  return [r.length, rh]
})
S("triggerHandler empty set", (env) => {
  const { $ } = env
  return [$().triggerHandler("click"), $().trigger("click").length]
})
S("triggerHandler returns last non-undefined", (env) => {
  const { $ } = env
  $("#p1").on("custom", () => 1).on("custom", () => undefined).on("custom", () => null)
  const a = $("#p1").triggerHandler("custom")
  $("#p2").on("custom", () => 1).on("custom", () => 0)
  return [a, $("#p2").triggerHandler("custom")]
})
S("trigger with jQuery.Event from native", (env) => {
  const { $, w, log } = env
  const native = new w.MouseEvent("click", { bubbles: true, button: 0, clientX: 3 })
  const e = $.Event(native)
  $("#p1").on("click", (ev) => log.push([ev === e, ev.clientX, ev.type, ev.originalEvent === native]))
  $("#p1").trigger(e)
  return e.isDefaultPrevented()
})
S("$.event.trigger direct", (env) => {
  const { $, H } = env
  $("#p1").on("custom", H("p", 2)); $("#mid").on("custom", H("mid"))
  const p = g(env, "p1")
  const a = $.event.trigger("custom", ["x"], p)
  const b = $.event.trigger("custom", ["y"], p, true)
  const c = $.event.trigger("custom", undefined, p, 1)
  return [a, b, c]
})
S("$.event.trigger without elem uses document", (env) => {
  const { $, d, H } = env
  $(d).on("custom", H("doc", 1))
  return $.event.trigger("custom")
})
S("trigger during dispatch adds/removes handlers", (env) => {
  const { $, log } = env
  const $p = $("#p1")
  const late = () => log.push("late")
  $p.on("click", () => { log.push("first"); $p.on("click", late) })
  $p.on("click", function second() { log.push("second"); $p.off("click", second) })
  $p.trigger("click"); $p.trigger("click")
  return dumpEvents(env, g(env, "p1")).click.handlers.length
})

// ---------------------------------------------------------------- delegation
S("delegation order and matching", (env) => {
  const { $, H } = env
  $("#outer").on("click", "p", H("p")).on("click", "span", H("span")).on("click", ".x", H(".x")).on("click", H("direct"))
    .on("click", "div", H("div")).on("click", "#outer", H("self"))
  $("#s1").trigger("click")
  $("#b1").trigger("click")
  $("#outer").trigger("click")
  return dumpEvents(env, g(env, "outer")).click.delegateCount
})
S("delegation stopPropagation beneath", (env) => {
  const { $, H } = env
  $("#outer").on("click", "span", H("span", (e) => e.stopPropagation())).on("click", "p", H("p")).on("click", H("direct"))
  $("#s1").trigger("click")
  return true
})
S("delegation disabled button suppressed", (env) => {
  const { $, H } = env
  $("#mid").on("click", "button", H("btn")).on("click", H("direct"))
  $("#btn").trigger("click"); $("#btn2").trigger("click")
  $("#mid").on("mouseover", "button", H("over"))
  $("#btn").trigger("mouseover")
  return true
})
S("delegation non-primary button suppressed", (env) => {
  const { $, H } = env
  $("#mid").on("click", "p", H("p")).on("click", H("direct")).on("mousedown", "p", H("md"))
  $("#s1").trigger($.Event("click", { button: 1 }))
  $("#s1").trigger($.Event("click", { button: 0 }))
  $("#s1").trigger($.Event("click", { button: -1 }))
  $("#s1").trigger($.Event("mousedown", { button: 2 }))
  return true
})
S("delegation needsContext selectors", (env) => {
  const { $, H } = env
  $("#mid").on("click", "> p", H("child-p")).on("click", "p:first", H("p:first")).on("click", "p:eq(1)", H("p:eq1"))
    .on("click", "> p > span", H("child-span")).on("click", ":last", H(":last"))
  const d = dumpEvents(env, g(env, "mid"))
  $("#s1").trigger("click"); $("#b1").trigger("click")
  return d
})
S("delegation same selector cached per element", (env) => {
  const { $, H } = env
  $("#mid").on("click", "p", H("a")).on("click", "p", H("b")).on("click", "p ", H("c"))
  $("#s1").trigger("click")
  return true
})
S("delegation invalid selector throws at bind", (env) => {
  const { $ } = env
  let err = "none"
  try { $("#mid").on("click", "[[[", () => {}) } catch (e) { err = e.name }
  return [err, $.hasData(g(env, "mid")), dumpEvents(env, g(env, "mid"))]
})
S("delegation on document with target text node", (env) => {
  const { $, w, d, H } = env
  $(d).on("click", "p", H("p"))
  const text = g(env, "p1").firstChild
  text.dispatchEvent(new w.MouseEvent("click", { bubbles: true }))
  return true
})
S("delegated selector matching Object.prototype names", (env) => {
  const { $, H } = env
  $("#mid").on("click", "constructor", H("c")).on("click", "p", H("p"))
  $("#s1").trigger("click")
  return true
})
S("$.event.handlers", (env) => {
  const { $ } = env
  const mid = g(env, "mid")
  const f = () => {}
  $("#mid").on("click", "p", f).on("click", f).on("click", "span", f)
  const handlers = $._data(mid, "events").click
  const ev = $.Event("click", { target: g(env, "s1") })
  let q
  try {
    q = $.event.handlers.call(mid, ev, handlers).map((e) => [env.id(e.elem), e.handlers.length])
  } catch (e) { q = "throw:" + e.name }
  return q
})

// ---------------------------------------------------------------- native dispatch
S("native dispatch through jq handle", (env) => {
  const { $, w, H } = env
  $("#p1").on("click", H("p", (e) => { env.log.push(["orig", e.originalEvent && e.originalEvent.type, e.isTrigger, e.clientX, e.which, e.button]) }))
  $("#outer").on("click", "span", H("deleg"))
  g(env, "s1").dispatchEvent(new w.MouseEvent("click", { bubbles: true, clientX: 7, button: 0 }))
  return true
})
S("native event preventDefault reflects", (env) => {
  const { $, w } = env
  $("#p1").on("click", (e) => { e.preventDefault() })
  $("#mid").on("click", (e) => { env.log.push(["mid", e.isDefaultPrevented()]) })
  const ev = new w.MouseEvent("click", { bubbles: true, cancelable: true })
  g(env, "s1").dispatchEvent(ev)
  return ev.defaultPrevented
})
S("native event return false stops native propagation", (env) => {
  const { $, w, log } = env
  $("#p1").on("click", () => false)
  g(env, "mid").addEventListener("click", () => log.push("native mid"))
  const ev = new w.MouseEvent("click", { bubbles: true, cancelable: true })
  g(env, "s1").dispatchEvent(ev)
  return ev.defaultPrevented
})
S("native keyboard event props", (env) => {
  const { $, w } = env
  let got
  $("#txt").on("keydown", (e) => {
    got = [e.key, e.keyCode, e.which, e.code, e.altKey, e.ctrlKey, e.shiftKey, e.metaKey, e.charCode, e.char, "key" in e, e.view === w]
  })
  g(env, "txt").dispatchEvent(new w.KeyboardEvent("keydown", { key: "a", code: "KeyA", bubbles: true, ctrlKey: true, view: w }))
  return got
})
S("native mouse props", (env) => {
  const { $, w } = env
  let got
  $("#p1").on("mousedown", (e) => {
    got = [e.pageX, e.pageY, e.clientX, e.clientY, e.screenX, e.button, e.buttons, e.which, e.offsetX, e.relatedTarget, e.detail, e.bubbles, e.cancelable, e.eventPhase]
  })
  g(env, "p1").dispatchEvent(new w.MouseEvent("mousedown", { clientX: 5, clientY: 6, screenX: 7, button: 2, buttons: 2, bubbles: true, detail: 1 }))
  return got
})
S("native beforeunload postDispatch", (env) => {
  const { $, w } = env
  $(w).on("beforeunload", () => "leave?")
  const ev = new w.Event("beforeunload")
  w.dispatchEvent(ev)
  const a = ev.returnValue
  $(w).off("beforeunload").on("beforeunload", () => undefined)
  const ev2 = new w.Event("beforeunload")
  w.dispatchEvent(ev2)
  return [a, ev2.returnValue]
})
S("native mouseenter via mouseover", (env) => {
  const { $, w, H } = env
  $("#p1").on("mouseenter", H("enter")).on("mouseleave", H("leave"))
  $("#mid").on("mouseenter", "p", H("deleg enter"))
  const s1 = g(env, "s1"), p1 = g(env, "p1"), mid = g(env, "mid")
  s1.dispatchEvent(new w.MouseEvent("mouseover", { bubbles: true, relatedTarget: p1 }))
  s1.dispatchEvent(new w.MouseEvent("mouseover", { bubbles: true, relatedTarget: mid }))
  s1.dispatchEvent(new w.MouseEvent("mouseover", { bubbles: true }))
  s1.dispatchEvent(new w.MouseEvent("mouseout", { bubbles: true, relatedTarget: mid }))
  s1.dispatchEvent(new w.MouseEvent("mouseout", { bubbles: true, relatedTarget: s1 }))
  return dumpEvents(env, p1)
})
S("trigger mouseenter / pointerleave", (env) => {
  const { $, H } = env
  $("#p1").on("mouseenter", H("enter", () => "r")).on("pointerleave", H("pleave"))
  $("#mid").on("mouseover", H("mid over"))
  const r = $("#p1").triggerHandler("mouseenter")
  $("#p1").trigger("mouseenter"); $("#p1").trigger("pointerleave"); $("#p1").trigger("mouseover")
  return r
})
S("native focus/blur with leverageNative", (env) => {
  const { $, d, log } = env
  $("#txt").on("focus", function (e) { log.push(["jq focus", e.type, typeof e.isTrigger, d.activeElement.id]) })
  $("#txt").on("blur", function (e) { log.push(["jq blur", e.type]) })
  g(env, "txt").focus()
  g(env, "txt").blur()
  $("#txt").trigger("focus")
  $("#txt").trigger("focus")
  return [d.activeElement.id, $._data(g(env, "txt"), "focus")]
})
S("trigger focus with extra data args", (env) => {
  const { $, log } = env
  $("#txt").on("focus", function (e, a, b) { log.push(["focus", a, b, e.isTrigger]) })
  $("#txt").trigger("focus", ["x", "y"])
  return true
})
S("checkbox click handler sees data args", (env) => {
  const { $, log } = env
  $("#cb").on("click", function (e, a) { log.push(["click", a, this.checked]) })
  $("#cb").trigger("click", ["arg"])
  return g(env, "cb").checked
})

// ---------------------------------------------------------------- jQuery.Event
S("$.Event basics", (env) => {
  const { $ } = env
  const e = $.Event("custom")
  const n = new $.Event("custom2", { a: 1 })
  return [e.type, n.type, n.a, e.isDefaultPrevented(), e.isPropagationStopped(), e.isImmediatePropagationStopped(), e.isSimulated,
    typeof e.timeStamp, e.originalEvent, e[$.expando], e.target, e.currentTarget, "timeStamp" in e]
})
S("$.Event instanceof and prototype", (env) => {
  const { $ } = env
  const e = $.Event("x"), n = new $.Event("y")
  return [e instanceof $.Event, n instanceof $.Event, Object.getPrototypeOf(e) === $.Event.prototype, e.constructor === $.Event,
    typeof $.Event.prototype.preventDefault, $.Event.prototype.isSimulated, Object.keys($.Event.prototype).sort().join()]
})
S("$.Event prototype extension visible", (env) => {
  const { $ } = env
  $.Event.prototype.myHelper = function () { return "h:" + this.type }
  let r = "n/a"
  try { r = $.Event("x").myHelper() } catch (e) { r = e.name }
  return r
})
S("$.Event methods", (env) => {
  const { $, w } = env
  const native = new w.Event("click", { cancelable: true, bubbles: true })
  const e = $.Event(native)
  const r1 = e.preventDefault()
  const r2 = e.stopPropagation()
  const r3 = e.stopImmediatePropagation()
  return [r1, r2, r3, native.defaultPrevented, e.isDefaultPrevented(), e.isPropagationStopped(), e.isImmediatePropagationStopped()]
})
S("$.Event simulated does not touch native", (env) => {
  const { $, w } = env
  const native = new w.Event("click", { cancelable: true })
  const e = $.Event(native, { isSimulated: true })
  e.preventDefault()
  return [native.defaultPrevented, e.isDefaultPrevented()]
})
S("$.Event from native: props", (env) => {
  const { $, w } = env
  const native = new w.MouseEvent("click", { cancelable: true, clientX: 1, ctrlKey: true })
  g(env, "p1").dispatchEvent(native)
  const e = $.Event(native)
  return [e.type, e.clientX, e.ctrlKey, e.target, e.currentTarget, e.relatedTarget, e.which, e.timeStamp === native.timeStamp, e.originalEvent === native]
})
S("$.Event from prevented native", (env) => {
  const { $, w } = env
  const native = new w.Event("click", { cancelable: true })
  native.preventDefault()
  return [$.Event(native).isDefaultPrevented(), $.Event(new w.Event("x")).isDefaultPrevented()]
})
S("$.Event from event-like objects", (env) => {
  const { $ } = env
  const r = []
  for (const src of [{ type: "a" }, { type: "" }, {}, { type: 0 }, { type: "b", defaultPrevented: undefined, returnValue: false },
    { type: "c", defaultPrevented: false, returnValue: false }, { type: "d", target: { nodeType: 3, parentNode: "P" } },
    { type: "e", timeStamp: 123 }, { type: "f", timeStamp: 0 }, { type: "g", which: 3, key: "k", nope: 1 }]) {
    const e = $.Event(src)
    r.push([typeof e.type === "object" ? "obj" : e.type, e.originalEvent === src, e.isDefaultPrevented(), e.target, e.timeStamp === 123 ? 123 : typeof e.timeStamp, e.which, e.key, e.nope])
  }
  return r
})
S("$.Event from scalars", (env) => {
  const { $ } = env
  return [undefined, null, "", 0, "x.y", 5].map((s) => { const e = $.Event(s); return [e.type, e.originalEvent] })
})
S("$.Event with props overriding", (env) => {
  const { $ } = env
  const e = $.Event("click", { type: "other", isDefaultPrevented: () => "own", foo: undefined })
  return [e.type, e.isDefaultPrevented(), "foo" in e, e.foo]
})
S("$.Event props: in operator and own keys", (env) => {
  const { $ } = env
  const e = $.Event("x")
  return ["which" in e, "altKey" in e, e.which, Object.keys(e).map((k) => (/^jQuery\d+$/.test(k) ? "<expando>" : k)).join()]
})
S("$.Event native: own keys", (env) => {
  const { $, w } = env
  const e = $.Event(new w.MouseEvent("click", { clientX: 2 }))
  return Object.keys(e).map((k) => (/^jQuery\d+$/.test(k) ? "<expando>" : k)).join()
})
S("$.Event prop getter is live", (env) => {
  const { $ } = env
  const src = { type: "k", which: 1 }
  const e = $.Event(src)
  src.which = 2
  const a = e.which
  e.which = 9
  return [a, e.which, src.which]
})
S("$.Event without originalEvent prop read", (env) => {
  const { $ } = env
  const e = $.Event("x")
  e.key = "z"
  return [e.key, e.pageX, e.touches]
})
S("$.event.addProp", (env) => {
  const { $ } = env
  if (typeof $.event.addProp !== "function") return "missing"
  $.event.addProp("custom1", (ev) => ev.foo + 1)
  $.event.addProp("custom2", true)
  const e = $.Event({ type: "x", foo: 1, custom2: "c2" })
  return [e.custom1, e.custom2, $.Event("y").custom1]
})
S("$.event.fix", (env) => {
  const { $, w } = env
  const n = new w.Event("click")
  const a = $.event.fix(n)
  const b = $.event.fix(a)
  return [a === b, a.originalEvent === n, a.type]
})
S("$.event keys", (env) => {
  const { $ } = env
  return [Object.keys($.event).sort().join(), Object.keys($.event.special).sort().join(), typeof $.removeEvent, $.event.triggered]
})
S("$.event.special shapes", (env) => {
  const { $ } = env
  const out = {}
  for (const k of Object.keys($.event.special).sort()) {
    const s = $.event.special[k]
    out[k] = Object.keys(s).sort().map((p) => p + ":" + (typeof s[p] === "function" ? "fn" + s[p].length : String(s[p]))).join(",")
  }
  return out
})
S("$.event.special static returns", (env) => {
  const { $, d } = env
  const sp = $.event.special
  const cb = g(env, "cb"), a1 = g(env, "a1"), p1 = g(env, "p1")
  return [sp.click.setup.call(p1), sp.click.trigger.call(p1), sp.click._default({ target: a1 }), sp.click._default({ target: p1 }),
    sp.focus.teardown.call(p1), sp.blur.teardown.call(p1), sp.focus._default({ target: p1 }), sp.focus.setup.call(p1), sp.focus.trigger.call(p1),
    $._data(p1, "focus"), sp.click.setup.call(cb), $._data(cb, "click")]
})

// ---------------------------------------------------------------- $.event.add / remove direct, custom specials
S("$.event.add/remove direct", (env) => {
  const { $, H } = env
  const p = g(env, "p1")
  const f = H("f")
  $.event.add(p, "click custom.ns", f, { d: 1 })
  $.event.add(p, "click", f, undefined, "span")
  const a = dumpEvents(env, p, new Map([[f, "f"]]))
  $("#s1").trigger("click")
  $.event.remove(p, "click", f, "span")
  const b = dumpEvents(env, p, new Map([[f, "f"]]))
  $.event.remove(p)
  return [a, b, dumpEvents(env, p), $.hasData(p)]
})
S("$.event.add handleObjIn", (env) => {
  const { $, H } = env
  const p = g(env, "p1")
  const f = H("f")
  $.event.add(p, "click", { handler: f, selector: "span", data: "ignored?", extra: 1, namespace: "zz", origType: "ot", guid: 99 }, { d: 2 })
  const events = $._data(p, "events").click
  const h = events[0]
  $("#s1").trigger("click")
  return [Object.keys(h).join(), h.selector, h.data, h.extra, h.namespace, h.origType, h.guid, events.delegateCount, f.guid]
})
S("$.event.add handleObjIn falsy fields", (env) => {
  const { $ } = env
  const p = g(env, "p1")
  const f = () => {}
  $.event.add(p, "click.ns", { handler: f, type: "", origType: undefined, data: null, namespace: false, needsContext: undefined })
  const h = $._data(p, "events").click[0]
  return [Object.keys(h).join(), h.type, h.origType, h.data, h.namespace, h.needsContext, h.selector]
})
S("$.event.add on non-accepting", (env) => {
  const { $, d } = env
  const t = d.createTextNode("x")
  $.event.add(t, "click", () => {})
  let r
  try { $.event.add(g(env, "p1"), "click", undefined) } catch (e) { r = e.name }
  return [$.hasData(t), r]
})
S("$.event.remove on element without data", (env) => {
  const { $ } = env
  $.event.remove(g(env, "p1"), "click")
  return $.hasData(g(env, "p1"))
})
S("$.event.dispatch direct", (env) => {
  const { $, w, H } = env
  const p = g(env, "p1")
  $("#p1").on("click", H("f", 5))
  const r = $.event.dispatch.call(p, new w.MouseEvent("click"), "extra")
  const r2 = $.event.dispatch.call(g(env, "p2"), new w.MouseEvent("click"))
  return [r, r2]
})
S("custom special hooks", (env) => {
  const { $, log } = env
  const L = (n) => function (...a) { log.push([n, env.id(this), a.map((x) => (x && x.type ? "ev:" + x.type : Array.isArray(x) ? "arr:" + x.join("|") : typeof x === "function" ? "fn" : env.ser(x)))]) }
  $.event.special.myev = {
    setup: L("setup"), teardown: L("teardown"),
    add: function (ho) { log.push(["add", ho.type, ho.origType, ho.namespace, ho.selector]) },
    remove: function (ho) { log.push(["remove", ho.type, ho.namespace]) },
    preDispatch: function (e) { log.push(["pre", e.type]) },
    postDispatch: function (e) { log.push(["post", e.type, e.result]) },
    trigger: function (e, x) { log.push(["trigger", e.type, x]) },
    _default: function (e, x) { log.push(["default", e.type, x, env.id(this)]) },
  }
  const f = function () { log.push("handler"); return "res" }
  $("#p1").on("myev.b.a", { d: 1 }, f).on("myev", "span", f)
  $("#s1").trigger("myev", ["X"])
  $("#p1").triggerHandler("myev")
  $("#p1").off("myev")
  return $.hasData(g(env, "p1"))
})
S("custom special trigger returns false", (env) => {
  const { $, log } = env
  $.event.special.blocked = { trigger: () => false, _default: () => log.push("default") }
  $("#p1").on("blocked", () => log.push("h"))
  $("#p1").trigger("blocked")
  $("#p1").triggerHandler("blocked")
  return true
})
S("custom special preDispatch false", (env) => {
  const { $, log } = env
  $.event.special.pd = { preDispatch: () => false, postDispatch: () => log.push("post") }
  $("#p1").on("pd", () => log.push("h"))
  return [$("#p1").triggerHandler("pd"), log.length]
})
S("custom special setup returning non-false skips addEventListener", (env) => {
  const { $, w, log } = env
  $.event.special.nat = { setup: () => true }
  $.event.special.nat2 = { setup: () => false }
  $("#p1").on("nat", () => log.push("nat")).on("nat2", () => log.push("nat2"))
  g(env, "p1").dispatchEvent(new w.Event("nat"))
  g(env, "p1").dispatchEvent(new w.Event("nat2"))
  return true
})
S("custom special teardown", (env) => {
  const { $, w, log } = env
  $.event.special.td = { teardown: () => true }
  $.event.special.td2 = { teardown: () => false }
  $("#p1").on("td", () => log.push("td")).on("td2", () => log.push("td2"))
  $("#p1").off("td td2")
  g(env, "p1").dispatchEvent(new w.Event("td"))
  g(env, "p1").dispatchEvent(new w.Event("td2"))
  return true
})
S("custom special bindType/delegateType", (env) => {
  const { $, H } = env
  $.event.special.alias = { bindType: "click", delegateType: "mouseover" }
  $("#p1").on("alias", H("direct")).on("alias", "span", H("deleg"))
  const d = dumpEvents(env, g(env, "p1"))
  $("#s1").trigger("click"); $("#s1").trigger("mouseover"); $("#p1").trigger("alias")
  $("#p1").off("alias")
  return [d, dumpEvents(env, g(env, "p1"))]
})
S("custom special handle", (env) => {
  const { $, log } = env
  $.event.special.hh = { handle: function (e, x) { log.push(["special handle", e.handleObj.origType, x]); return e.handleObj.handler.apply(this, arguments) } }
  $("#p1").on("hh", (e, x) => { log.push(["inner", x]); return "v" })
  return $("#p1").triggerHandler("hh", ["arg"])
})
S("custom special noBubble", (env) => {
  const { $, H } = env
  $.event.special.nb = { noBubble: true }
  $("#s1").on("nb", H("s1")); $("#p1").on("nb", H("p1"))
  $("#s1").trigger("nb")
  return true
})
S("special add rewriting handler", (env) => {
  const { $, log } = env
  $.event.special.wrap = {
    add: function (ho) {
      const orig = ho.handler
      ho.handler = function () { log.push("wrapped"); return orig.apply(this, arguments) }
    },
  }
  const f = () => log.push("orig")
  $("#p1").on("wrap", f)
  const h = $._data(g(env, "p1"), "events").wrap[0]
  $("#p1").trigger("wrap")
  $("#p1").off("wrap", f)
  return [h.handler.guid === f.guid, $.hasData(g(env, "p1"))]
})
S("$.removeEvent direct", (env) => {
  const { $, w, log } = env
  const h = () => log.push("h")
  g(env, "p1").addEventListener("x", h)
  $.removeEvent(g(env, "p1"), "x", h)
  $.removeEvent({}, "x", h)
  g(env, "p1").dispatchEvent(new w.Event("x"))
  return true
})
S("$.event.simulate", (env) => {
  const { $, w, H } = env
  $("#mid").on("focusin", H("mid")); $("#txt").on("focusin", H("txt"))
  const native = new w.FocusEvent("focus")
  $.event.simulate("focusin", g(env, "txt"), $.event.fix(native))
  return true
})
S("$.event.global", (env) => {
  const { $ } = env
  const before = Object.keys($.event.global).sort()
  $("#p1").on("click mouseenter focus", () => {}).on("custom", "span", () => {})
  return [before, Object.keys($.event.global).sort()]
})
S("guid assignment", (env) => {
  const { $ } = env
  const f1 = () => {}, f2 = () => {}
  const g0 = $.guid
  $("#p1").on("click", f1).on("click", f1).on("dblclick", f2).one("x", f2)
  const f3 = () => {}
  $("#p1").one("y", f3)
  return [f1.guid - g0, f2.guid - g0, f3.guid - g0, $.guid - g0]
})
S("handler this and args", (env) => {
  const { $, log } = env
  const o = { name: "o" }
  $("#p1").on("custom", function (e, ...rest) { log.push([env.id(this), rest.length, env.ser(rest)]) })
  $(o).on("custom", function (e, ...rest) { log.push([env.id(this), rest.length]) })
  $("#p1").trigger("custom", [1, [2], null, undefined])
  $(o).trigger("custom", "s")
  return true
})
S("handler return values and event.result chain", (env) => {
  const { $, log } = env
  $("#s1").on("custom", () => "a")
  $("#p1").on("custom", (e) => { log.push(["p1 sees", e.result]); return undefined })
  $("#mid").on("custom", (e) => { log.push(["mid sees", e.result]); return 0 })
  const e = $.Event("custom")
  $("#s1").trigger(e)
  return e.result
})
S("data on handlers", (env) => {
  const { $, log } = env
  const data = { k: 1 }
  $("#p1").on("custom", data, (e) => log.push(e.data === data)).on("custom", (e) => log.push(e.data))
  $("#p1").trigger("custom")
  return true
})
S("chain return values", (env) => {
  const { $ } = env
  const $p = $("#p1")
  return [$p.on("click", () => {}) === $p, $p.one("x", () => {}) === $p, $p.off("click") === $p, $p.trigger("x") === $p,
    $p.on("click") === $p, $p.on({}) === $p, $p.off({}) === $p]
})
S("on/one/off/trigger function lengths", (env) => {
  const { $ } = env
  return ["on", "one", "off", "trigger", "triggerHandler", "bind", "unbind", "delegate", "undelegate", "hover", "click", "blur", "contextmenu"].map((n) => [n, typeof $.fn[n], $.fn[n] && $.fn[n].length])
})
S("$.Event / $.event function lengths", (env) => {
  const { $ } = env
  return [$.Event.length, $.event.add.length, $.event.remove.length, $.event.trigger.length, $.event.dispatch.length, $.event.fix.length, $.event.simulate.length, $.removeEvent.length]
})

// ---------------------------------------------------------------- removal details
S("remove namespaces subset semantics", (env) => {
  const { $ } = env
  const $p = $("#p1")
  const f = () => {}
  const combos = ["click.a", "click.b", "click.a.b", "click.a.b.c", "click.c"]
  const out = []
  for (const off of ["click.a", "click.b.a", ".a.c", "click.c", ".c.b.a", "click"]) {
    combos.forEach((c) => $p.on(c, f))
    $p.off(off)
    out.push([off, ($._data(g(env, "p1"), "events") || { click: [] }).click.map((h) => h.namespace)])
    $p.off()
  }
  return out
})
S("remove mappedTypes via ns-only with mouseenter", (env) => {
  const { $ } = env
  const f = () => {}
  $("#p1").on("mouseenter.q", f).on("mouseover.q", f).on("click.q", f).on("mouseenter", "span", f)
  $("#p1").off(".q")
  const a = dumpEvents(env, g(env, "p1"))
  $("#p1").off("mouseenter", "**")
  return [a, dumpEvents(env, g(env, "p1"))]
})
S("remove delegated by selector string equality", (env) => {
  const { $ } = env
  const f = () => {}
  $("#p1").on("click", "span", f).on("click", "span ", f).on("click", "b", f)
  $("#p1").off("click", "span")
  return dumpEvents(env, g(env, "p1"))
})
S("hasData cleanup after last off", (env) => {
  const { $, d } = env
  const f = () => {}
  $("#p1").on("click", f)
  $("#p1").off("click", f)
  $(d).on("focusin", f)
  $(d).off("focusin")
  return [$.hasData(g(env, "p1")), $._data(g(env, "p1")), $.hasData(d), Object.keys($._data(d)).sort()]
})

// ---------------------------------------------------------------- deprecated/event
S("bind/unbind", (env) => {
  const { $, H } = env
  const f = H("f"), f2 = H("f2")
  const $p = $("#p1")
  const r = [$p.bind("click", f) === $p, $p.bind("custom", { d: 1 }, f2) === $p, $p.bind({ x: f, y: f2 }) === $p, $p.bind("z", false) === $p]
  $p.bind("dbl", f, undefined)
  const d1 = dumpEvents(env, g(env, "p1"), new Map([[f, "f"], [f2, "f2"]]))
  $p.trigger("click").trigger("custom").trigger("x").trigger("y").trigger("dbl")
  $p.unbind("click", f2); $p.unbind("custom", f2); $p.unbind("x"); $p.unbind("z", false)
  const d2 = dumpEvents(env, g(env, "p1"), new Map([[f, "f"], [f2, "f2"]]))
  $p.unbind()
  return [r, d1, d2, $.hasData(g(env, "p1"))]
})
S("bind with selector-like string data", (env) => {
  const { $, H } = env
  const f = H("f")
  $("#p1").bind("click", "span", f)
  $("#s1").trigger("click")
  $("#p1").trigger("click")
  return dumpEvents(env, g(env, "p1"), new Map([[f, "f"]]))
})
S("delegate/undelegate", (env) => {
  const { $, H } = env
  const f = H("f"), f2 = H("f2")
  const $m = $("#mid")
  const r = [$m.delegate("p", "click", f) === $m, $m.delegate("span", "click.ns", { d: 1 }, f2) === $m, $m.delegate("b", { click: f, mouseover: f2 }) === $m]
  const d1 = dumpEvents(env, g(env, "mid"), new Map([[f, "f"], [f2, "f2"]]))
  $("#s1").trigger("click"); $("#b1").trigger("mouseover")
  const out = [r, d1]
  $m.undelegate("p", "click")
  out.push(dumpEvents(env, g(env, "mid"), new Map([[f, "f"], [f2, "f2"]])))
  $m.undelegate(".ns")
  out.push(dumpEvents(env, g(env, "mid"), new Map([[f, "f"], [f2, "f2"]])))
  $m.undelegate("b", "click", f)
  out.push(dumpEvents(env, g(env, "mid"), new Map([[f, "f"], [f2, "f2"]])))
  $m.undelegate()
  out.push(dumpEvents(env, g(env, "mid"), new Map([[f, "f"], [f2, "f2"]])))
  return out
})
S("undelegate argument forms", (env) => {
  const { $ } = env
  const f = () => {}, f2 = () => {}
  const out = []
  const forms = [
    ($m) => $m.undelegate(),
    ($m) => $m.undelegate("click"),
    ($m) => $m.undelegate("p"),
    ($m) => $m.undelegate("p", "click"),
    ($m) => $m.undelegate("", "click"),
    ($m) => $m.undelegate(null, "click"),
    ($m) => $m.undelegate(undefined, "click"),
    ($m) => $m.undelegate(false, "click"),
    ($m) => $m.undelegate(f2, "click"),
    ($m) => $m.undelegate("p", "click", f2),
    ($m) => $m.undelegate(0, "click"),
    ($m) => $m.undelegate({ click: f }, undefined),
    ($m) => $m.undelegate(".ns"),
    ($m) => $m.undelegate("p", "click.ns", f),
  ]
  for (const form of forms) {
    const $m = $("#mid")
    $m.on("click", "p", f).on("click.ns", "span", f2).on("click", f).on("click", f2).on("mouseover", "p", f)
    let r
    try { r = form($m) === $m } catch (e) { r = "throw:" + e.name }
    out.push([r, dumpEvents(env, g(env, "mid"), new Map([[f, "f"], [f2, "f2"]]))])
    $m.off()
  }
  return out
})
S("hover", (env) => {
  const { $, w, H } = env
  const over = H("over"), out = H("out")
  const $p = $("#p1")
  const r = $p.hover(over, out) === $p
  $("#p2").hover(over)
  $("#other").hover(over, null)
  const d = [dumpEvents(env, g(env, "p1"), new Map([[over, "over"], [out, "out"]])), dumpEvents(env, g(env, "p2"), new Map([[over, "over"], [out, "out"]]))]
  $("#p1").trigger("mouseenter").trigger("mouseleave")
  $("#p2").trigger("mouseenter").trigger("mouseleave")
  g(env, "b1").dispatchEvent(new w.MouseEvent("mouseout", { bubbles: true, relatedTarget: g(env, "other") }))
  return [r, d, dumpEvents(env, g(env, "other"))]
})
S("hover with no args", (env) => {
  const { $ } = env
  let r
  try { r = $("#p1").hover() === undefined ? "u" : "jq" } catch (e) { r = e.name }
  return [r, dumpEvents(env, g(env, "p1"))]
})
const shortcutNames = "blur focus focusin focusout resize scroll click dblclick mousedown mouseup mousemove mouseover mouseout mouseenter mouseleave change select submit keydown keypress keyup contextmenu".split(" ")
S("shortcut methods exist", (env) => {
  const { $ } = env
  return shortcutNames.map((n) => typeof $.fn[n])
})
for (const name of shortcutNames) {
  if (name === "submit") continue
  S(`shortcut ${name}`, (env) => {
    const { $, H } = env
    const f = H("f", "r"), f2 = H("f2")
    const $p = $("#txt")
    const r = [$p[name](f) === $p, $p[name]({ d: 1 }, f2) === $p, $p[name](undefined) === $p, $p[name](null, f) === $p, $p[name]("dstr", f2) === $p]
    const d = dumpEvents(env, g(env, "txt"), new Map([[f, "f"], [f2, "f2"]]))
    const t = $p[name]() === $p
    return [r, d, t]
  })
}
S("shortcut submit bind + triggerHandler", (env) => {
  const { $, H } = env
  const f = H("f", false)
  $("#f1").submit(f)
  return [$("#f1").triggerHandler("submit"), dumpEvents(env, g(env, "f1"), new Map([[f, "f"]]))]
})
S("shortcut click trigger toggles checkbox", (env) => {
  const { $, log } = env
  $("#cb").click(function () { log.push(this.checked) })
  $("#cb").click()
  $("#cb").click()
  return g(env, "cb").checked
})
S("shortcut on empty set", (env) => {
  const { $ } = env
  return [$().click().length, $().click(() => {}).length, $().hover(() => {}).length, $().bind("x", () => {}).length, $().undelegate().length]
})
S("shortcut false handler", (env) => {
  const { $ } = env
  $("#p1").click(false)
  const e = $.Event("click")
  $("#p1").trigger(e)
  return [e.isDefaultPrevented(), e.isPropagationStopped()]
})

// ---------------------------------------------------------------- misc interplay
S("clone(true) copies events", (env) => {
  const { $, H } = env
  const f = H("f")
  $("#p1").on("click", f).on("click", "span", { d: 1 }, f).on("custom.ns", f)
  const $c = $("#p1").clone(true).attr("id", "p1c").appendTo("#other")
  $c.find("span").attr("id", "s1c").trigger("click")
  $c.trigger("custom")
  return dumpEvents(env, $c[0], new Map([[f, "f"]]))
})
S("remove() cleans events", (env) => {
  const { $ } = env
  const p = g(env, "p1")
  $("#p1").on("click", () => {})
  $("#s1").on("focus", () => {})
  $("#p1").remove()
  return [$.hasData(p), $.hasData(g(env, "outer"))]
})
S("empty() and html() clean events", (env) => {
  const { $ } = env
  const s = g(env, "s1")
  $(s).on("click", () => {})
  $("#p1").empty()
  return $.hasData(s)
})
S("event on window resize/scroll triggers", (env) => {
  const { $, w, H } = env
  $(w).on("resize", H("resize")).on("scroll", H("scroll"))
  $(w).trigger("resize")
  $(w).resize()
  w.dispatchEvent(new w.Event("scroll"))
  return true
})
S("event handler throws propagates", (env) => {
  const { $, log } = env
  $("#p1").on("custom", () => { throw new Error("boom") }).on("custom", () => log.push("after"))
  let err
  try { $("#p1").trigger("custom") } catch (e) { err = e.message }
  return [err, $.event.triggered]
})
S("nested trigger inside handler", (env) => {
  const { $, log } = env
  $("#p1").on("a", () => { log.push("a start"); $("#p2").trigger("b"); log.push("a end") })
  $("#p2").on("b", (e) => { log.push(["b", e.type]) })
  $("#p1").trigger("a")
  return true
})
S("event type changes mid-dispatch", (env) => {
  const { $, log } = env
  $("#p1").on("custom", (e) => { e.type = "changed" }).on("custom", (e) => log.push(e.type))
  $("#mid").on("custom", (e) => log.push("mid " + e.type)).on("changed", () => log.push("mid changed"))
  $("#p1").trigger("custom")
  return true
})
S("on with handler that has handler prop", (env) => {
  const { $, log } = env
  const f = function () { log.push("f") }
  const g2 = function () { log.push("g") }
  g2.handler = f
  $("#p1").on("click", g2)
  $("#p1").trigger("click")
  return dumpEvents(env, g(env, "p1"), new Map([[f, "f"], [g2, "g2"]]))
})
S("isTrigger during native vs trigger", (env) => {
  const { $, w, log } = env
  $("#p1").on("click", (e) => log.push(["it", e.isTrigger === undefined ? "u" : e.isTrigger]))
  $("#p1").trigger("click")
  $("#p1").triggerHandler("click")
  g(env, "p1").dispatchEvent(new w.MouseEvent("click"))
  return true
})
S("trigger array-like jQuery data", (env) => {
  const { $, log } = env
  $("#p1").on("custom", (e, a, b) => log.push([env.id(a), env.id(b)]))
  $("#p1").trigger("custom", $("#p2, #other"))
  return true
})
S("stopImmediatePropagation delegated and direct", (env) => {
  const { $, log } = env
  $("#mid").on("click", "p", (e) => { log.push("d1"); e.stopImmediatePropagation() }).on("click", "p", () => log.push("d2")).on("click", () => log.push("direct"))
  $("#outer").on("click", () => log.push("outer"))
  $("#s1").trigger("click")
  return true
})
S("off during delegated dispatch", (env) => {
  const { $, log } = env
  const $m = $("#mid")
  $m.on("click", "p", () => { log.push("d1"); $m.off("click", "**") }).on("click", "span", () => log.push("d2")).on("click", () => log.push("direct"))
  $("#s1").trigger("click")
  $("#s1").trigger("click")
  return true
})
S("focus special on non-focusable plain object", (env) => {
  const { $, log } = env
  const o = { name: "o" }
  $(o).on("focus", () => log.push("focus"))
  $(o).trigger("focus")
  return $(o).triggerHandler("focus")
})
S("click on checkbox inside handler checks state via native click", (env) => {
  const { $, log } = env
  $("#rd").on("click", function (e) { log.push(["rd", this.checked, e.isTrigger]) })
  g(env, "rd").click()
  $("#rd").trigger("click")
  return g(env, "rd").checked
})
S("trigger click stopPropagation with native default", (env) => {
  const { $, log } = env
  $("#cb").on("click", (e) => { log.push("cb"); e.stopPropagation() })
  $("#mid").on("click", () => log.push("mid"))
  g(env, "mid").addEventListener("click", () => log.push("native mid"))
  $("#cb").trigger("click")
  return g(env, "cb").checked
})
S("trigger btn2 click stopPropagation native default", (env) => {
  const { $, log } = env
  $("#btn2").on("click", (e) => { log.push("btn2"); e.stopPropagation() })
  $("#mid").on("click", () => log.push("mid"))
  g(env, "mid").addEventListener("click", () => log.push("native mid"))
  g(env, "btn2").addEventListener("click", () => log.push("native btn2"))
  $("#btn2").trigger("click")
  return true
})
S("onfoo temporarily removed during native call", (env) => {
  const { $, log } = env
  const b = g(env, "btn2")
  b.onclick = () => { log.push("onclick") }
  b.addEventListener("click", () => log.push(["native", typeof b.onclick]))
  $("#btn2").trigger("click")
  return typeof b.onclick
})
S("event.triggered during native call", (env) => {
  const { $, log } = env
  const b = g(env, "btn2")
  $("#btn2").on("click", () => log.push("jq"))
  b.addEventListener("click", () => log.push(["native", $.event.triggered]))
  $("#btn2").trigger("click")
  return $.event.triggered
})

// ---------------------------------------------------------------- extra coverage
S("one with multiple types", (env) => {
  const { $, H } = env
  const f = H("f")
  $("#p1").one("click dblclick", f)
  $("#p1").trigger("click").trigger("click").trigger("dblclick").trigger("dblclick")
  return dumpEvents(env, g(env, "p1"))
})
S("one delegated + stopImmediatePropagation", (env) => {
  const { $, log } = env
  $("#mid").one("click", "p", (e) => { log.push("one"); e.stopImmediatePropagation() }).on("click", "p", () => log.push("on"))
  $("#s1").trigger("click"); $("#s1").trigger("click")
  return dumpEvents(env, g(env, "mid")).click.handlers.length
})
S("off with undispatched jQuery.Event", (env) => {
  const { $ } = env
  const f = () => {}
  $("#p1").on("click", f).on("preventDefault", f).on("type", f)
  let r
  try { r = $("#p1").off($.Event("click")) === undefined ? "u" : "jq" } catch (e) { r = "throw:" + e.name }
  return [r, dumpEvents(env, g(env, "p1"))]
})
S("triggerHandler with no handlers", (env) => {
  const { $ } = env
  return [$("#p1").triggerHandler("click"), $("#p1").triggerHandler("custom"), $("#txt").triggerHandler("focus"), g(env, "txt") === env.d.activeElement]
})
S("window trigger/triggerHandler", (env) => {
  const { $, w, H } = env
  $(w).on("custom", H("win", 3))
  return [$(w).triggerHandler("custom"), $(w).trigger("custom").length]
})
S("on null selector string data", (env) => {
  const { $, H } = env
  $("#p1").on("click", null, "str", H("f"))
  $("#p1").trigger("click")
  return dumpEvents(env, g(env, "p1"))
})
S("$.Event null/undefined props", (env) => {
  const { $ } = env
  const a = $.Event("x", null), b = $.Event("y", undefined), c = $.Event("z", false)
  return [a.type, b.type, c.type]
})
S("trigger empty data array", (env) => {
  const { $, H } = env
  $("#p1").on("custom", H("f"))
  $("#p1").trigger("custom", [])
  return true
})
S("delegated click on checkbox leverages native", (env) => {
  const { $, log } = env
  $("#mid").on("click", "input", function (e) { log.push(["deleg", this.id, this.checked, e.isTrigger]) })
  $("#cb").trigger("click")
  return [g(env, "cb").checked, env.ser(dumpEvents(env, g(env, "cb")))]
})
S("focus trigger when already focused", (env) => {
  const { $, d, log } = env
  g(env, "txt").focus()
  $("#txt").on("focus", (e) => log.push(["focus", e.isTrigger]))
  $("#txt").trigger("focus")
  return d.activeElement.id
})
S("namespaced focus trigger", (env) => {
  const { $, d, log } = env
  $("#txt").on("focus.a", (e) => log.push(["a", e.namespace])).on("focus.b", (e) => log.push(["b", e.namespace]))
  $("#txt").trigger("focus.a")
  return d.activeElement && d.activeElement.id
})
S("blur trigger on focused", (env) => {
  const { $, d, log } = env
  $("#txt").on("blur", (e) => log.push(["blur", e.isTrigger, e.type]))
  $("#mid").on("focusout", (e) => log.push(["focusout", e.target.id]))
  g(env, "txt").focus()
  $("#txt").trigger("blur")
  return d.activeElement && d.activeElement.id
})
S("one mouseenter delegated", (env) => {
  const { $, w, H } = env
  $("#mid").one("mouseenter", "p", H("enter"))
  const s1 = g(env, "s1"), mid = g(env, "mid"), p2 = g(env, "p2")
  s1.dispatchEvent(new w.MouseEvent("mouseover", { bubbles: true, relatedTarget: p2 }))
  s1.dispatchEvent(new w.MouseEvent("mouseover", { bubbles: true, relatedTarget: mid }))
  return dumpEvents(env, mid)
})
S("custom special _default this and args", (env) => {
  const { $, log } = env
  $.event.special.dd = { _default: function (e, a) { log.push(["default", env.id(this), e.type, a, e.isTrigger]); return false } }
  g(env, "p1").dd = () => log.push("native dd")
  $("#p1").trigger("dd", ["x"])
  $("#p1").triggerHandler("dd")
  return true
})
S("simulate copies event props", (env) => {
  const { $, w, log } = env
  $("#mid").on("focusin", (e) => log.push([e.type, e.isSimulated, e.relatedTarget, e.detail, e.originalEvent && e.originalEvent.type, e.target.id]))
  g(env, "txt").dispatchEvent(new w.FocusEvent("focus", { detail: 4 }))
  return true
})
S("event.data per handler with delegation and map", (env) => {
  const { $, log } = env
  $("#mid").on({ click: (e) => log.push(["c", env.ser(e.data)]), custom: (e) => log.push(["cu", env.ser(e.data)]) }, "p", { m: 1 })
  $("#s1").trigger("click").trigger("custom")
  return true
})
S("jQuery.event.global and special are shared objects", (env) => {
  const { $ } = env
  const sp = $.event.special
  sp.zz = { bindType: "click" }
  const log = []
  $("#p1").on("zz", () => log.push("zz"))
  $("#p1").trigger("click")
  return [log, Object.keys($.event.global).sort()]
})
S("removing all handlers inside handler", (env) => {
  const { $, log } = env
  $("#p1").on("click", () => { log.push("1"); $("#p1").off() }).on("click", () => log.push("2"))
  $("#p1").trigger("click").trigger("click")
  return $.hasData(g(env, "p1"))
})
