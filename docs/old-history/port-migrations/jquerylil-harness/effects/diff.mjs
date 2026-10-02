// Differential harness for the effects group (queue, delay, Tween, animatedSelector, effects).
// usage: node diff.mjs <our dist/jquery.esm.js> [--verbose] [--only substring]
// Every case runs twice, each time in a fresh JSDOM with a fresh jQuery instance and a fresh
// fake clock: once with official jquery@3.7.1 (dist/jquery.js evaluated in a function scope)
// and once with our ESM build (imported with a cache-busting query). Results are serialized
// and compared. Exit status 0; the summary line lists the mismatching case names.
import { createRequire } from "node:module"
import { readFileSync } from "node:fs"
import { pathToFileURL } from "node:url"
import { resolve } from "node:path"

const require = createRequire("/home/azureuser/jquerylil/package.json")
const { JSDOM } = require("jsdom")
const officialSrc = readFileSync(require.resolve("jquery/dist/jquery.js"), "utf8")
const ourPath = resolve(process.argv[2])
const verbose = process.argv.includes("--verbose")
const onlyIdx = process.argv.indexOf("--only")
const only = onlyIdx > 0 ? process.argv[onlyIdx + 1] : null

// ---------------------------------------------------------------- fake clock
const clock = { now: 0, timers: new Map(), nextId: 1 }
const realDateNow = Date.now
Date.now = () => clock.now
function addTimer(fn, ms) {
  const id = clock.nextId++
  let d = Number(ms)
  if (!(d > 0)) d = 0
  clock.timers.set(id, { id, due: clock.now + d, fn })
  return id
}
function nextTimer(limit) {
  let best = null
  for (const t of clock.timers.values()) {
    if (t.due <= limit && (!best || t.due < best.due || (t.due === best.due && t.id < best.id))) best = t
  }
  return best
}
function advance(ms) {
  const target = clock.now + ms
  for (let guard = 0; guard < 200000; guard++) {
    const t = nextTimer(target)
    if (!t) break
    clock.timers.delete(t.id)
    if (t.due > clock.now) clock.now = t.due
    t.fn()
  }
  clock.now = target
}
function pending() {
  return clock.timers.size
}

// ---------------------------------------------------------------- environments
let serial = 0
function freshDom() {
  const dom = new JSDOM("<!doctype html><html><head></head><body></body></html>", { pretendToBeVisual: true })
  const win = dom.window
  win.setTimeout = (fn, ms, ...args) => addTimer(() => (typeof fn === "function" ? fn(...args) : 0), ms)
  win.clearTimeout = (id) => { clock.timers.delete(id) }
  win.requestAnimationFrame = (fn) => addTimer(() => fn(clock.now), 16)
  win.cancelAnimationFrame = (id) => { clock.timers.delete(id) }
  globalThis.window = win
  globalThis.document = win.document
  clock.now = 1000000
  clock.timers.clear()
  return win
}
function loadOfficial() {
  const win = freshDom()
  const module = { exports: {} }
  new Function("module", "exports", "window", officialSrc)(module, module.exports, win)
  return { $: module.exports, win }
}
async function loadOurs() {
  const win = freshDom()
  const mod = await import(pathToFileURL(ourPath).href + "?run=" + serial++)
  return { $: mod.jQuery, win }
}

// ---------------------------------------------------------------- serialization
function ser(v, depth = 0, seen = new Set()) {
  if (v === undefined) return "<undef>"
  if (v === null) return null
  const t = typeof v
  if (t === "number") {
    if (Number.isNaN(v)) return "<NaN>"
    if (!Number.isFinite(v)) return v > 0 ? "<Inf>" : "<-Inf>"
    if (Object.is(v, -0)) return "<-0>"
    return Math.round(v * 1e9) / 1e9
  }
  if (t === "string" || t === "boolean") return v
  if (t === "function") return "<fn>"
  if (t === "symbol") return "<symbol>"
  if (typeof v.nodeType === "number") {
    if (v.nodeType === 9) return "<document>"
    return "<node " + (v.nodeName || "") + (v.id ? "#" + v.id : "") + ">"
  }
  if (v.window === v) return "<window>"
  if (seen.has(v)) return "<cycle>"
  if (depth > 4) return "<deep>"
  seen.add(v)
  let out
  if (v.jquery && typeof v.length === "number" && typeof v.each === "function") {
    out = { jq: Array.from({ length: v.length }, (_, i) => ser(v[i], depth + 1, seen)) }
  } else if (Array.isArray(v)) {
    out = v.map((x) => ser(x, depth + 1, seen))
  } else {
    out = {}
    for (const k of Object.keys(v).sort()) out[k] = ser(v[k], depth + 1, seen)
  }
  seen.delete(v)
  return out
}

// ---------------------------------------------------------------- case context
function makeCtx($, win) {
  const doc = win.document
  const log = []
  const ctx = {
    $, win, doc, log, advance, pending,
    body: doc.body,
    el(html) {
      const holder = doc.createElement("div")
      holder.innerHTML = html
      const first = holder.firstElementChild
      doc.body.appendChild(first)
      return first
    },
    rec(label) {
      return function (...args) {
        log.push([label, ser(this), ...args.map((a) => ser(a))])
      }
    },
    push(...items) { log.push(items.map((a) => ser(a))) },
    st(el, ...props) {
      return props.map((p) => el.style[p])
    },
  }
  return ctx
}

async function runOne(loader, fn) {
  const { $, win } = await loader()
  const ctx = makeCtx($, win)
  let out
  try {
    out = await fn(ctx)
  } catch (e) {
    out = { threw: e && e.constructor ? e.constructor.name : String(e) }
  }
  let result
  try {
    result = JSON.stringify(ser({ out, log: ctx.log }))
  } catch (e) {
    result = "<unserializable " + e + ">"
  }
  // drop the environment: stop timers so nothing leaks into the next case
  clock.timers.clear()
  return result
}

// ---------------------------------------------------------------- cases
const cases = []
const C = (name, fn) => cases.push([name, fn])

// ===== jQuery.queue / jQuery.dequeue / jQuery._queueHooks
C("queue: static get on fresh element", ({ $, el }) => {
  const e = el("<div></div>")
  return [$.queue(e), $.queue(e, "fx"), $.queue(e, "foo"), $.queue(e, "fx").length]
})
C("queue: static on null/undefined elem", ({ $ }) => [$.queue(null), $.queue(undefined, "fx", () => {}), $.queue(0)])
C("queue: static push functions", ({ $, el, rec }) => {
  const e = el("<div></div>")
  const f1 = rec("f1"), f2 = rec("f2")
  const q1 = $.queue(e, "foo", f1)
  const q2 = $.queue(e, "foo", f2)
  return [q1 === q2, q1.length, q1[0] === f1, q1[1] === f2]
})
C("queue: static array replaces", ({ $, el, rec }) => {
  const e = el("<div></div>")
  const f1 = rec("f1"), f2 = rec("f2"), f3 = rec("f3")
  $.queue(e, "foo", f1)
  const arr = [f2, f3]
  const q = $.queue(e, "foo", arr)
  return [q.length, q === arr, q[0] === f2, $.queue(e, "foo").length]
})
C("queue: static non-function data", ({ $, el }) => {
  const e = el("<div></div>")
  const a = $.queue(e, "foo", "str")
  const b = $.queue(e, "foo", 5)
  const c = $.queue(e, "foo", { a: 1 })
  return [a.length, b.length, c.length, $.queue(e, "foo")]
})
C("queue: static type variants", ({ $, el }) => {
  const e = el("<div></div>")
  const r = []
  for (const type of ["", undefined, null, 0, false, 5, "fx", "custom"]) {
    const f = function () {}
    $.queue(e, type, f)
    r.push([String(type), $.queue(e, "fx").length, $.queue(e, type).length, $.queue(e, "5").length])
  }
  return r
})
C("queue: static data falsy", ({ $, el, rec }) => {
  const e = el("<div></div>")
  $.queue(e, "foo", rec("a"))
  return [$.queue(e, "foo", null).length, $.queue(e, "foo", 0).length, $.queue(e, "foo", "").length, $.queue(e, "foo", false).length]
})
C("dequeue: static runs with this, next, hooks", ({ $, el, log, push }) => {
  const e = el("<div id='d1'></div>")
  const seen = []
  $.queue(e, "foo", function (next, hooks) {
    seen.push(this === e, typeof next, typeof hooks, typeof hooks.empty, typeof hooks.empty.fire)
    push("first", arguments.length)
    next()
  })
  $.queue(e, "foo", function (next, hooks) {
    push("second", this === e, hooks === $._queueHooks(e, "foo"))
  })
  $.dequeue(e, "foo")
  return [seen, $.queue(e, "foo").length, $.hasData(e)]
})
C("dequeue: empty queue fires empty and cleans data", ({ $, el, push }) => {
  const e = el("<div></div>")
  $.queue(e, "foo", function (next) { push("run"); next() })
  const hooks = $._queueHooks(e, "foo")
  hooks.empty.add(() => push("empty fired"))
  $.dequeue(e, "foo")
  const d = $._data(e)
  return [Object.keys(d).sort(), $.queue(e, "foo").length]
})
C("dequeue: fx inprogress sentinel", ({ $, el, push }) => {
  const e = el("<div></div>")
  let saved
  $.queue(e, "fx", function (next) { push("a"); saved = next })
  $.queue(e, "fx", function (next) { push("b"); next() })
  const before = $.queue(e, "fx").slice()
  $.dequeue(e)
  const mid = $.queue(e, "fx").slice()
  saved()
  const after = $.queue(e, "fx").slice()
  return [before.length, mid, after, $._data(e)]
})
C("dequeue: type default and odd types", ({ $, el, push }) => {
  const e = el("<div></div>")
  $.queue(e, "fx", function () { push("fx ran", this === e) })
  $.dequeue(e, "")
  $.queue(e, "fx", function () { push("fx ran2") })
  $.dequeue(e, null)
  $.queue(e, "fx", function () { push("fx ran3") })
  $.dequeue(e, undefined)
  $.queue(e, "7", function () { push("7 ran") })
  $.dequeue(e, 7)
  $.queue(e, "fx", function () { push("fx ran4") })
  $.dequeue(e, 0)
  return $.queue(e, "fx")
})
C("dequeue: nothing queued", ({ $, el, push }) => {
  const e = el("<div></div>")
  const hooks = $._queueHooks(e, "bar")
  hooks.empty.add(() => push("empty"))
  $.dequeue(e, "bar")
  return [$.hasData(e), Object.keys($._data(e))]
})
C("dequeue: hooks.stop deleted before call", ({ $, el, push }) => {
  const e = el("<div></div>")
  const hooks = $._queueHooks(e, "foo")
  hooks.stop = () => push("stop")
  $.queue(e, "foo", function (next, h) { push("has stop", "stop" in h) })
  $.dequeue(e, "foo")
  return "stop" in hooks
})
C("_queueHooks: shape and identity", ({ $, el }) => {
  const e = el("<div></div>")
  const h1 = $._queueHooks(e, "fx")
  const h2 = $._queueHooks(e, "fx")
  const h3 = $._queueHooks(e, "foo")
  const h4 = $._queueHooks(e)
  return [h1 === h2, h1 === h3, Object.keys(h1), typeof h1.empty.add, Object.keys($._data(e)).sort(), h4 === h1]
})
C("_queueHooks: empty memory callback", ({ $, el, push }) => {
  const e = el("<div></div>")
  const h = $._queueHooks(e, "foo")
  $.data(e, "x", 1)
  h.empty.fire()
  h.empty.add(() => push("late add fires"))
  return [$._data(e), h.empty.fired()]
})

// ===== .queue / .dequeue / .clearQueue
C("fn.queue: getter forms", ({ $, el }) => {
  const e = el("<div></div>")
  const $e = $(e)
  $.queue(e, "foo", function () {})
  return [$e.queue(), $e.queue("foo").length, $e.queue("fx"), $().queue(), $().queue("foo"), $e.queue(undefined) === $e]
})
C("fn.queue: fx autostart", ({ $, el, push }) => {
  const e = el("<div></div>")
  const r = $(e).queue(function (next, hooks) {
    push("run", this === e, typeof next, typeof hooks)
  })
  $(e).queue(function (next) { push("second") })
  return [r.length, $(e).queue(), $(e).queue().length]
})
C("fn.queue: custom type does not autostart", ({ $, el, push }) => {
  const e = el("<div></div>")
  const $e = $(e)
  const r = $e.queue("foo", function (next) { push("foo ran"); next() })
  const q = $e.queue("foo")
  $e.dequeue("foo")
  return [r === $e, q.length, $e.queue("foo").length]
})
C("fn.queue: replace with array", ({ $, el, push }) => {
  const e = el("<div></div>")
  const $e = $(e)
  $e.queue("foo", function () { push("x") })
  const r = $e.queue("foo", [function (n) { push("a"); n() }, function () { push("b") }])
  $e.dequeue("foo")
  return [r === $e, $e.queue("foo").length]
})
C("fn.queue: data undefined returns this", ({ $, el }) => {
  const e = el("<div></div>")
  const $e = $(e)
  return [$e.queue("foo", undefined) === $e, $e.queue("fx", undefined) === $e, $.hasData(e)]
})
C("fn.queue: null data on fx", ({ $, el, push }) => {
  const e = el("<div></div>")
  const $e = $(e)
  $.queue(e, "fx", function (n) { push("queued fn") })
  const r = $e.queue(null)
  return [r === $e, $e.queue()]
})
C("fn.queue: non-string type", ({ $, el, push }) => {
  const e = el("<div></div>")
  const $e = $(e)
  const r1 = $e.queue(5)
  return [r1, $e.queue().length]
})
C("fn.queue: multiple elements", ({ $, el, push }) => {
  const a = el("<p id='a'></p>"), b = el("<p id='b'></p>")
  const $s = $([a, b])
  const r = $s.queue("foo", function (n) { push("run", this.id); n() })
  const len = $s.queue("foo").length
  $s.dequeue("foo")
  return [r === $s, len, $.queue(b, "foo").length]
})
C("fn.queue: empty set", ({ $, push }) => {
  const $s = $()
  return [$s.queue("foo", () => push("never")) === $s, $s.queue(), $s.dequeue() === $s, $s.clearQueue() === $s]
})
C("fn.queue: arity via arguments", ({ $, el }) => {
  const e = el("<div></div>")
  const $e = $(e)
  $.queue(e, "fx", function () {})
  return [$e.queue("fx").length, Array.isArray($e.queue("fx")), $e.queue(undefined, undefined) === $e, $e.queue("x", undefined, 3) === $e]
})
C("fn.dequeue: default fx and types", ({ $, el, push }) => {
  const e = el("<div></div>")
  const $e = $(e)
  $.queue(e, "fx", function (n) { push("fx1") })
  $.queue(e, "foo", function (n) { push("foo1") })
  const r = $e.dequeue()
  $e.dequeue("foo")
  $e.dequeue(null)
  return [r === $e, $e.queue(), $e.queue("foo")]
})
C("fn.clearQueue", ({ $, el, push }) => {
  const e = el("<div></div>")
  const $e = $(e)
  $.queue(e, "foo", function () {})
  $.queue(e, "foo", function () {})
  $.queue(e, "fx", function () {})
  $.queue(e, "fx", function () {})
  const r = $e.clearQueue("foo")
  const a = $e.queue("foo").length
  $e.clearQueue()
  const b = $e.queue("fx").length
  $.queue(e, "fx", function () {})
  $e.clearQueue("")
  const c = $e.queue("fx").length
  $.queue(e, "fx", function () {})
  $e.clearQueue(null)
  return [r === $e, a, b, c, $e.queue("fx").length]
})

// ===== .promise
C("fn.promise: resolves immediately when idle", ({ $, el, push }) => {
  const a = el("<p id='a'></p>")
  const $s = $(a)
  const p = $s.promise()
  p.done(function (arg) { push("done", this === $s, arg === $s) })
  return [p.state(), typeof p.promise, typeof p.resolve]
})
C("fn.promise: waits for fx queue", ({ $, el, push, advance }) => {
  const a = el("<p id='a'></p>"), b = el("<p id='b'></p>")
  const $s = $([a, b])
  const nexts = []
  $s.queue(function (n) { nexts.push(n) })
  const p = $s.promise()
  p.done(function (arg) { push("done", this === $s) })
  const s1 = p.state()
  nexts[0]()
  const s2 = p.state()
  nexts[1]()
  return [s1, s2, p.state()]
})
C("fn.promise: custom type and target", ({ $, el, push }) => {
  const a = el("<p id='a'></p>")
  const $s = $(a)
  let nx
  $s.queue("foo", function (n) { nx = n })
  $s.dequeue("foo")
  const target = { mine: 1 }
  const p = $s.promise("foo", target)
  p.done(() => push("foo done"))
  const s1 = p.state()
  nx()
  return [p === target, s1, p.state(), target.mine]
})
C("fn.promise: object as first arg", ({ $, el }) => {
  const a = el("<p></p>")
  const target = {}
  const p = $(a).promise(target)
  return [p === target, typeof target.done, p.state()]
})
C("fn.promise: empty-string type", ({ $, el, push }) => {
  const a = el("<p></p>")
  let nx
  $(a).queue(function (n) { nx = n })
  const p = $(a).promise("")
  const s1 = p.state()
  nx()
  return [s1, p.state()]
})
C("fn.promise: empty set and non-string type", ({ $, push }) => {
  const p = $().promise(5)
  const p2 = $().promise(null, {})
  return [p.state(), p2.state()]
})
C("fn.promise: with running animation", ({ $, el, push, advance }) => {
  const e = el("<div style='width:10px'></div>")
  $(e).animate({ width: 50 }, 100)
  const p = $(e).promise()
  p.done(() => push("anim promise done", e.style.width))
  const s1 = p.state()
  advance(200)
  return [s1, p.state(), e.style.width]
})

// ===== .delay
C("delay: number", ({ $, el, push, advance }) => {
  const e = el("<div></div>")
  const r = $(e).delay(100).queue(function (n) { push("after delay", Date.now()); n() })
  advance(50)
  push("t50")
  advance(60)
  return [r.length, $(e).queue().length]
})
C("delay: named speeds", ({ $, el, push, advance }) => {
  const e = el("<div></div>")
  const times = []
  const t0 = Date.now()
  $(e).delay("slow").queue(function (n) { times.push(Date.now() - t0); n() })
    .delay("fast").queue(function (n) { times.push(Date.now() - t0); n() })
    .delay("_default").queue(function (n) { times.push(Date.now() - t0); n() })
    .delay("bogus").queue(function (n) { times.push(Date.now() - t0); n() })
    .delay("300").queue(function (n) { times.push(Date.now() - t0); n() })
    .delay().queue(function (n) { times.push(Date.now() - t0); n() })
    .delay(null).queue(function (n) { times.push(Date.now() - t0); n() })
    .delay(-5).queue(function (n) { times.push(Date.now() - t0); n() })
  advance(5000)
  return times
})
C("delay: custom queue type", ({ $, el, push, advance }) => {
  const e = el("<div></div>")
  $(e).delay(100, "foo").queue("foo", function (n) { push("foo after", Date.now()); n() })
  const q1 = $(e).queue("foo").length
  $(e).dequeue("foo")
  advance(150)
  $(e).delay(10, "").queue(function (n) { push("fx after", Date.now()); n() })
  advance(50)
  $(e).delay(10, null).queue(function (n) { push("fx after2", Date.now()); n() })
  advance(50)
  return q1
})
C("delay: stop clears timeout", ({ $, el, push, advance }) => {
  const e = el("<div></div>")
  $(e).delay(100).queue(function (n) { push("should not run"); n() })
  advance(10)
  $(e).stop(true)
  advance(200)
  return [$(e).queue().length, pending()]
})
C("delay: speeds modified", ({ $, el, push, advance }) => {
  const e = el("<div></div>")
  $.fx.speeds.custom = 42
  const t0 = Date.now()
  $(e).delay("custom").queue(function (n) { push("at", Date.now() - t0); n() })
  advance(100)
  delete $.fx.speeds.custom
  return 1
})
C("delay: this and return", ({ $, el }) => {
  const e = el("<div></div>")
  const $e = $(e)
  return [$e.delay(1) === $e, $().delay(5).length]
})

// ===== jQuery.speed
const speedArgs = [
  [], [400], [0], ["slow"], ["fast"], ["_default"], ["bogus"], [null], [undefined], [false], [true],
  ["fn"], [400, "fn"], [400, "linear"], [400, "linear", "fn"], ["slow", "fn", "fn2"], [undefined, "linear"],
  [undefined, undefined, "fn"], [null, "fn"], [{}], [{ duration: 100 }], [{ duration: "slow" }],
  [{ duration: "nope", easing: "linear", queue: false }], [{ queue: true }], [{ queue: "foo" }],
  [{ queue: null }], [{ queue: "" }], [{ complete: "fn" }], [{ duration: 50, extra: 1 }], ["300"], [NaN], [-1],
  [400, null, "fn"], [400, "", "fn"], [400, 0], [400, "fn", undefined], ["fn", "linear"],
]
for (const args of speedArgs) {
  C("speed: " + JSON.stringify(args), ({ $ }) => {
    const fns = { fn: function () {}, fn2: function () {} }
    const real = args.map((a) => {
      if (a === "fn" || a === "fn2") return fns[a]
      if (a && typeof a === "object") {
        const o = { ...a }
        if (o.complete === "fn") o.complete = fns.fn
        return o
      }
      return a
    })
    const out = $.speed(...real)
    const off = (() => { $.fx.off = true; const r = $.speed(...real); $.fx.off = false; return r })()
    return {
      keys: Object.keys(out).sort(),
      out,
      completeIsFn: typeof out.complete,
      oldIsFn: out.old === fns.fn ? "fn" : out.old === fns.fn2 ? "fn2" : ser(out.old),
      same: real[0] && typeof real[0] === "object" ? out === real[0] : null,
      off: off.duration,
    }
  })
}
C("speed: complete calls old and dequeues", ({ $, el, push }) => {
  const e = el("<div></div>")
  const opt = $.speed(100, function () { push("old", this === e) })
  $.queue(e, "fx", function (n) { push("queued") })
  opt.complete.call(e)
  const opt2 = $.speed({ queue: false, complete: function () { push("old2") } })
  opt2.complete.call(e)
  const opt3 = $.speed({ queue: "foo", complete: 5 })
  $.queue(e, "foo", function () { push("foo dequeued") })
  opt3.complete.call(e)
  return [opt.complete.length, typeof opt.complete.prototype]
})

// ===== Tween
C("Tween: construct and fields", ({ $ }) => {
  const obj = { x: 5 }
  const t = $.Tween(obj, { duration: 100 }, "x", 10, "linear")
  return [Object.keys(t), t instanceof $.Tween, t instanceof $.fx, t.constructor === $.Tween, t.elem === obj, t.prop, t.start, t.now, t.end, t.unit, t.easing, t.pos]
})
C("Tween: defaults", ({ $, el }) => {
  const obj = { x: 5 }
  const t = $.Tween(obj, {}, "x", 10)
  const e = el("<div style='opacity:0.5;width:10px'></div>")
  const t2 = $.Tween(e, {}, "opacity", 1)
  const t3 = $.Tween(e, {}, "width", 1, undefined, "em")
  const t4 = $.Tween(e, {}, "height", 1, "")
  return [t.easing, t.unit, t2.unit, t2.start, t3.unit, t3.start, t4.start, t4.easing, $.Tween.length]
})
C("Tween: run with duration", ({ $ }) => {
  const obj = { x: 0 }
  const t = $.Tween(obj, { duration: 100 }, "x", 100, "swing")
  const r = []
  for (const p of [0, 0.25, 0.5, 1]) { t.run(p); r.push([t.pos, t.now, obj.x]) }
  return r
})
C("Tween: run without duration", ({ $ }) => {
  const obj = { x: 0 }
  const t = $.Tween(obj, {}, "x", 100, "swing")
  t.run(0.3)
  return [t.pos, t.now, obj.x]
})
C("Tween: run string numbers", ({ $ }) => {
  const obj = { x: "10" }
  const t = $.Tween(obj, { duration: 10 }, "x", "20", "linear")
  t.run(0.5)
  return [t.now, obj.x, t.start, t.end]
})
C("Tween: step option", ({ $, rec }) => {
  const obj = { x: 0 }
  const t = $.Tween(obj, { duration: 10, step: rec("step") }, "x", 10, "linear")
  t.run(0.5)
  return obj.x
})
C("Tween: element style", ({ $, el }) => {
  const e = el("<div style='left:10px;opacity:1'></div>")
  const t = $.Tween(e, { duration: 100 }, "left", 110, "linear")
  t.run(0.5)
  const t2 = $.Tween(e, { duration: 100 }, "opacity", 0, "linear")
  t2.run(0.25)
  return [e.style.left, e.style.opacity, t.start, t2.start, t.cur(), t2.cur()]
})
C("Tween: propHooks", ({ $, el, push }) => {
  const ph = $.Tween.propHooks
  const keys = Object.keys(ph).sort()
  const same = ph.scrollTop === ph.scrollLeft
  const e = el("<div></div>")
  e.scrollTop = 7
  const t = $.Tween(e, {}, "scrollTop", 20, "linear")
  const start = t.start
  ph.myprop = { get: (tw) => { push("get", tw.prop); return 3 }, set: (tw) => push("set", tw.now) }
  const obj = { myprop: 100 }
  const t2 = $.Tween(obj, { duration: 10 }, "myprop", 13, "linear")
  t2.run(0.5)
  ph.onlyget = { get: () => 11 }
  const obj2 = { onlyget: 1 }
  const t3 = $.Tween(obj2, {}, "onlyget", 21, "linear")
  t3.run(0.5)
  ph.onlyset = { set: (tw) => push("onlyset", tw.now) }
  const obj3 = { onlyset: 4 }
  const t4 = $.Tween(obj3, {}, "onlyset", 8, "linear")
  t4.run(0.5)
  delete ph.myprop; delete ph.onlyget; delete ph.onlyset
  return [keys, same, typeof ph._default.get, typeof ph._default.set, start, t2.start, obj.myprop, t3.start, obj2.onlyget, t4.start, obj3.onlyset]
})
C("Tween: _default get variants", ({ $, el }) => {
  const g = $.Tween.propHooks._default.get
  const e = el("<div style='width:auto;height:15px;margin-left:3px'></div>")
  return [
    g({ elem: { a: 1 }, prop: "a" }), g({ elem: { a: null }, prop: "a" }), g({ elem: e, prop: "height" }),
    g({ elem: e, prop: "width" }), g({ elem: e, prop: "marginLeft" }), g({ elem: e, prop: "scrollTop" }),
    g({ elem: e, prop: "unknownProp" }), g({ elem: e, prop: "zIndex" }),
  ]
})
C("Tween: _default set variants", ({ $, el, push }) => {
  const s = $.Tween.propHooks._default.set
  const e = el("<div></div>")
  s({ elem: e, prop: "height", now: 12, unit: "px" })
  s({ elem: e, prop: "opacity", now: 0.5, unit: "" })
  s({ elem: e, prop: "foo", now: 3, unit: "px" })
  const obj = {}
  s({ elem: obj, prop: "x", now: 4, unit: "px" })
  $.fx.step.custom = function (tw) { push("step hook", this === $.fx.step, tw.now) }
  s({ elem: obj, prop: "custom", now: 9, unit: "px" })
  delete $.fx.step.custom
  return [e.style.height, e.style.opacity, e.foo, obj.x, obj.custom]
})
C("Tween: scroll set", ({ $, el }) => {
  const s = $.Tween.propHooks.scrollTop.set
  const e = el("<div></div>")
  s({ elem: e, prop: "scrollTop", now: 5 })
  const detached = document.createElement("div")
  s({ elem: detached, prop: "scrollTop", now: 5 })
  const obj = {}
  s({ elem: obj, prop: "x", now: 5 })
  return [e.scrollTop, detached.scrollTop, obj.x]
})
C("Tween: prototype and fx", ({ $ }) => {
  const p = $.Tween.prototype
  return [Object.keys(p), p.init === $.fx, p.init.prototype === p, p.constructor === $.Tween, typeof p.cur, typeof p.run,
    Object.keys($.fx.step), typeof $.fx, $.fx.interval, $.fx.speeds, typeof $.fx.tick, typeof $.fx.timer, typeof $.fx.start, typeof $.fx.stop]
})
C("Tween: new fx init", ({ $ }) => {
  const obj = { y: 2 }
  const t = new $.fx(obj, { duration: 1 }, "y", 4, "linear")
  const t2 = new $.fx(obj, { duration: 1 }, "y", 4, "linear", "em")
  return [t instanceof $.Tween, t.start, t.unit, t2.unit, t.end]
})
C("Tween: init with call on object", ({ $ }) => {
  const target = {}
  const r = $.Tween.prototype.init.call(target, { z: 1 }, {}, "z", 3)
  return [r, target.start, target.unit, target.easing]
})
C("Tween: run returns this, cur override", ({ $ }) => {
  const obj = { x: 0 }
  const t = $.Tween(obj, { duration: 10 }, "x", 10, "linear")
  return [t.run(0.5) === t, t.cur(), t.now]
})
C("easing", ({ $ }) => {
  return [Object.keys($.easing), $.easing.linear(0.3), $.easing.swing(0), $.easing.swing(0.5), $.easing.swing(1), $.easing.swing(0.25), $.easing._default]
})
C("Tween: custom easing", ({ $, push }) => {
  $.easing.myEase = function (p, ms, a, b, d) { push("ease", p, ms, a, b, d); return p * p }
  const obj = { x: 0 }
  const t = $.Tween(obj, { duration: 200 }, "x", 10, "myEase")
  t.run(0.5)
  delete $.easing.myEase
  return [t.pos, t.now]
})

// ===== animate / Animation
C("animate: plain object", ({ $, advance, push }) => {
  const obj = { x: 0, y: 100 }
  const $o = $(obj)
  const r = $o.animate({ x: 100, y: 0 }, { duration: 100, easing: "linear", step(now, fx) { push("step", fx.prop, now) }, complete() { push("complete", this === obj) } })
  const r1 = r === $o
  advance(16)
  const a = [obj.x, obj.y]
  advance(48)
  const b = [obj.x, obj.y]
  advance(100)
  return [r1, a, b, obj.x, obj.y, $.timers.length]
})
C("animate: element css px", ({ $, el, advance, push }) => {
  const e = el("<div style='left:0px;top:10px;opacity:1'></div>")
  $(e).animate({ left: 100, top: "+=20", opacity: 0.5 }, 160, "linear", function () { push("done", e.style.cssText) })
  const r = []
  for (let i = 0; i < 6; i++) { advance(32); r.push(e.style.cssText) }
  return r
})
C("animate: relative and units", ({ $, el, advance }) => {
  const e = el("<div style='left:10px;width:5em;margin-top:2px'></div>")
  $(e).animate({ left: "-=5", width: "10em", marginTop: "+=3px", paddingLeft: "4px" }, 100, "linear")
  advance(50)
  const mid = e.style.cssText
  advance(100)
  return [mid, e.style.cssText]
})
C("animate: callbacks order", ({ $, el, advance, push }) => {
  const e = el("<div style='left:0px'></div>")
  $(e).animate({ left: 50 }, {
    duration: 48, easing: "linear",
    start(anim) { push("start", this === e, typeof anim.stop, anim.elem === e) },
    progress(anim, p, rem) { push("progress", p, rem) },
    step(now, tw) { push("step", now, tw.prop, tw.unit) },
    complete() { push("complete") },
    done(anim, jumped) { push("done", jumped) },
    fail() { push("fail") },
    always(anim, jumped) { push("always", jumped) },
  })
  advance(100)
  return e.style.left
})
C("animate: queue chaining", ({ $, el, advance, push }) => {
  const e = el("<div style='left:0px;top:0px'></div>")
  $(e).animate({ left: 10 }, 32, "linear", () => push("a", e.style.left))
    .animate({ top: 10 }, 32, "linear", () => push("b", e.style.top))
    .queue(function (n) { push("q"); n() })
  const q = $(e).queue().length
  advance(200)
  return [q, $(e).queue().length]
})
C("animate: queue false runs in parallel", ({ $, el, advance, push }) => {
  const e = el("<div style='left:0px;top:0px'></div>")
  $(e).animate({ left: 10 }, { duration: 64, queue: false, easing: "linear" })
  $(e).animate({ top: 10 }, { duration: 32, queue: false, easing: "linear", complete() { push("top done", e.style.left) } })
  advance(40)
  const mid = e.style.cssText
  advance(100)
  return [mid, e.style.cssText, $(e).queue().length]
})
C("animate: custom queue", ({ $, el, advance, push }) => {
  const e = el("<div style='left:0px'></div>")
  $(e).animate({ left: 10 }, { duration: 32, queue: "foo", easing: "linear", complete() { push("done") } })
  advance(50)
  const before = e.style.left
  $(e).dequeue("foo")
  advance(100)
  return [before, e.style.left]
})
C("animate: empty props", ({ $, el, advance, push }) => {
  const e = el("<div></div>")
  const r = $(e).animate({}, 100, () => push("complete empty"))
  push("sync")
  advance(10)
  return r.length
})
C("animate: fx.off", ({ $, el, advance, push }) => {
  const e = el("<div style='left:0px'></div>")
  $.fx.off = true
  $(e).animate({ left: 100 }, 1000, () => push("done", e.style.left))
  push("sync", e.style.left)
  advance(20)
  $.fx.off = false
  return e.style.left
})
C("animate: duration 0", ({ $, el, advance, push }) => {
  const e = el("<div style='left:0px'></div>")
  $(e).animate({ left: 100 }, 0, () => push("done", e.style.left))
  push("sync", e.style.left)
  advance(20)
  return e.style.left
})
C("animate: specialEasing and array values", ({ $, advance, push }) => {
  const obj = { a: 0, b: 0, c: 0 }
  $.easing.twice = (p) => Math.min(1, p * 2)
  $(obj).animate({ a: 100, b: [100, "twice"], c: 100 }, { duration: 100, specialEasing: { c: "twice" }, easing: "linear" })
  advance(32)
  const mid = [obj.a, obj.b, obj.c]
  advance(100)
  delete $.easing.twice
  return [mid, obj]
})
C("animate: camelCase and expand hooks", ({ $, el, advance }) => {
  const e = el("<div style='margin:0px;padding:0px'></div>")
  $(e).animate({ "margin-left": 10, padding: 8 }, 32, "linear")
  advance(100)
  return e.style.cssText
})
C("animate: toggle/show/hide values", ({ $, el, advance, push }) => {
  const e = el("<div style='height:20px;width:30px;opacity:1'></div>")
  $(e).animate({ height: "hide", opacity: "hide" }, 32, "linear", () => push("hidden", e.style.cssText))
  advance(100)
  $(e).animate({ height: "show", opacity: "show" }, 32, "linear", () => push("shown", e.style.cssText))
  advance(16)
  push("mid", e.style.cssText)
  advance(100)
  $(e).animate({ height: "toggle" }, 32, "linear", () => push("toggled", e.style.cssText))
  advance(100)
  $(e).animate({ height: "toggle" }, 32, "linear", () => push("toggled2", e.style.cssText))
  advance(100)
  return [e.style.cssText, $(e).is(":hidden")]
})
C("animate: show already visible / hide already hidden", ({ $, el, advance, push }) => {
  const e = el("<div style='height:20px'></div>")
  const h = el("<div style='height:20px;display:none'></div>")
  $(e).animate({ height: "show" }, 32, () => push("show visible done", e.style.cssText))
  $(h).animate({ height: "hide" }, 32, () => push("hide hidden done", h.style.cssText))
  advance(100)
  return [e.style.cssText, h.style.cssText]
})
C("animate: inline element width", ({ $, el, advance, push }) => {
  const e = el("<span style='width:10px'></span>")
  $(e).animate({ width: 40 }, 32, "linear", () => push("done", e.style.cssText))
  push("start", e.style.cssText)
  advance(100)
  return e.style.cssText
})
C("animate: overflow restore", ({ $, el, advance, push }) => {
  const e = el("<div style='height:10px;overflow:auto;overflow-x:scroll'></div>")
  $(e).animate({ height: 40 }, 32, "linear")
  push("during", e.style.overflow, e.style.overflowX, e.style.overflowY)
  advance(100)
  return [e.style.overflow, e.style.overflowX, e.style.overflowY]
})
C("animate: returns and Animation object", ({ $, advance, push }) => {
  const obj = { x: 0 }
  let anim
  $(obj).animate({ x: 10 }, { duration: 32, start(a) { anim = a } })
  const keys = Object.keys(anim).sort()
  const shape = [anim.elem === obj, anim.props, anim.originalProperties, anim.opts.duration, anim.opts.easing, anim.opts.queue, anim.startTime, anim.duration, anim.tweens.length, typeof anim.createTween, typeof anim.stop, typeof anim.promise, typeof anim.done, anim.state()]
  advance(100)
  return [keys, shape, anim.state()]
})
C("Animation: direct call", ({ $, advance, push }) => {
  const obj = { x: 0, y: 5 }
  const props = { x: 10, y: [15, "linear"] }
  const opts = { duration: 32, easing: "linear" }
  const anim = $.Animation(obj, props, opts)
  anim.progress((a, p, r) => push("progress", p, r)).done(() => push("done"))
  const d = [anim.props, props, anim.opts === opts, anim.originalOptions === opts, anim.originalProperties === props, $.timers.length]
  advance(100)
  return [d, obj, $.timers.length, anim.state()]
})
C("Animation: stop(false) and stop(true)", ({ $, advance, push }) => {
  const o1 = { x: 0 }, o2 = { x: 0 }
  const a1 = $.Animation(o1, { x: 100 }, { duration: 100, easing: "linear" })
  const a2 = $.Animation(o2, { x: 100 }, { duration: 100, easing: "linear" })
  a1.fail((a, j) => push("a1 fail", j)).done(() => push("a1 done"))
  a2.done((a, j) => push("a2 done", j)).progress((a, p, r) => push("a2 progress", p, r))
  advance(48)
  const r1 = a1.stop()
  const r2 = a2.stop(true)
  const again = a2.stop(true)
  advance(100)
  return [r1 === a1, r2 === a2, again === a2, o1.x, o2.x, a1.state(), a2.state(), $.timers.length]
})
C("Animation: zero tweens notifies", ({ $, advance, push }) => {
  const anim = $.Animation({}, {}, { duration: 50 })
  anim.progress((a, p, r) => push("progress", p, r)).done(() => push("done"))
  advance(20)
  return anim.state()
})
C("Animation: prefilter and tweener", ({ $, advance, push }) => {
  const pf = function (elem, props, opts) { push("prefilter", this.elem === elem, Object.keys(props), opts.duration) }
  $.Animation.prefilter(pf)
  $.Animation.prefilter(function () { push("prepended") }, true)
  const tw = function (prop, value) { push("tweener", prop, value); return this.createTween(prop, value) }
  $.Animation.tweener("x", tw)
  $.Animation.tweener("y z", tw)
  $.Animation.tweener(function (prop, value) { push("star tweener", prop, value) })
  const obj = { x: 0, y: 0, q: 0 }
  $(obj).animate({ x: 10, y: 20, q: 5 }, 32)
  advance(100)
  const pre = $.Animation.prefilters
  const tws = $.Animation.tweeners
  const res = [pre.length, pre[0] !== pf, pre[pre.length - 1] === pf, Object.keys(tws).sort(), tws["*"].length, tws.x.length, tws.z.length, obj]
  pre.splice(pre.indexOf(pf), 1)
  pre.shift()
  delete tws.x; delete tws.y; delete tws.z
  tws["*"].shift()
  return res
})
C("Animation: tweener odd props", ({ $ }) => {
  const tws = $.Animation.tweeners
  const before = Object.keys(tws).sort()
  $.Animation.tweener("  a   b ", () => {})
  $.Animation.tweener("", () => {})
  const after = Object.keys(tws).sort()
  const r = [before, after, tws.a.length, tws.b.length]
  delete tws.a; delete tws.b
  return [r, $.Animation.tweener("q", () => {}), $.Animation.prefilter(() => {}, false), typeof $.Animation.tweeners, typeof $.Animation.prefilters]
})
C("Animation: prefilter returning result with stop", ({ $, el, advance, push }) => {
  const e = el("<div></div>")
  const custom = { stop() { push("custom stop", this === custom) } }
  const pf = function () { return custom }
  $.Animation.prefilters.unshift(pf)
  const r = $.Animation(e, { left: 10 }, $.speed(100))
  $.Animation.prefilters.shift()
  const h = $._queueHooks(e, "fx")
  const hasStop = typeof h.stop
  if (h.stop) h.stop()
  const r2 = (() => {
    $.Animation.prefilters.unshift(pf)
    const res = $.Animation(e, { left: 10 }, { duration: 10, queue: false })
    $.Animation.prefilters.shift()
    return res === custom
  })()
  return [r === custom, hasStop, r2, Object.keys($._data(e)).sort()]
})
C("Animation: tweener returning custom tween", ({ $, advance, push }) => {
  const fake = { run(p) { push("fake run", p); return this } }
  $.Animation.tweener("zz", function () { return fake })
  const obj = { zz: 0 }
  const anim = $.Animation(obj, { zz: 5 }, { duration: 32 })
  advance(100)
  delete $.Animation.tweeners.zz
  return anim.tweens.length
})
C("Animation: createTween on anim", ({ $, advance, push }) => {
  const obj = { a: 0, b: 0 }
  const anim = $.Animation(obj, { a: 10 }, { duration: 32, easing: "linear", specialEasing: { b: "swing" } })
  const t = anim.createTween("b", 20)
  const t2 = anim.createTween("c", 20)
  advance(100)
  return [anim.tweens.length, t.easing, t2.easing, t.start, obj.b]
})
C("Animation: timer shape", ({ $ }) => {
  const obj = { x: 0 }
  const anim = $.Animation(obj, { x: 1 }, { duration: 100, queue: "fx" })
  const t = $.timers[0]
  const r = [typeof t, t.elem === obj, t.anim === anim, t.queue, Object.keys(t).sort()]
  anim.stop()
  return r
})
C("Tween.run override is used by animation", ({ $, advance, push }) => {
  const orig = $.Tween.prototype.run
  $.Tween.prototype.run = function (p) { push("patched run", p); return orig.call(this, p) }
  const obj = { x: 0 }
  $(obj).animate({ x: 10 }, 32, "linear")
  advance(100)
  $.Tween.prototype.run = orig
  return obj.x
})
C("jQuery.Tween override is used by createTween", ({ $, advance, push }) => {
  const orig = $.Tween
  $.Tween = function (...a) { push("patched Tween", a.length, a[2], a[3], a[4]); return orig(...a) }
  const obj = { x: 0 }
  $(obj).animate({ x: 10 }, 32, "linear")
  advance(100)
  $.Tween = orig
  return obj.x
})
C("animate: stop mid-flight", ({ $, el, advance, push }) => {
  const e = el("<div style='left:0px;top:0px'></div>")
  $(e).animate({ left: 100 }, 100, "linear", () => push("left done"))
    .animate({ top: 100 }, 100, "linear", () => push("top done"))
  advance(48)
  const r = $(e).stop()
  const mid = e.style.cssText
  advance(48)
  const mid2 = e.style.cssText
  advance(200)
  return [r.length, mid, mid2, e.style.cssText]
})
C("animate: stop(true) clears queue", ({ $, el, advance, push }) => {
  const e = el("<div style='left:0px;top:0px'></div>")
  $(e).animate({ left: 100 }, 100, "linear", () => push("left done"))
    .animate({ top: 100 }, 100, "linear", () => push("top done"))
  advance(48)
  $(e).stop(true)
  advance(300)
  return [e.style.cssText, $(e).queue().length, $.timers.length]
})
C("animate: stop(true, true) jumps to end", ({ $, el, advance, push }) => {
  const e = el("<div style='left:0px;top:0px'></div>")
  $(e).animate({ left: 100 }, 100, "linear", () => push("left done", e.style.left))
    .animate({ top: 100 }, 100, "linear", () => push("top done"))
  advance(48)
  $(e).stop(true, true)
  const mid = e.style.cssText
  advance(300)
  return [mid, e.style.cssText, $(e).queue().length]
})
C("animate: stop(false, true)", ({ $, el, advance, push }) => {
  const e = el("<div style='left:0px;top:0px'></div>")
  $(e).animate({ left: 100 }, 100, "linear", () => push("left done", e.style.left))
    .animate({ top: 100 }, 100, "linear", () => push("top done"))
  advance(48)
  $(e).stop(false, true)
  const mid = e.style.cssText
  advance(300)
  return [mid, e.style.cssText]
})
C("stop: typed queues", ({ $, el, advance, push }) => {
  const e = el("<div style='left:0px;top:0px'></div>")
  $(e).animate({ left: 100 }, { duration: 100, queue: "foo", easing: "linear", complete() { push("foo done") } })
  $(e).dequeue("foo")
  $(e).animate({ top: 100 }, { duration: 100, easing: "linear", complete() { push("fx done") } })
  advance(48)
  $(e).stop("foo", true, true)
  const mid = e.style.cssText
  advance(48)
  const mid2 = e.style.cssText
  $(e).stop("fx")
  advance(300)
  return [mid, mid2, e.style.cssText, $(e).queue("foo").length]
})
C("stop: argument shapes", ({ $, el, advance, push }) => {
  const r = []
  for (const args of [[], [true], [false], [true, true], ["fx"], ["fx", true], ["fx", true, true], [undefined, true, true], [null, true], ["foo", false, true], [1, true]]) {
    const e = el("<div style='left:0px'></div>")
    $(e).animate({ left: 100 }, 100, "linear", () => push("done", JSON.stringify(args)))
      .delay(50).queue(function (n) { push("queued", JSON.stringify(args)); n() })
    advance(32)
    const ret = $(e).stop(...args)
    advance(500)
    r.push([ret.length, e.style.left, $(e).queue().length])
  }
  return r
})
C("stop: delay hooks stop", ({ $, el, advance, push }) => {
  const e = el("<div></div>")
  $(e).delay(100, "foo").queue("foo", function (n) { push("foo ran"); n() })
  $(e).dequeue("foo")
  advance(10)
  $(e).stop("foo")
  advance(200)
  const a = $(e).queue("foo").length
  $(e).dequeue("foo")
  return [a, $(e).queue("foo").length]
})
C("stop: user hooks stop via queue fn", ({ $, el, advance, push }) => {
  const e = el("<div></div>")
  $(e).queue(function (next, hooks) { hooks.stop = function (gotoEnd) { push("user stop", gotoEnd, this === hooks) } })
  $(e).stop(false, true)
  $(e).queue(function (next, hooks) { hooks.stop = function (g) { push("user stop2", g) } })
  $(e).stop(true)
  return $(e).queue().length
})
C("finish: jumps all", ({ $, el, advance, push }) => {
  const e = el("<div style='left:0px;top:0px'></div>")
  $(e).animate({ left: 100 }, 100, "linear", () => push("left done", e.style.cssText))
    .animate({ top: 100 }, 100, "linear", () => push("top done", e.style.cssText))
    .queue(function (n) { push("plain queued"); n() })
  advance(32)
  const r = $(e).finish()
  const after = e.style.cssText
  advance(300)
  return [r.length, after, e.style.cssText, $(e).queue().length, $.timers.length]
})
C("finish: types", ({ $, el, advance, push }) => {
  const e = el("<div style='left:0px;top:0px'></div>")
  $(e).animate({ left: 100 }, { duration: 100, queue: "foo", complete() { push("foo done") } })
  $(e).dequeue("foo")
  $(e).animate({ top: 100 }, { duration: 100, complete() { push("fx done") } })
  advance(32)
  $(e).finish("foo")
  const a = e.style.cssText
  $(e).finish(false)
  const b = e.style.cssText
  $(e).finish("")
  const c = e.style.cssText
  advance(300)
  return [a, b, c]
})
C("finish: queue false animation", ({ $, el, advance, push }) => {
  const e = el("<div style='left:0px'></div>")
  $(e).animate({ left: 100 }, { duration: 100, queue: false, complete() { push("done") } })
  advance(32)
  $(e).finish()
  const a = e.style.left
  $(e).finish(false)
  advance(300)
  return [a, e.style.left]
})
C("finish: empty and no animations", ({ $, el }) => {
  const e = el("<div></div>")
  return [$().finish().length, $(e).finish().length, $(e).finish("foo").length, Object.keys($._data(e)).sort()]
})

// ===== show/hide/toggle with speed, slide*, fade*
C("show/hide/toggle: no speed delegates to css", ({ $, el }) => {
  const e = el("<div style='display:none'></div>")
  const r = []
  $(e).show(); r.push(e.style.display)
  $(e).hide(); r.push(e.style.display)
  $(e).toggle(); r.push(e.style.display)
  $(e).toggle(false); r.push(e.style.display)
  $(e).toggle(true); r.push(e.style.display)
  $(e).show(null); r.push(e.style.display)
  $(e).hide(undefined); r.push(e.style.display)
  $(e).show(false); r.push(e.style.display)
  return [r, $.timers.length]
})
C("show/hide: with speed", ({ $, el, advance, push }) => {
  const e = el("<div style='width:20px;height:10px'></div>")
  $(e).hide(64, "linear", () => push("hidden", e.style.cssText))
  advance(32)
  push("mid", e.style.cssText)
  advance(100)
  $(e).show(64, () => push("shown", e.style.cssText))
  advance(32)
  push("mid2", e.style.cssText)
  advance(100)
  $(e).toggle("fast", () => push("toggled", e.style.cssText))
  advance(300)
  $(e).toggle(0, () => push("toggled0", e.style.cssText))
  advance(20)
  return e.style.cssText
})
C("show: with options object", ({ $, el, advance, push }) => {
  const e = el("<div style='display:none;height:10px'></div>")
  $(e).show({ duration: 32, complete() { push("done", e.style.cssText) } })
  advance(100)
  $(e).hide("slow")
  advance(1000)
  return e.style.cssText
})
C("show/hide: function speed", ({ $, el, advance, push }) => {
  const e = el("<div style='height:10px'></div>")
  $(e).hide(function () { push("cb", e.style.cssText) })
  advance(1000)
  return e.style.cssText
})
C("slideUp/Down/Toggle", ({ $, el, advance, push }) => {
  const e = el("<div style='height:30px;padding-top:4px;margin-bottom:2px'></div>")
  $(e).slideUp(64, () => push("up", e.style.cssText))
  advance(32)
  push("mid", e.style.cssText)
  advance(100)
  $(e).slideDown(64, "linear", () => push("down", e.style.cssText))
  advance(32)
  push("mid2", e.style.cssText)
  advance(100)
  $(e).slideToggle(() => push("toggle", e.style.cssText))
  advance(1000)
  $(e).slideToggle(10)
  advance(100)
  return [e.style.cssText, $.fn.slideUp.length, $.fn.slideDown.length]
})
C("fadeIn/Out/Toggle/To", ({ $, el, advance, push }) => {
  const e = el("<div style='opacity:1'></div>")
  $(e).fadeOut(64, "linear", () => push("out", e.style.cssText))
  advance(32)
  push("mid", e.style.cssText)
  advance(100)
  $(e).fadeIn(64, () => push("in", e.style.cssText))
  advance(100)
  $(e).fadeTo(64, 0.3, "linear", () => push("to", e.style.cssText))
  advance(32)
  push("mid2", e.style.cssText)
  advance(100)
  $(e).fadeToggle("fast", () => push("toggle", e.style.cssText))
  advance(300)
  $(e).fadeToggle({ duration: 32, complete() { push("toggle2", e.style.cssText) } })
  advance(100)
  return [e.style.cssText, $.fn.fadeTo.length, $.fn.fadeIn.length, $.fn.animate.length, $.fn.stop.length, $.fn.finish.length, $.fn.delay.length, $.fn.queue.length, $.fn.promise.length, $.fn.show.length]
})
C("fadeTo: hidden element", ({ $, el, advance, push }) => {
  const e = el("<div style='display:none;opacity:1'></div>")
  const r = $(e).fadeTo(32, 0.5, () => push("done", e.style.cssText))
  push("sync", e.style.cssText, r.length)
  advance(100)
  return e.style.cssText
})
C("fadeTo: returns end set", ({ $, el }) => {
  const a = el("<div id='a'></div>"), b = el("<div id='b' style='display:none'></div>")
  const $s = $([a, b])
  const r = $s.fadeTo(0, 0.5)
  return [r === $s, r.length, a.style.opacity, b.style.opacity]
})
C("fx: speeds defaults and tick/timer", ({ $, push, advance }) => {
  const calls = []
  let n = 0
  const timer = () => { calls.push(++n); return n < 3 }
  $.fx.timer(timer)
  const l1 = $.timers.length
  advance(100)
  return [l1, calls, $.timers.length, $.fx.speeds, $.fx.interval]
})
C("fx: tick removes finished", ({ $ }) => {
  const r = []
  const t1 = () => false
  const t2 = () => true
  $.timers.push(t1, t2)
  $.fx.tick()
  r.push($.timers.length, $.timers[0] === t2)
  $.timers.length = 0
  $.fx.tick()
  return r
})
C("fx: tick with self-removing timer", ({ $ }) => {
  const order = []
  const a = () => { order.push("a"); $.timers.splice(0, 1); return false }
  const b = () => { order.push("b"); return false }
  $.timers.push(a, b)
  $.fx.tick()
  return [order, $.timers.length]
})
C("fx: interval and start/stop without raf", ({ $, win, advance, push }) => {
  const raf = win.requestAnimationFrame
  win.requestAnimationFrame = undefined
  const obj = { x: 0 }
  $.fx.interval = 50
  $(obj).animate({ x: 10 }, 100, "linear", () => push("done", Date.now()))
  advance(60)
  const mid = obj.x
  advance(200)
  win.requestAnimationFrame = raf
  $.fx.interval = 13
  return mid
})
C("fx: hidden document uses timeout", ({ $, win, doc, advance, push }) => {
  Object.defineProperty(doc, "hidden", { value: true, configurable: true })
  const obj = { x: 0 }
  $(obj).animate({ x: 10 }, 100, "linear", () => push("done"))
  advance(26)
  const mid = obj.x
  advance(200)
  return mid
})
C(":animated selector", ({ $, el, advance }) => {
  const a = el("<div id='a' style='left:0px'></div>"), b = el("<div id='b'></div>")
  $(a).animate({ left: 10 }, 100)
  const r = [$("div:animated").length, $(a).is(":animated"), $(b).is(":animated"), $("div").filter(":animated")[0] === a, $("div").not(":animated").length]
  $(a).animate({ left: 20 }, { duration: 100, queue: false })
  r.push($("div:animated").length, $.expr.pseudos.animated(a), $.expr.pseudos.animated(b))
  advance(300)
  r.push($("div:animated").length)
  return r
})
C("animate: on detached and multiple elems", ({ $, doc, advance, push }) => {
  const a = doc.createElement("div"); a.style.left = "0px"
  const b = doc.createElement("div"); b.style.left = "10px"
  $([a, b]).animate({ left: 50 }, 32, "linear", function () { push("done", this === a ? "a" : "b", this.style.left) })
  advance(100)
  return [a.style.left, b.style.left]
})
C("animate: finish flag and data finish", ({ $, el, advance, push }) => {
  const e = el("<div style='left:0px'></div>")
  $(e).animate({ left: 50 }, 100, () => push("first"))
  $(e).animate({ left: 70 }, 100, () => push("second"))
  $(e).queue(function (n) { push("plain", $._data(e, "finish")); n() })
  $(e).finish()
  return [e.style.left, $._data(e, "finish")]
})
C("animate: hidden element show with display restore", ({ $, el, advance, push }) => {
  const e = el("<span style='display:none;width:10px'></span>")
  $(e).animate({ width: "show" }, 32, () => push("shown", e.style.cssText))
  push("start", e.style.cssText)
  advance(100)
  $(e).animate({ width: "hide" }, 32, () => push("hidden", e.style.cssText))
  advance(100)
  return [e.style.cssText, $._data(e)]
})
C("animate: fxshow data reuse (toggle twice quickly)", ({ $, el, advance, push }) => {
  const e = el("<div style='height:20px'></div>")
  $(e).slideToggle(64)
  advance(32)
  $(e).stop().slideToggle(64, () => push("back", e.style.cssText))
  advance(200)
  return [e.style.cssText, Object.keys($._data(e)).sort()]
})
C("animate: unqueued hooks bookkeeping", ({ $, el, advance, push }) => {
  const e = el("<div style='left:0px;top:0px'></div>")
  $(e).animate({ left: 10 }, { duration: 32, queue: false })
  const h = $._queueHooks(e, "fx")
  const u1 = h.unqueued
  $(e).animate({ top: 10 }, { duration: 64, queue: false })
  const u2 = h.unqueued
  const p = $(e).promise()
  p.done(() => push("promise done", Date.now()))
  advance(40)
  const u3 = h.unqueued
  advance(100)
  return [u1, u2, u3, h.unqueued, p.state(), Object.keys($._data(e)).sort()]
})
C("animate: string numbers and bad values", ({ $, advance }) => {
  const obj = { a: "5", b: 0, c: "abc" }
  $(obj).animate({ a: "15", b: "x", c: 10, d: 4 }, 32, "linear")
  advance(100)
  return obj
})
C("animate: args shapes", ({ $, advance, push }) => {
  const r = []
  for (const args of [[], [100], [100, "linear"], ["fast", () => push("cb fast")], [{ duration: 20 }], [null, null, () => push("cb3")], [undefined, () => push("cb2")]]) {
    const obj = { x: 0 }
    const ret = $(obj).animate({ x: 10 }, ...args)
    advance(16)
    const mid = obj.x
    advance(1000)
    r.push([ret.length, mid, obj.x])
  }
  return r
})
C("animate: prop object not mutated", ({ $, el, advance }) => {
  const e = el("<div style='height:10px'></div>")
  const props = { height: "toggle", "margin-left": 5, opacity: [0.5, "linear"] }
  $(e).animate(props, 32)
  advance(100)
  return props
})
C("animate: step can stop", ({ $, advance, push }) => {
  const obj = { x: 0 }
  $(obj).animate({ x: 100 }, { duration: 100, easing: "linear", step(now) { push("step", now); if (now > 30) $(obj).stop() } })
  advance(200)
  return obj.x
})
C("animate: complete this per element and opts reuse", ({ $, doc, advance, push }) => {
  const opts = { duration: 32, complete() { push("complete", this.id) } }
  const a = doc.createElement("div"); a.id = "a"
  const b = doc.createElement("div"); b.id = "b"
  $([a, b]).animate({ opacity: 0.5 }, opts)
  advance(100)
  return Object.keys(opts).sort()
})
C("animate: toggle hidden reading dataShow", ({ $, el, advance, push }) => {
  const e = el("<div style='height:20px;opacity:1'></div>")
  $(e).fadeToggle(64)
  advance(16)
  $(e).fadeToggle(64)
  advance(16)
  $(e).fadeToggle(64)
  advance(500)
  return [e.style.cssText, $(e).css("display")]
})
C("animate: show on hidden with dataShow hidden", ({ $, el, advance, push }) => {
  const e = el("<div style='height:20px'></div>")
  $(e).hide(32)
  advance(8)
  $(e).stop(true).show(32, () => push("shown", e.style.cssText))
  advance(200)
  return e.style.cssText
})
C("fx.timer and fx.start return", ({ $ }) => {
  const r = [$.fx.timer(() => false), $.fx.start(), $.fx.stop(), $.fx.tick()]
  return r
})


// ===== additional edge cases
C("Animation: no duration option", ({ $, advance, push }) => {
  const obj = { x: 0 }
  const anim = $.Animation(obj, { x: 10 }, {})
  anim.progress((a, p, r) => push("progress", p, r)).done(() => push("done"))
  const d = anim.duration
  advance(20)
  return [d, obj.x, anim.state()]
})
C("Animation: string duration", ({ $, advance, push }) => {
  const obj = { x: 0 }
  const anim = $.Animation(obj, { x: 10 }, { duration: "100" })
  anim.progress((a, p, r) => push("progress", p, r))
  advance(20)
  const mid = obj.x
  advance(200)
  return [anim.duration, mid, obj.x, anim.state()]
})
C("Animation: negative duration", ({ $, advance, push }) => {
  const obj = { x: 0 }
  const anim = $.Animation(obj, { x: 10 }, { duration: -100, easing: "linear" })
  anim.progress((a, p, r) => push("progress", p, r))
  advance(20)
  return [obj.x, anim.state()]
})
C("Tween: easing returning string", ({ $ }) => {
  $.easing.strEase = () => "0.5"
  const obj = { x: 0 }
  const t = $.Tween(obj, { duration: 10 }, "x", 10, "strEase")
  t.run(0.3)
  delete $.easing.strEase
  return [t.pos, typeof t.pos, t.now, obj.x]
})
C("fx.step hook through animate", ({ $, advance, push }) => {
  $.fx.step.zz = function (tw) { push("zz step", tw.now, tw.prop); tw.elem.zz = tw.now * 2 }
  const obj = { zz: 0 }
  $(obj).animate({ zz: 10 }, 32, "linear")
  advance(100)
  delete $.fx.step.zz
  return obj.zz
})
C("animate: step modifies tween end", ({ $, advance }) => {
  const obj = { x: 0 }
  $(obj).animate({ x: 100 }, { duration: 64, easing: "linear", step(now, tw) { tw.end = 50 } })
  advance(200)
  return obj.x
})
C("toggle with boolean and extra args", ({ $, el, advance }) => {
  const e = el("<div style='height:10px'></div>")
  const r = []
  $(e).toggle(false, 100); r.push(e.style.display)
  $(e).toggle(true, 100); r.push(e.style.display)
  $(e).hide(0); r.push(e.style.display)
  advance(50); r.push(e.style.display)
  $(e).show(null, "linear"); r.push(e.style.display)
  return [r, $.timers.length]
})
C("fadeIn default duration", ({ $, el, advance, push }) => {
  const e = el("<div style='display:none;opacity:1'></div>")
  $(e).fadeIn(() => push("done", Date.now()))
  advance(200)
  push("at200", e.style.opacity)
  advance(400)
  return e.style.cssText
})
C("slideDown with options", ({ $, el, advance, push }) => {
  const e = el("<div style='display:none;height:40px'></div>")
  $(e).slideDown({ duration: 64, easing: "linear", step(now, tw) { if (tw.prop === "height") push("h", now) }, complete() { push("c", e.style.cssText) } })
  advance(200)
  return e.style.cssText
})
C("queue: async next and hooks reuse", ({ $, el, advance, push }) => {
  const e = el("<div></div>")
  $(e).queue(function (next, hooks) { push("a", typeof hooks.empty); setTimeout(next, 30) })
    .queue(function (next, hooks) { push("b", hooks === $._queueHooks(e, "fx")); next() })
  const p = $(e).promise().done(() => push("promise"))
  advance(10)
  push("t10", $(e).queue().length)
  advance(50)
  return [$(e).queue().length, p.state(), $.hasData(e)]
})
C("stop: string type clearQueue", ({ $, el, advance, push }) => {
  const e = el("<div style='left:0px'></div>")
  $(e).animate({ left: 50 }, { duration: 100, queue: "foo" }).animate({ left: 80 }, { duration: 100, queue: "foo" })
  $(e).dequeue("foo")
  advance(32)
  $(e).stop("foo", true)
  advance(300)
  return [e.style.left, $(e).queue("foo").length]
})
C("finish: queued function with finish", ({ $, el, push }) => {
  const e = el("<div></div>")
  const f = function (next) { push("f run"); next() }
  f.finish = function () { push("f.finish", this === e, arguments.length) }
  $(e).queue(function (next) { push("blocker") })
  $(e).queue(f)
  $(e).finish()
  return $(e).queue().length
})
C("speed: fx.off with object", ({ $ }) => {
  $.fx.off = true
  const o = $.speed({ duration: 500, queue: false, easing: "swing" })
  $.fx.off = false
  return [o.duration, o.queue, o.easing]
})
C("animate: multiple elements promise", ({ $, doc, advance, push }) => {
  const a = doc.createElement("div"), b = doc.createElement("div")
  a.style.left = "0px"; b.style.left = "0px"
  doc.body.append(a, b)
  const $s = $([a, b])
  $s.animate({ left: 10 }, 32).animate({ left: 20 }, 64)
  $s.promise().done(function () { push("all done", Date.now(), this.length) })
  advance(500)
  return [a.style.left, b.style.left]
})
C("dequeue: no data at all", ({ $, el, push }) => {
  const e = el("<div></div>")
  $.dequeue(e)
  return [$.hasData(e), Object.keys($._data(e))]
})
C("delay then animate then stop(true,true)", ({ $, el, advance, push }) => {
  const e = el("<div style='left:0px'></div>")
  $(e).delay(50).animate({ left: 100 }, 100, () => push("anim done"))
  advance(20)
  $(e).stop(true, true)
  advance(300)
  return [e.style.left, $(e).queue().length]
})
C("propHooks custom get returning string", ({ $, advance }) => {
  $.Tween.propHooks.weird = { get: () => "7px" }
  const obj = { weird: 0 }
  $(obj).animate({ weird: 17 }, 32, "linear")
  advance(100)
  delete $.Tween.propHooks.weird
  return obj.weird
})
C("Animation.tweener with array props", ({ $ }) => {
  let r
  try { $.Animation.tweener(["a", "b"], () => {}); r = "ok" } catch (e) { r = e.constructor.name }
  const keys = Object.keys($.Animation.tweeners).sort()
  for (const k of keys) if (k !== "*") delete $.Animation.tweeners[k]
  return [r, keys]
})
C("fx.tick override is used by scheduler", ({ $, advance, push }) => {
  const orig = $.fx.tick
  $.fx.tick = function () { push("tick"); return orig.apply(this, arguments) }
  const obj = { x: 0 }
  $(obj).animate({ x: 1 }, 32)
  advance(100)
  $.fx.tick = orig
  return obj.x
})
C("fx.timer override is used by Animation", ({ $, advance, push }) => {
  const orig = $.fx.timer
  $.fx.timer = function (t) { push("timer", typeof t, typeof t.elem, t.queue); return orig.apply(this, arguments) }
  const obj = { x: 0 }
  $(obj).animate({ x: 1 }, 32)
  advance(100)
  $.fx.timer = orig
  return obj.x
})


C("public function arities", ({ $ }) => {
  const f = {
    queue: $.queue, dequeue: $.dequeue, _queueHooks: $._queueHooks, speed: $.speed, Animation: $.Animation,
    tweener: $.Animation.tweener, prefilter: $.Animation.prefilter, Tween: $.Tween, fx: $.fx,
    tick: $.fx.tick, timer: $.fx.timer, start: $.fx.start, stop: $.fx.stop,
    cur: $.Tween.prototype.cur, run: $.Tween.prototype.run, init: $.Tween.prototype.init,
    linear: $.easing.linear, swing: $.easing.swing,
    dget: $.Tween.propHooks._default.get, dset: $.Tween.propHooks._default.set, sset: $.Tween.propHooks.scrollTop.set,
  }
  for (const n of ["queue", "dequeue", "clearQueue", "promise", "delay", "fadeTo", "animate", "stop", "finish", "toggle", "show", "hide", "slideDown", "slideUp", "slideToggle", "fadeIn", "fadeOut", "fadeToggle"]) f["fn." + n] = $.fn[n]
  const out = {}
  for (const k of Object.keys(f)) out[k] = [typeof f[k], f[k] && f[k].length]
  return [out, Object.keys($.Tween.propHooks), Object.keys($.fx.speeds), Object.keys($.Animation.tweeners), $.Animation.prefilters.length]
})

// ---------------------------------------------------------------- run
const results = []
let mismatches = []
for (const [name, fn] of cases) {
  if (only && !name.includes(only)) continue
  const u = await runOne(loadOfficial, fn)
  const l = await runOne(loadOurs, fn)
  const ok = u === l
  results.push({ name, ok, u, l })
  if (!ok) mismatches.push(name)
  if (verbose || (!ok && process.argv.includes("--show"))) {
    console.log((ok ? "ok   " : "DIFF ") + name)
    if (!ok) {
      console.log("  official: " + u.slice(0, 3000))
      console.log("  ours    : " + l.slice(0, 3000))
    }
  }
}
Date.now = realDateNow
console.log(`cases ${results.length} match ${results.length - mismatches.length} mismatch ${mismatches.length}`)
for (const m of mismatches) console.log("  MISMATCH " + m)
process.exit(0)
