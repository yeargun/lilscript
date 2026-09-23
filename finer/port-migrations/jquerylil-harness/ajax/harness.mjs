// Differential harness for the ajax group (ajax.lil, ajax/*, serialize.lil).
// usage: node harness.mjs <dist/jquery.esm.js> [--json out.json] [--only name] [--verbose]
// Each scenario runs twice, in two fresh identical jsdom windows: once with official jquery@3.7.1,
// once with the LilScript build. Logs are normalized and compared entry by entry.
import { createRequire } from "node:module"
import { pathToFileURL } from "node:url"
import { writeFileSync } from "node:fs"

const req = createRequire("/home/azureuser/jquerylil/package.json")
const { JSDOM, VirtualConsole } = req("jsdom")
const jqPath = req.resolve("jquery")

const args = process.argv.slice(2)
const distPath = args[0]
const jsonOut = args.includes("--json") ? args[args.indexOf("--json") + 1] : null
const only = args.includes("--only") ? args[args.indexOf("--only") + 1] : null
const verbose = args.includes("--verbose")
const dump = args.includes("--dump")
let importCounter = 0

const PAGE = `<!doctype html><html><head><title>t</title></head><body><div id="root"></div></body></html>`

function installMockXHR(win, env) {
  const opts = env.xhrOptions || {}
  class MockXHR {
    constructor() {
      this.readyState = 0
      this.status = 0
      this.statusText = ""
      this.responseText = ""
      this.response = ""
      this.responseType = ""
      if (!opts.noCors) this.withCredentials = false
      this.onload = null
      this.onerror = null
      if (!opts.noOnabort) this.onabort = null
      this.ontimeout = null
      this.onreadystatechange = null
      this._headers = []
      env.xhrs.push(this)
      this._id = env.xhrs.length
    }
    open(method, url, async, user, pass) {
      this._req = { method, url, async, user, pass }
      this.readyState = 1
    }
    setRequestHeader(k, v) { this._headers.push([k, String(v)]) }
    overrideMimeType(m) { this._mime = m }
    getAllResponseHeaders() { return this._respHeaders ?? "" }
    abort() { env.log.push(["xhr.abort", this._id]); this._aborted = true }
    send(body) {
      const extra = {}
      for (const k of Object.keys(this)) {
        if (k[0] === "_" || ["readyState", "status", "statusText", "responseText", "response", "responseType",
          "onload", "onerror", "onabort", "ontimeout", "onreadystatechange"].includes(k)) continue
        extra[k] = this[k]
      }
      const handlers = ["onload", "onerror", "onabort", "ontimeout", "onreadystatechange"].filter((k) => typeof this[k] === "function")
      const reqInfo = { ...this._req, headers: this._headers, body: body === undefined ? "<undef>" : body, mime: this._mime ?? null, extra, handlers }
      env.log.push(["xhr.send", this._id, reqInfo])
      const resp = env.server ? env.server(reqInfo, this) : { status: 200, body: "ok" }
      if (resp.throwOnSend) throw new Error(resp.throwOnSend)
      const finish = () => {
        if (this._aborted) return
        this.readyState = 4
        try {
          if (resp.kind === "error") {
            this.status = resp.status ?? 0
            if (resp.statusNotNumber) this.status = undefined
            this.onreadystatechange && this.onreadystatechange()
            this.onerror && this.onerror()
            return
          }
          if (resp.kind === "timeout") { this.ontimeout && this.ontimeout(); return }
          if (resp.kind === "abortevent") { this.onabort && this.onabort(); return }
          if (resp.kind === "rsconly") { this.status = resp.status ?? 0; this.onreadystatechange && this.onreadystatechange(); return }
          this.status = resp.status ?? 200
          this.statusText = resp.statusText ?? "OK"
          this._respHeaders = resp.headers ?? ""
          this.responseType = resp.responseType ?? ""
          if ("binary" in resp) { this.response = resp.binary; this.responseText = resp.responseText }
          else { this.responseText = resp.body ?? ""; this.response = this.responseText }
          this.onreadystatechange && this.onreadystatechange()
          this.onload && this.onload()
        } catch (e) {
          env.log.push(["xhr.handler-threw", ser(e, env)])
        }
      }
      if (resp.delay === "never") return
      if (resp.delay === "sync") finish()
      else win.setTimeout(finish, resp.delay ?? 1)
    }
  }
  win.XMLHttpRequest = MockXHR
}

async function makeEnv(which, envOpts) {
  const vc = new VirtualConsole()
  const pending = []
  vc.on("jsdomError", (e) => pending.push(["jsdomError", String(e && e.message).split("\n")[0].replace(/at .*/, "")]))
  const dom = new JSDOM(PAGE, { url: "http://example.com/dir/page.html", runScripts: "dangerously", pretendToBeVisual: true, virtualConsole: vc })
  const win = dom.window
  const env = { log: pending, xhrs: [], server: null, win, doc: win.document, dom, xhrOptions: envOpts.xhr || {} }
  installMockXHR(win, env)
  globalThis.window = win
  globalThis.document = win.document
  globalThis.DOMParser = win.DOMParser // a browser has it as a global; the port's host helper reads the global
  let $
  if (which === "official") {
    delete req.cache[jqPath]
    const f = req("jquery")
    $ = f.fn?.jquery ? f : f(win)
  } else {
    $ = (await import(pathToFileURL(distPath).href + "?n=" + importCounter++)).jQuery
  }
  env.$ = $
  env.wait = (ms = 20) => new Promise((r) => setTimeout(r, ms))
  env.L = (...entry) => env.log.push(entry.map((x) => ser(x, env)))
  return env
}

function isJqXHR(v) {
  return v && typeof v === "object" && typeof v.setRequestHeader === "function" && typeof v.getAllResponseHeaders === "function"
}

function ser(v, env, depth = 0, seen = new Set()) {
  if (v === undefined) return "<undef>"
  if (v === null) return null
  const t = typeof v
  if (t === "number") return Number.isNaN(v) ? "<NaN>" : Object.is(v, -0) ? "<-0>" : v
  if (t === "string" || t === "boolean") return v
  if (t === "bigint") return "<big>"
  if (t === "symbol") return "<sym>"
  if (t === "function") {
    if (v === String) return "<String>"
    if (v === JSON.parse) return "<JSON.parse>"
    if (env && env.$ && v === env.$.parseXML) return "<parseXML>"
    return "<fn/" + v.length + ">"
  }
  if (env && v === env.win) return "<window>"
  if (env && v === env.$) return "<jQuery>"
  const tag = Object.prototype.toString.call(v)
  if (tag === "[object RegExp]") return "<re " + v.source + "/" + v.flags + ">"
  if (tag === "[object Error]" || (v && typeof v.name === "string" && typeof v.message === "string" && "stack" in v)) {
    return { error: v.name, message: v.name === "TypeError" || v.name === "ReferenceError" ? "" : v.message }
  }
  if (typeof v.nodeType === "number" && typeof v.nodeName === "string") {
    if (v.nodeType === 9) return "<#document " + (v.documentElement ? v.documentElement.nodeName : "") + (v === env?.doc ? " (page)" : "") + ">"
    let s = "<" + v.nodeName + (v.id ? "#" + v.id : "") + (v.className && typeof v.className === "string" ? "." + v.className : "") + ">"
    return s
  }
  if (isJqXHR(v)) {
    return "<jqXHR rs=" + v.readyState + " st=" + v.status + " stx=" + v.statusText + " rt=" + JSON.stringify(v.responseText) +
      ("responseJSON" in v ? " json=" + JSON.stringify(ser(v.responseJSON, env, 3)) : "") +
      ("responseXML" in v ? " xml=" + JSON.stringify(ser(v.responseXML, env, 3)) : "") + ">"
  }
  if (typeof v.jquery === "string" && typeof v.length === "number") {
    return { $: Array.from({ length: v.length }, (_, i) => ser(v[i], env, depth + 1, seen)) }
  }
  if (v && typeof v === "object" && typeof v.type === "string" && typeof v.isDefaultPrevented === "function") {
    return { event: v.type, target: ser(v.target, env, 3), cur: ser(v.currentTarget, env, 3) }
  }
  if (Array.isArray(v) && Array.isArray(v.dataTypes) === false && depth > 5) return "<deep>"
  if (seen.has(v)) return "<cycle>"
  if (depth > 5) return "<deep>"
  seen.add(v)
  let out
  if (Array.isArray(v)) out = v.map((x) => ser(x, env, depth + 1, seen))
  else if (tag === "[object Arguments]") out = { args: Array.from(v).map((x) => ser(x, env, depth + 1, seen)) }
  else {
    out = {}
    for (const k of Object.keys(v)) out[k] = ser(v[k], env, depth + 1, seen)
  }
  seen.delete(v)
  return out
}

// settings object snapshot: only request-shaping keys, in their own-key order
const SETTINGS_KEYS = new Set(["url", "type", "method", "dataType", "dataTypes", "data", "crossDomain", "hasContent", "contentType",
  "async", "global", "cache", "processData", "isLocal", "jsonp", "jsonpCallback", "mimeType", "headers", "timeout", "traditional",
  "scriptCharset", "scriptAttrs", "ifModified", "username", "password", "throws", "context", "custom"])
function snapS(s, env) {
  const o = {}
  for (const k of Object.keys(s)) if (SETTINGS_KEYS.has(k)) o[k] = ser(s[k], env, 2)
  o.__keys = Object.keys(s).join(",")
  return o
}

function hooks($, env, extra = {}) {
  // full callback set on a request
  return {
    beforeSend(xhr, s) { env.L("beforeSend", this === s ? "this=s" : this, xhr, snapS(s, env)); if (extra.beforeSend) return extra.beforeSend.call(this, xhr, s) },
    success(data, st, xhr) { env.L("success", this === extra.ctx ? "this=ctx" : "this?", data, st, xhr) },
    error(xhr, st, err) { env.L("error", this === extra.ctx ? "this=ctx" : "this?", xhr, st, err) },
    complete(xhr, st) { env.L("complete", this === extra.ctx ? "this=ctx" : "this?", xhr, st) },
  }
}
function track(env, jq, label = "p") {
  if (!isJqXHR(jq)) { env.L(label + ".ret", jq); return jq }
  jq.done(function (...a) { env.L(label + ".done", a) })
  jq.fail(function (...a) { env.L(label + ".fail", a) })
  jq.always(function (...a) { env.L(label + ".always", a.length) })
  env.L(label + ".rs", jq.readyState, typeof jq.then, typeof jq.promise, jq.state())
  return jq
}
function globals($, env, target) {
  $(target || env.doc).on("ajaxStart ajaxStop ajaxSend ajaxSuccess ajaxError ajaxComplete", function (e, xhr, s, extra) {
    env.L("G:" + e.type, this, e.target, xhr, s ? s.url : "<nos>", extra, $.active)
  })
}
const ok = (body = "ok", more = {}) => () => ({ status: 200, statusText: "OK", body, headers: "Content-Type: text/plain\r\n", ...more })

const scenarios = []
const S = (name, fn, envOpts = {}) => scenarios.push({ name, fn, envOpts })

// ---------------------------------------------------------------- surface
S("surface", async ($, env) => {
  const names = ["ajax", "ajaxSetup", "ajaxPrefilter", "ajaxTransport", "get", "post", "getJSON", "getScript", "param"]
  for (const n of names) env.L(n, typeof $[n], $[n].length)
  env.L("fn", typeof $.fn.load, $.fn.load.length, typeof $.fn.serialize, $.fn.serialize.length, typeof $.fn.serializeArray, $.fn.serializeArray.length)
  env.L("active", $.active, $.lastModified, $.etag)
  env.L("support", $.support.ajax, $.support.cors)
})
S("ajaxSettings-shape", async ($, env) => {
  const s = $.ajaxSettings
  env.L("keys", Object.keys(s))
  for (const k of ["accepts", "contents", "responseFields", "converters", "flatOptions"]) env.L(k, Object.keys(s[k]), s[k])
  env.L("vals", s.url, s.type, s.isLocal, s.global, s.processData, s.async, s.contentType, typeof s.xhr, s.jsonp, typeof s.jsonpCallback)
  env.L("conv", s.converters["* text"](12), s.converters["text json"]("[1,2]"), s.converters["* text"]({ toString() { return "T" }, valueOf() { return "V" } }))
  let e1; try { s.converters["text json"]("{bad") } catch (e) { e1 = e } env.L("conv-err", e1)
  env.L("xhr", Object.prototype.toString.call(s.xhr()))
})
S("ajaxSetup-forms", async ($, env) => {
  const r1 = $.ajaxSetup({ custom: { a: 1, deep: { x: 1 } }, url: "/x", context: { c: 1 }, undef: undefined, headers: { A: "1" } })
  env.L("r1===settings", r1 === $.ajaxSettings, $.ajaxSettings.custom, $.ajaxSettings.url, $.ajaxSettings.context, "undef" in $.ajaxSettings)
  const r2 = $.ajaxSetup({ custom: { b: 2, deep: { y: 2 } }, headers: { B: "2" } })
  env.L("r2", $.ajaxSettings.custom, $.ajaxSettings.headers)
  const target = { t: 1 }
  const r3 = $.ajaxSetup(target, { url: "/y", z: { q: [1, 2] }, context: target })
  env.L("r3", r3 === target, Object.keys(target), target.url, target.z, target.context === target, target.type, target.accepts === $.ajaxSettings.accepts)
  env.L("r3-deepcopy", target.custom !== $.ajaxSettings.custom, target.converters !== $.ajaxSettings.converters)
  const r4 = $.ajaxSetup({}, null)
  env.L("r4", r4 === $.ajaxSettings)
  const r5 = $.ajaxSetup()
  env.L("r5", r5 === $.ajaxSettings)
  $.ajaxSettings.flatOptions.custom = true
  const t6 = {}
  $.ajaxSetup(t6, { custom: { k: 1 } })
  env.L("flat", t6.custom === $.ajaxSettings.custom, t6.custom)
  $.ajaxSettings.flatOptions = null
  const t7 = $.ajaxSetup({}, { url: "/z", deepx: { a: 1 } })
  env.L("flat-null", t7.url, t7.deepx)
})
S("prefilter-transport-registration", async ($, env) => {
  const seen = []
  env.L("ret", $.ajaxPrefilter("Foo  BAR +baz", () => { seen.push("f1") }), $.ajaxPrefilter(123), $.ajaxPrefilter("x", "notfn"), $.ajaxTransport("  "))
  $.ajaxPrefilter("foo", function (s) { seen.push("foo-a:" + s.dataTypes.join("|")) })
  $.ajaxPrefilter("+foo", function (s) { seen.push("foo-pre") })
  $.ajaxPrefilter("+", function (s) { seen.push("star-pre") })
  $.ajaxPrefilter(function (s, o, x) { seen.push("star:" + typeof o + ":" + isJqXHR(x)) })
  $.ajaxTransport("foo", function (s, o, x) { seen.push("t-foo"); return null })
  $.ajaxTransport("+foo", function (s) { seen.push("t-foo-pre"); return undefined })
  $.ajaxTransport("foo", function (s) {
    seen.push("t-foo-real")
    return { send(h, done) { seen.push("send:" + JSON.stringify(h)); done(200, "fine", { text: "hello" }, "X-A: 1\r\n") }, abort() { seen.push("abort") } }
  })
  const p = track(env, $.ajax({ url: "/q", dataType: "foo" }))
  await env.wait()
  env.L("seen", seen)
})

// ---------------------------------------------------------------- basic requests
S("get-basic", async ($, env) => {
  globals($, env)
  env.server = ok("hello", { headers: "Content-Type: text/plain\r\nX-Multi: a\r\nx-multi: b\r\nX-Empty:\r\n" })
  const jq = track(env, $.ajax("data.txt", hooks($, env)))
  env.L("before", jq.getResponseHeader("X-Multi"), jq.getAllResponseHeaders(), jq.readyState, $.active)
  await env.wait()
  env.L("after", jq.getResponseHeader("x-MULTI"), jq.getResponseHeader("missing"), jq.getResponseHeader("x-empty"), jq.getAllResponseHeaders(), jq.readyState, jq.status, jq.statusText, $.active)
  env.L("chain", jq.setRequestHeader("A", "b") === jq, jq.overrideMimeType("x") === jq, jq.statusCode() === jq, jq.abort() === jq)
  env.L("keys", Object.keys(jq))
})
S("ajax-arg-forms", async ($, env) => {
  env.server = (r) => ({ status: 200, body: r.url })
  track(env, $.ajax(), "noargs")
  track(env, $.ajax({ url: "a1" }), "opts")
  track(env, $.ajax("a2", { url: "ignored", type: "post" }), "urlopts")
  track(env, $.ajax("a3", null), "urlnull")
  track(env, $.ajax(null), "null")
  track(env, $.ajax(undefined, { url: "a4" }), "undefurl")
  track(env, $.ajax("", { url: "a5" }), "emptyurl")
  track(env, $.ajax({ url: "//other.org/p" }), "protorel")
  track(env, $.ajax({ url: "http://example.com:80/same" }), "sameport")
  track(env, $.ajax({ url: "https://example.com/sec" }), "https")
  track(env, $.ajax({ url: "http://[bad" }), "badurl")
  track(env, $.ajax({ url: "x", crossDomain: false }), "cdfalse")
  track(env, $.ajax({ url: "x", crossDomain: true }), "cdtrue")
  await env.wait(30)
})
S("types-methods", async ($, env) => {
  env.server = (r) => ({ status: 200, body: r.method + " " + r.url + " " + r.body })
  const o = hooks($, env)
  track(env, $.ajax({ url: "t1", type: "post", data: { a: 1 }, ...o }), "post")
  track(env, $.ajax({ url: "t2", method: "put", type: "get", data: "x=1" }), "put")
  track(env, $.ajax({ url: "t3", type: "head" }), "head")
  track(env, $.ajax({ url: "t4", type: "delete", data: { b: [1, 2] } }), "del")
  $.ajaxSetup({ method: "PATCH" })
  track(env, $.ajax({ url: "t5" }), "setupmethod")
  track(env, $.ajax({ url: "t6", type: "GET" }), "opttype")
  await env.wait(30)
})
S("data-processing", async ($, env) => {
  env.server = (r) => ({ status: 200, body: JSON.stringify([r.url, r.body, r.headers]) })
  track(env, $.ajax({ url: "d1?x=1#hash", data: { a: "b c", d: [1, 2] } }), "get-obj")
  track(env, $.ajax({ url: "d2#h", data: "raw=1 2" }), "get-str")
  track(env, $.ajax({ url: "d3", data: { a: [1, 2] }, traditional: true }), "trad")
  track(env, $.ajax({ url: "d4", data: { a: 1 }, processData: false }), "noprocess-obj")
  track(env, $.ajax({ url: "d5", data: "q=1", processData: false }), "noprocess-str")
  track(env, $.ajax({ url: "d6", type: "POST", data: { a: "b c", e: "%20" } }), "post-obj")
  track(env, $.ajax({ url: "d7", type: "POST", data: "a=b c%20d" }), "post-str")
  track(env, $.ajax({ url: "d8", type: "POST", data: "a=b%20c", contentType: "text/plain" }), "post-ct")
  track(env, $.ajax({ url: "d9", type: "POST", data: "a=b%20c", contentType: false }), "post-ctfalse")
  track(env, $.ajax({ url: "d10", type: "POST", data: { a: 1 }, processData: false, contentType: "application/json" }), "post-nop")
  track(env, $.ajax({ url: "d11", contentType: "text/x" }), "get-ct")
  track(env, $.ajax({ url: "d12", type: "POST" }), "post-nodata")
  track(env, $.ajax({ url: "d13", data: "", type: "POST" }), "post-empty")
  track(env, $.ajax({ url: "d14", data: 0 }), "get-zero")
  track(env, $.ajax({ url: "d15", type: "POST", data: 5 }), "post-num")
  track(env, $.ajax({ url: "d16", type: "POST", headers: { "content-TYPE": "app/x" }, data: "a" }), "hdr-ct")
  track(env, $.ajax({ url: "d17", type: "POST", headers: { "Content-Type": "" }, data: "a" }), "hdr-ct-empty")
  await env.wait(30)
})
S("cache-anticache", async ($, env) => {
  env.server = (r) => ({ status: 200, body: JSON.stringify(r.url) })
  track(env, $.ajax({ url: "c1", cache: false }), "c1")
  track(env, $.ajax({ url: "c2?_=123&b=1", cache: false }), "c2")
  track(env, $.ajax({ url: "c3?a=1#frag", cache: false, data: { z: 1 } }), "c3")
  track(env, $.ajax({ url: "c4", cache: false, type: "POST" }), "c4")
  track(env, $.ajax({ url: "c5", cache: true }), "c5")
  track(env, $.ajax({ url: "c6", dataType: "script" }), "c6script")
  track(env, $.ajax({ url: "c7", dataType: "script", cache: true }), "c7script")
  await env.wait(30)
  env.L("urls", env.log.filter((e) => e[0] === "xhr.send").map((e) => e[2].url))
})
S("headers-accept", async ($, env) => {
  env.server = (r) => ({ status: 200, body: "{}", headers: "Content-Type: application/json" })
  for (const dt of [undefined, "json", "xml", "html", "text", "script", "*", "unknown", "text json", " JSON  ", "jsonp"]) {
    track(env, $.ajax({ url: "h?" + dt, dataType: dt, accepts: { unknown: "x/u" } }), "dt:" + dt)
  }
  track(env, $.ajax({ url: "h2", headers: { "X-One": "1", accept: "mine", "x-one": "2" } }), "hdrs")
  track(env, $.ajax({ url: "h3", beforeSend(x) { x.setRequestHeader("x-requested-with", "Me"); x.setRequestHeader("X-Two", 2); x.setRequestHeader("x-two", 3) } }), "bs")
  track(env, $.ajax({ url: "http://other.org/h4" }), "cross")
  track(env, $.ajax({ url: "h5", xhrFields: { withCredentials: true, custom: 5 }, username: "u", password: "p", async: false }), "fields")
  track(env, $.ajax({ url: "h6", mimeType: "text/x-mine" }), "mime")
  await env.wait(30)
})
S("beforeSend-abort", async ($, env) => {
  globals($, env)
  env.server = ok()
  const ctx = { me: 1 }
  track(env, $.ajax({ url: "b1", ...hooks($, env, { ctx, beforeSend: () => false }), context: ctx }), "false")
  track(env, $.ajax({ url: "b2", ...hooks($, env, { beforeSend: (x) => { x.abort("mine") } }) }), "abortinside")
  track(env, $.ajax({ url: "b3", ...hooks($, env, { beforeSend: () => 0 }) }), "zero")
  track(env, $.ajax({ url: "b4", global: false, ...hooks($, env, { beforeSend: () => false }) }), "noglobal")
  await env.wait(30)
})
S("status-handling", async ($, env) => {
  globals($, env)
  const codes = { s200: [200, "OK"], s201: [201, "Created"], s204: [204, "No Content"], s304: [304, "Not Modified"], s404: [404, "Not Found"],
    s500: [500, ""], s0: [0, ""], s1223: [1223, "x"], s299: [299, "edge"], s300: [300, "mc"], s199: [199, "info"] }
  env.server = (r) => { const [st, tx] = codes[r.url.split("/").pop()]; return { status: st, statusText: tx, body: "b-" + st } }
  for (const k of Object.keys(codes)) track(env, $.ajax({ url: k, ...hooks($, env) }), k)
  track(env, $.ajax({ url: "s200", type: "HEAD" }), "head200")
  await env.wait(40)
})
S("xhr-errors", async ($, env) => {
  env.server = (r) => {
    if (r.url.endsWith("e1")) return { kind: "error", status: 0 }
    if (r.url.endsWith("e2")) return { kind: "error", status: 503 }
    if (r.url.endsWith("e3")) return { kind: "timeout" }
    if (r.url.endsWith("e4")) return { kind: "abortevent" }
    if (r.url.endsWith("e5")) return { throwOnSend: "sendfail" }
    if (r.url.endsWith("e6")) return { kind: "error", statusNotNumber: true }
    return { status: 200 }
  }
  for (const k of ["e1", "e2", "e3", "e4", "e5", "e6"]) track(env, $.ajax({ url: k, ...hooks($, env) }), k)
  const saved = $.ajaxSettings.xhr
  $.ajaxSettings.xhr = () => null
  track(env, $.ajax({ url: "e7", ...hooks($, env) }), "nullxhr")
  $.ajaxSettings.xhr = saved
  await env.wait(40)
})
S("xhr-no-onabort", async ($, env) => {
  env.server = (r) => {
    if (r.url.endsWith("r1")) return { status: 200, body: "fine" }
    if (r.url.endsWith("r2")) return { kind: "rsconly", status: 0 }
    if (r.url.endsWith("r3")) return { kind: "error", status: 404 }
    return { delay: "never" }
  }
  for (const k of ["r1", "r2", "r3"]) track(env, $.ajax({ url: k, ...hooks($, env) }), k)
  const p = track(env, $.ajax({ url: "r4", ...hooks($, env) }), "r4")
  await env.wait(10)
  p.abort()
  await env.wait(40)
}, { xhr: { noOnabort: true } })
S("no-transport", async ($, env) => {
  env.server = ok()
  track(env, $.ajax({ url: "http://other.org/x", ...hooks($, env) }), "cross-nocors")
  track(env, $.ajax({ url: "local", ...hooks($, env) }), "local")
  env.L("support", $.support.cors, $.support.ajax)
  await env.wait(30)
}, { xhr: { noCors: true } })
S("abort-and-timeout", async ($, env) => {
  globals($, env)
  env.server = (r) => ({ delay: r.url.endsWith("slow") ? 60 : "never", status: 200, body: "late" })
  const a = track(env, $.ajax({ url: "a1", ...hooks($, env) }), "a1")
  const b = track(env, $.ajax({ url: "a2", ...hooks($, env) }), "a2")
  const c = track(env, $.ajax({ url: "a3", ...hooks($, env) }), "a3")
  const d = track(env, $.ajax({ url: "a4", timeout: 15, ...hooks($, env) }), "a4-timeout")
  const e = track(env, $.ajax({ url: "slow", timeout: 5, async: true }), "a5-timeout-slow")
  const f = track(env, $.ajax({ url: "a6", timeout: -1 }), "a6")
  a.abort()
  b.abort("custom")
  c.abort(123)
  await env.wait(100)
  a.abort("again")
  env.L("after", a.status, a.statusText, b.statusText, c.statusText, d.statusText, e.statusText, f.state())
  f.abort("")
  await env.wait(5)
  env.L("f", f.status, f.statusText)
})
S("statusCode-map", async ($, env) => {
  env.server = (r) => ({ status: +r.url.split("/").pop(), body: "x" })
  const mk = (tag) => ({ 200() { env.L(tag + ":200", arguments.length, this === window) }, 404() { env.L(tag + ":404", arguments.length) } })
  const p1 = $.ajax({ url: "200", statusCode: mk("opt") })
  p1.statusCode(mk("pre"))
  p1.statusCode(null)
  const p2 = $.ajax({ url: "404", statusCode: mk("opt") })
  await env.wait(30)
  p1.statusCode(mk("post"))
  p2.statusCode({ 404: [() => env.L("arr404a"), () => env.L("arr404b")] })
  p2.statusCode({ 500: () => env.L("never") })
  env.L("ret", p1.statusCode({}) === p1)
})
S("global-events", async ($, env) => {
  globals($, env)
  const el = env.doc.getElementById("root")
  $(el).on("ajaxSend ajaxComplete ajaxSuccess ajaxError", function (e) { env.L("EL:" + e.type, this, e.target) })
  env.server = (r) => ({ status: r.url.endsWith("bad") ? 500 : 200, body: "g", delay: r.url.endsWith("b") ? 40 : 1 })
  track(env, $.ajax({ url: "g-a" }), "ga")
  track(env, $.ajax({ url: "g-b", context: el }), "gb-el")
  track(env, $.ajax({ url: "g-bad", context: $(el) }), "gbad-jq")
  track(env, $.ajax({ url: "g-c", context: { plain: 1 } }), "gc-plain")
  track(env, $.ajax({ url: "g-d", global: false }), "gd-noglobal")
  env.L("active", $.active)
  await env.wait(80)
  env.L("active-after", $.active)
})
S("ajaxSend-abort", async ($, env) => {
  env.server = ok()
  $(env.doc).on("ajaxSend", (e, x) => { env.L("send"); x.abort("fromsend") })
  $(env.doc).on("ajaxStop", () => env.L("stop", $.active))
  track(env, $.ajax({ url: "x", ...hooks($, env) }), "p")
  await env.wait(20)
})
S("prefilter-abort-redirect", async ($, env) => {
  env.server = (r) => ({ status: 200, body: '{"a":1}', headers: "Content-Type: application/json" })
  $.ajaxPrefilter("abortme", (s, o, x) => { x.abort("pre") })
  $.ajaxPrefilter("redir", (s) => "json")
  $.ajaxPrefilter("json", (s) => { env.L("json-prefilter", s.dataTypes) })
  $.ajaxPrefilter("loop1", () => "loop2")
  $.ajaxPrefilter("loop2", () => "loop1")
  track(env, $.ajax({ url: "p1", dataType: "abortme", ...hooks($, env) }), "abort")
  track(env, $.ajax({ url: "p2", dataType: "redir", ...hooks($, env) }), "redir")
  track(env, $.ajax({ url: "p3", dataType: "loop1" }), "loop")
  await env.wait(30)
})
S("custom-transports", async ($, env) => {
  let sendCount = 0
  $.ajaxTransport("custom", function (s, orig, jq) {
    return {
      send(headers, complete) {
        env.L("custom-send", headers, s.url, orig === undefined ? "no-orig" : ser(orig, env, 3))
        sendCount++
        if (s.url.endsWith("throw")) throw new Error("boom")
        if (s.url.endsWith("after")) { complete(200, "OK", { text: "t" }); throw new Error("after") }
        if (s.url.endsWith("multi")) { complete(200, "OK", { text: "1", json: { j: 1 } }, "A: 1"); complete(500, "again") }
        if (s.url.endsWith("neg")) complete(-5, "neg")
        if (s.url.endsWith("none")) complete(200, "", undefined, undefined)
        if (s.url.endsWith("later")) setTimeout(() => complete(200, "ok", { text: "later" }), 5)
      },
      abort(t) { env.L("custom-abort", t) },
    }
  })
  for (const u of ["throw", "multi", "neg", "none", "later"]) track(env, $.ajax({ url: "c/" + u, dataType: "custom", ...hooks($, env) }), u)
  try { track(env, $.ajax({ url: "c/after", dataType: "custom" }), "after") } catch (e) { env.L("after-threw", e) }
  const l = track(env, $.ajax({ url: "c/later", dataType: "custom" }), "later-abort")
  l.abort()
  await env.wait(30)
})

// ---------------------------------------------------------------- conversions
S("convert-json-xml-html", async ($, env) => {
  const bodies = {
    j1: ['{"a":[1,2]}', "application/json"], j2: ["{bad", "application/json"], j3: ['"str"', "text/plain"],
    x1: ["<r><i>1</i></r>", "text/xml"], x2: ["<r><i>", "application/xml"], h1: ["<b>hi</b>", "text/html"],
    t1: ["plain", "text/plain"], n1: ["null", "application/json"], e1: ["", "application/json"],
  }
  env.server = (r) => { const [b, ct] = bodies[r.url.split("/").pop().split("?")[0]]; return { status: 200, body: b, headers: "Content-Type: " + ct + "\r\n" } }
  const cases = [["j1", "json"], ["j2", "json"], ["j3", "json"], ["j1", undefined], ["j2", undefined], ["x1", "xml"], ["x2", "xml"],
    ["x1", undefined], ["h1", undefined], ["h1", "html"], ["t1", undefined], ["t1", "text"], ["n1", "json"], ["e1", "json"], ["e1", undefined],
    ["j1", "text json"], ["j1", "html json"], ["t1", "text foo"], ["j1", "json text"], ["x1", "text xml"]]
  for (const [u, dt] of cases) track(env, $.ajax({ url: "v/" + u, dataType: dt, ...hooks($, env) }), u + ":" + dt)
  await env.wait(40)
})
S("convert-custom", async ($, env) => {
  env.server = (r) => ({ status: 200, body: "BODY", headers: "Content-Type: text/weird\r\n" })
  $.ajaxSetup({ converters: { "text upper": (t) => t.toUpperCase() + "!", "upper lower": (t) => t.toLowerCase(), "text same": true, "same final": (t) => "final:" + t, "* star": (t) => "star:" + t, "Text Mixed": (t) => "mixed:" + t } })
  const cases = ["upper", "text upper", "lower", "text upper lower", "same", "final", "star", "mixed", "text mixed", "unknownx", "text unknownx", "* upper", "upper *", "text * upper", "binary upper"]
  for (const dt of cases) track(env, $.ajax({ url: "cv?" + dt, dataType: dt, ...hooks($, env) }), dt)
  track(env, $.ajax({ url: "cv-throws", dataType: "text json", throws: false }), "nothrow")
  track(env, $.ajax({ url: "cv-df", dataType: "text upper", dataFilter(d, t) { env.L("dataFilter", d, t, this === window); return "df:" + d } }), "dataFilter")
  track(env, $.ajax({ url: "cv-df2", dataFilter(d, t) { env.L("dataFilter2", d, t); return d + "2" }, converters: { "text html": (t) => "conv:" + t } }), "dataFilter2")
  track(env, $.ajax({ url: "cv-err", dataType: "json", dataFilter(d) { env.L("df-on-error", d); return d } }), "err")
  track(env, $.ajax({ url: "cv-bad", dataType: "text upper", converters: { "text upper": () => { throw new Error("convfail") } } }), "convthrow")
  await env.wait(40)
  let thrown = []
  env.server = (r) => ({ status: 200, body: "{bad", headers: "Content-Type: application/json", delay: "sync" })
  try { $.ajax({ url: "cv-throws2", dataType: "json", throws: true, async: false, error: () => env.L("err-cb") }) } catch (e) { thrown.push(ser(e, env)) }
  env.L("throws", thrown)
})
S("convert-binary-responses", async ($, env) => {
  env.server = (r) => {
    const k = r.url.split("/").pop()
    if (k === "ab") return { status: 200, binary: { byteLength: 3 }, responseText: undefined, responseType: "arraybuffer", headers: "Content-Type: application/octet-stream" }
    if (k === "blob") return { status: 200, binary: "BLOBDATA", responseText: 7, responseType: "", headers: "" }
    if (k === "jsonrt") return { status: 200, binary: { j: 1 }, responseText: "{\"j\":1}", responseType: "json", headers: "Content-Type: application/json" }
    return { status: 200, body: "t" }
  }
  for (const k of ["ab", "blob", "jsonrt"]) track(env, $.ajax({ url: "b/" + k, ...hooks($, env) }), k)
  track(env, $.ajax({ url: "b/ab", dataType: "binary", converters: { "binary binary": true } }), "ab-binary")
  track(env, $.ajax({ url: "b/ab", dataType: "text" }), "ab-text")
  await env.wait(30)
})
S("content-detection", async ($, env) => {
  const cts = { a: "application/json; charset=utf-8", b: "text/html", c: "application/xml", d: "text/javascript", e: "", f: "image/png", g: "application/xhtml+xml" }
  env.server = (r) => { const k = r.url.split("/").pop(); return { status: 200, body: k === "d" ? "window.__detect=(window.__detect||0)+1" : k === "a" ? '{"x":1}' : k === "c" ? "<r/>" : "<p>x</p>", headers: cts[k] ? "Content-Type: " + cts[k] : "" } }
  for (const k of Object.keys(cts)) track(env, $.ajax({ url: "cd/" + k, ...hooks($, env) }), k)
  track(env, $.ajax({ url: "cd/b", mimeType: "application/json" }), "mime-override")
  track(env, $.ajax({ url: "cd/a", beforeSend(x) { x.overrideMimeType("text/plain") } }), "override-method")
  track(env, $.ajax({ url: "cd/a", contents: { json: null, custom: /json/ }, converters: { "text custom": (t) => "custom:" + t } }), "contents-custom")
  await env.wait(40)
  env.L("detect", env.win.__detect)
})
S("ifModified", async ($, env) => {
  let n = 0
  env.server = (r) => { n++; return n === 1 ? { status: 200, body: "v1", headers: "Last-Modified: Mon\r\nEtag: E1\r\n" } : n === 2 ? { status: 304, body: "", headers: "" } : { status: 200, body: "v3", headers: "ETag: E3" } }
  track(env, $.ajax({ url: "im?x=1#f", ifModified: true, ...hooks($, env) }), "first")
  await env.wait(10)
  track(env, $.ajax({ url: "im?x=1", ifModified: true, ...hooks($, env) }), "second")
  await env.wait(10)
  track(env, $.ajax({ url: "im?x=1", ifModified: true }), "third")
  await env.wait(10)
  track(env, $.ajax({ url: "im?x=1" }), "fourth-nomod")
  await env.wait(10)
  env.L("store", $.lastModified, $.etag)
  track(env, $.ajax({ url: "im2", ifModified: true, data: { q: 1 } }), "data")
  await env.wait(10)
  env.L("store2", $.lastModified, $.etag)
})
S("sync-requests", async ($, env) => {
  env.server = (r) => ({ status: 200, body: '{"s":1}', headers: "Content-Type: application/json", delay: "sync" })
  const p = track(env, $.ajax({ url: "sync", async: false, ...hooks($, env) }), "sync")
  env.L("immediate", p.state(), p.readyState, p.responseJSON)
  const q = $.ajax({ url: "sync2", async: false, timeout: 5 })
  env.L("q", q.state())
})
S("context-this", async ($, env) => {
  env.server = ok()
  const ctx = { c: 1 }
  const p = $.ajax({ url: "ctx", context: ctx, ...hooks($, env, { ctx }) })
  p.done(function () { env.L("done-this", this === ctx) })
  p.always(function () { env.L("always-this", this === ctx) })
  const q = $.ajax({ url: "noctx" })
  q.done(function () { env.L("noctx-this-is-settings", !!(this && this.url && this.dataTypes), this && this.url) })
  await env.wait(20)
})
S("jqXHR-api-before-after", async ($, env) => {
  env.server = (r) => ({ status: 200, body: "x", headers: "A: 1\r\nB: 2\r\n", delay: 5 })
  const p = $.ajax({ url: "api", beforeSend(x, s) {
    x.setRequestHeader("K", "v1"); x.setRequestHeader("k", "v2"); x.overrideMimeType("text/foo"); env.L("mime", s.mimeType)
  } })
  env.L("pre", p.getAllResponseHeaders(), p.getResponseHeader("A"), p.readyState)
  p.setRequestHeader("Late", "no")
  await env.wait(20)
  p.setRequestHeader("After", "x")
  p.overrideMimeType("after/x")
  env.L("post", p.getAllResponseHeaders(), p.getResponseHeader("a"), p.getResponseHeader("b"), p.getResponseHeader("c"))
  const set = p.setRequestHeader
  env.L("detached", set("Q", "1") === undefined)
  const other = { o: 1 }
  env.L("otherthis", p.setRequestHeader.call(other, "Z", "1") === other, p.overrideMimeType.call(other, "t") === other, p.statusCode.call(other, null) === other)
  const gr = p.getResponseHeader
  env.L("gr-detached", gr("A"))
})

// ---------------------------------------------------------------- get/post shorthands
S("shorthands", async ($, env) => {
  env.server = (r) => ({ status: 200, body: r.url.includes("json") ? '{"k":"' + r.method + '"}' : r.method + ":" + r.url + ":" + r.body, headers: r.url.includes("json") ? "Content-Type: application/json" : "" })
  const cb = (tag) => function (d, s, x) { env.L(tag, d, s, isJqXHR(x), this && this.url) }
  track(env, $.get("s1", { a: 1 }, cb("g1"), "text"), "g1")
  track(env, $.get("s2", cb("g2")), "g2")
  track(env, $.get("s3json", cb("g3"), "json"), "g3")
  track(env, $.get("s4json", cb("g4"), undefined, "json"), "g4")
  track(env, $.get("s5", "q=1"), "g5")
  track(env, $.get({ url: "s6json", data: { b: 2 }, success: cb("g6"), dataType: "json" }), "g6")
  track(env, $.get(), "g7-noargs")
  track(env, $.post("p1", { a: [1, 2] }, cb("p1")), "p1")
  track(env, $.post("p2", cb("p2")), "p2")
  track(env, $.post({ url: "p3", type: "PUT", data: "raw" }), "p3")
  track(env, $.post("p4json", null, cb("p4"), "json"), "p4")
  track(env, $.getJSON("j1json", { x: 1 }, cb("j1")), "j1")
  track(env, $.getJSON("j2json", cb("j2")), "j2")
  track(env, $.get("s8", cb("g8"), cb("g8b")), "g8-twofn")
  track(env, $.get("s9", null, null, null), "g9-nulls")
  await env.wait(40)
})
S("getScript-xhr", async ($, env) => {
  env.server = (r) => ({ status: 200, body: "window.__gs=(window.__gs||[]);window.__gs.push('" + r.url.split("?")[0].split("/").pop() + "')", headers: "Content-Type: text/javascript" })
  track(env, $.getScript("js/one.js", function (d, s, x) { env.L("gs-cb", d, s, window.__gs && window.__gs.slice()) }), "one")
  track(env, $.ajax({ url: "js/two.js", dataType: "script", cache: true }), "two")
  track(env, $.ajax({ url: "js/three.js" }), "three-detect")
  track(env, $.ajax({ url: "js/four.js", type: "POST", dataType: "script" }), "four-post")
  await env.wait(40)
  env.L("gs", env.win.__gs)
  env.server = (r) => ({ status: 404, body: "window.__bad=1", headers: "Content-Type: text/javascript" })
  track(env, $.getScript("js/missing.js"), "missing")
  track(env, $.ajax({ url: "js/missing2.js", dataType: "script json" }), "missing-json")
  await env.wait(20)
  env.L("bad", env.win.__bad)
})
S("script-tag-transport", async ($, env) => {
  globals($, env)
  const fire = async (type) => {
    await env.wait(5)
    const scripts = Array.from(env.doc.head.querySelectorAll("script"))
    env.L("scripts", scripts.map((s) => [s.src, s.charset, s.getAttribute("data-x"), s.getAttribute("nonce"), s.type]))
    const last = scripts[scripts.length - 1]
    if (last && type) last.dispatchEvent(new env.win.Event(type))
    await env.wait(5)
    env.L("remaining", env.doc.head.querySelectorAll("script").length)
  }
  track(env, $.ajax({ url: "http://cdn.org/a.js", dataType: "script", scriptCharset: "utf-8", type: "POST", ...hooks($, env) }), "cross-load")
  await fire("load")
  track(env, $.getScript("http://cdn.org/b.js", () => env.L("b-cb")), "cross-error")
  await fire("error")
  track(env, $.ajax({ url: "local.js", dataType: "script", scriptAttrs: { "data-x": "1", nonce: "abc" } }), "attrs")
  await fire("load")
  const p = track(env, $.ajax({ url: "http://cdn.org/c.js", dataType: "script" }), "cross-abort")
  await fire(null)
  p.abort()
  await fire(null)
  track(env, $.ajax({ url: "http://cdn.org/d.js", dataType: "script", cache: true, crossDomain: true, ...hooks($, env) }), "cache-true")
  await fire("load")
})
S("script-contents-crossdomain", async ($, env) => {
  env.server = (r) => ({ status: 200, body: "window.__cross=1", headers: "Content-Type: text/javascript" })
  track(env, $.ajax({ url: "http://other.org/x", ...hooks($, env) }), "cross-nodt")
  await env.wait(20)
  env.L("cross", env.win.__cross)
})

// ---------------------------------------------------------------- jsonp
S("jsonp-basic", async ($, env) => {
  env.server = (r) => {
    const m = /[?&](\w+)=([\w$]+)/.exec(r.url + "&" + (r.body || ""))
    const cbm = /(?:callback|cb|jsonp)=([\w$]+)/.exec(r.url + " " + (typeof r.body === "string" ? r.body : ""))
    const name = cbm ? cbm[1] : "fixedName"
    return { status: 200, body: r.url.includes("nocall") ? "void 0" : name + '({"from":"' + r.url.split("?")[0] + '"})', headers: "Content-Type: text/javascript" }
  }
  track(env, $.ajax({ url: "jp1?callback=?", dataType: "json", ...hooks($, env) }), "url-q")
  await env.wait(20)
  track(env, $.ajax({ url: "jp2", dataType: "jsonp", ...hooks($, env) }), "dt-jsonp")
  await env.wait(20)
  track(env, $.ajax({ url: "jp3?x=1", dataType: "jsonp", jsonp: "cb" }), "custom-param")
  await env.wait(20)
  track(env, $.ajax({ url: "jp4", dataType: "jsonp", jsonpCallback: "fixedName" }), "fixed")
  await env.wait(20)
  track(env, $.ajax({ url: "jp5", dataType: "jsonp", jsonp: false, jsonpCallback: "fixedName" }), "jsonp-false")
  await env.wait(20)
  track(env, $.ajax({ url: "jp6", dataType: "jsonp", jsonpCallback: function () { env.L("jcb-this", !!this.dataTypes); return "fnName" } }), "fn-cb")
  await env.wait(20)
  track(env, $.ajax({ url: "jp7nocall", dataType: "jsonp", ...hooks($, env) }), "nocall")
  await env.wait(20)
  track(env, $.ajax({ url: "jp8", type: "POST", data: "callback=?&z=1", dataType: "json" }), "data-q")
  await env.wait(20)
  track(env, $.ajax({ url: "jp9??", dataType: "json" }), "doubleq")
  await env.wait(20)
  track(env, $.ajax({ url: "jp10?callback=?", dataType: "json", jsonp: false }), "jsonpfalse-url")
  await env.wait(20)
  track(env, $.ajax({ url: "jp11", type: "POST", data: "callback=?", contentType: "text/plain", dataType: "json" }), "data-wrongct")
  await env.wait(20)
  track(env, $.getJSON("jp12?callback=?", (d) => env.L("getJSON-jsonp", d)), "getJSON")
  await env.wait(20)
  env.L("globals", Object.keys(env.win).filter((k) => /^jQuery.|fixedName|fnName/.test(k)))
})
S("jsonp-overwritten-reuse", async ($, env) => {
  env.server = (r) => {
    const cbm = /callback=([\w$]+)/.exec(r.url)
    return { status: r.url.includes("fail") ? 500 : 200, body: (cbm ? cbm[1] : "preexisting") + '({"v":1})', headers: "Content-Type: text/javascript" }
  }
  env.win.preexisting = function (d) { env.L("pre-called", d, arguments.length) }
  track(env, $.ajax({ url: "o1", dataType: "jsonp", jsonpCallback: "preexisting" }), "overwritten")
  await env.wait(20)
  env.L("restored", typeof env.win.preexisting)
  env.win.notfn = 5
  track(env, $.ajax({ url: "o2", dataType: "jsonp", jsonpCallback: "notfn" }), "overwritten-nonfn")
  await env.wait(20)
  env.L("notfn", env.win.notfn)
  const names = []
  for (let i = 0; i < 3; i++) {
    const p = $.ajax({ url: "o3", dataType: "jsonp" })
    names.push(p)
    await env.wait(15)
  }
  env.L("names", env.log.filter((e) => e[0] === "xhr.send").map((e) => e[2].url))
  track(env, $.ajax({ url: "o4fail", dataType: "jsonp", ...hooks($, env) }), "fail")
  await env.wait(20)
  const p = $.ajax({ url: "o5", dataType: "jsonp", beforeSend(x) { env.L("bs") } })
  p.abort()
  await env.wait(20)
  env.L("globals", Object.keys(env.win).filter((k) => /^jQuery./.test(k)).length)
})

// ---------------------------------------------------------------- load
S("fn-load", async ($, env) => {
  env.server = (r) => ({ status: r.url.includes("bad") ? 404 : 200, body: '<div id="a">A<span class="s">S1</span></div><p class="s">P</p><script>window.__ls=(window.__ls||0)+1</script>', headers: "Content-Type: text/html" })
  const root = env.doc.getElementById("root")
  root.innerHTML = '<div id="t1"></div><div id="t2"></div><div id="t3" class="m"></div><div id="t4" class="m"></div>'
  const cb = (tag) => function (rt, st, x) { env.L(tag, this, rt, st, isJqXHR(x), arguments.length) }
  env.L("ret", $("#t1").load("frag.html", cb("l1")).length)
  $("#t2").load("frag.html .s", cb("l2"))
  $(".m").load("frag.html  #a  span ", { a: 1 }, cb("l3"))
  $("#t1").load("frag2.html", "q=1")
  env.L("empty", $("#none").load("frag.html", cb("never")).length)
  $("#t2").load("bad.html", cb("l-bad"))
  await env.wait(40)
  env.L("html", root.innerHTML, env.win.__ls)
  $("#t1").load("frag.html ", cb("trailing-space"))
  await env.wait(20)
  env.L("html2", root.innerHTML)
  $("#t3").load("frag.html", function () { env.L("fn-params", arguments.length) }, cb("extra"))
  await env.wait(20)
  env.L("sends", env.log.filter((e) => e[0] === "xhr.send").map((e) => [e[2].method, e[2].url, e[2].body]))
})

// ---------------------------------------------------------------- serialize
S("param", async ($, env) => {
  const inputs = [
    null, undefined, {}, [], { a: 1, b: "x y", c: "&=?" }, { a: [1, 2, 3] }, { a: [1, [2, 3], { b: 4 }] }, { a: { b: { c: 1 }, d: [5] } },
    { a: null, b: undefined, c: false, d: 0, e: "" }, { f() { return "called" }, g: () => null, h: () => undefined },
    [{ name: "n1", value: "v1" }, { name: "n 2", value: "v&2" }, { name: "n3" }, { value: "noname" }],
    { "a[]": [1, 2] }, { a: { "b[]": [1, 2] } }, "ab", 5, true, { jquery: "x", k: 1 }, { d: new Date(0) },
    { a: [null, undefined, { x: null }] }, { a: [[]] }, { a: { } }, { s: Symbol.iterator ? "sym-skip" : 1 },
  ]
  for (const inp of inputs) {
    let r1, r2
    try { r1 = $.param(inp) } catch (e) { r1 = e }
    try { r2 = $.param(inp, true) } catch (e) { r2 = e }
    env.L(r1, r2)
  }
  class K { constructor() { this.own = 1 } } K.prototype.inherited = 2
  env.L("class", $.param({ k: new K() }), $.param(new K()), $.param({ k: new K() }, true))
  const o = Object.create({ proto: 1 }); o.own = 2
  env.L("proto", $.param(o), $.param({ o }))
  env.L("trad-truthy", $.param({ a: [1, 2] }, 1), $.param({ a: [1, 2] }, ""), $.param({ a: { b: 1 } }, "yes"))
  const root = env.doc.getElementById("root")
  root.innerHTML = '<input name="i1" value="a b"><input name="i2" value="c"><input value="noname">'
  env.L("jq", $.param($("input")), $.param($("#root input"), true))
  env.L("arraylike", $.param({ length: 1, 0: "x" }))
  let thrown; try { $.param({ a: Symbol("q") }) } catch (e) { thrown = e } env.L("sym", thrown)
})
S("serialize-forms", async ($, env) => {
  const root = env.doc.getElementById("root")
  root.innerHTML = `<form id="f">
    <input name="text" value="t v">
    <input name="pw" type="password" value="p">
    <input name="hid" type="hidden" value="h">
    <input name="cb1" type="checkbox" value="c1" checked>
    <input name="cb2" type="checkbox" value="c2">
    <input name="cb3" type="checkbox" checked>
    <input name="r" type="radio" value="r1">
    <input name="r" type="radio" value="r2" checked>
    <input name="dis" value="d" disabled>
    <fieldset disabled><input name="fsdis" value="x"></fieldset>
    <fieldset><legend><input name="leg" value="l"></legend></fieldset>
    <input type="submit" name="sub" value="s">
    <input type="button" name="btn" value="b">
    <input type="image" name="img">
    <input type="reset" name="rst">
    <input type="file" name="file">
    <input type="TEXT" name="upper" value="U">
    <input value="noname">
    <input name="">
    <select name="sel"><option>o1</option><option selected>o2</option></select>
    <select name="selnone"><option value="v1">o1</option></select>
    <select name="multi" multiple><option selected>m1</option><option>m2</option><option selected value="m3v">m3</option></select>
    <select name="multinone" multiple><option>m1</option></select>
    <select name="empty"></select>
    <textarea name="ta">line1
line2\r\nline3</textarea>
    <button name="button" value="bv">B</button>
    <output name="out">o</output>
    <object name="obj"></object>
    <input name="num" type="number" value="12">
    <input name="unicode" value="ü€𝄞">
  </form>
  <input name="outside" value="o" form="f">
  <input name="loose" value="l">
  <div id="d"><input name="indiv" value="i"></div>`
  env.L("arr", $("#f").serializeArray())
  env.L("str", $("#f").serialize())
  env.L("inputs", $("#f input").serializeArray())
  env.L("mixed", $("#f, input[name=loose]").serialize())
  env.L("div", $("#d").serializeArray(), $("#d input").serialize())
  env.L("empty", $().serializeArray(), $().serialize(), $("#nothing").serialize())
  env.L("doc", $(env.doc).serializeArray())
  env.L("ta", $("textarea").serializeArray())
  const f = env.doc.getElementById("f")
  env.L("twice", $([f, f]).serialize())
  env.L("text-node", $(root.childNodes).serializeArray().length)
})

// ---------------------------------------------------------------- public-namespace indirections
S("monkeypatch-ajax", async ($, env) => {
  env.server = ok("real")
  const real = $.ajax
  $.ajax = function (url, opts) { env.L("patched-ajax", url, opts, arguments.length); return real.apply(this, arguments) }
  track(env, $.get("m1", { a: 1 }), "get")
  track(env, $.post("m2", "x=1"), "post")
  track(env, $.getJSON("m3"), "getJSON")
  track(env, $.getScript("m4.js"), "getScript")
  env.doc.getElementById("root").innerHTML = '<div id="lt"></div>'
  env.L("load-ret", $("#lt").load("m5").length)
  $.ajax = real
  await env.wait(30)
})
S("monkeypatch-globalEval-settings", async ($, env) => {
  env.server = (r) => ({ status: 200, body: "window.__ge=1", headers: "Content-Type: text/javascript" })
  const realEval = $.globalEval
  $.globalEval = function (code) { env.L("patched-globalEval", code, arguments.length) }
  track(env, $.ajax({ url: "g1.js", dataType: "script" }), "script")
  await env.wait(20)
  $.globalEval = realEval
  env.L("ge", env.win.__ge)
  const oldSettings = $.ajaxSettings
  $.ajaxSettings = $.extend(true, {}, oldSettings, { type: "POST", headers: { "X-From": "new-settings" } })
  track(env, $.ajax({ url: "g2" }), "replaced-settings")
  await env.wait(20)
  env.L("setup-ret", $.ajaxSetup({ custom: 1 }) === $.ajaxSettings, oldSettings.custom)
})
S("support-cors-runtime", async ($, env) => {
  env.server = ok()
  env.L("initial", $.support.cors, $.support.ajax)
  $.support.cors = false
  track(env, $.ajax({ url: "http://other.org/c1", ...hooks($, env) }), "cors-off-cross")
  track(env, $.ajax({ url: "same" }), "cors-off-same")
  $.support.ajax = false
  track(env, $.ajax({ url: "same2" }), "ajax-flag-off")
  await env.wait(30)
})
S("load-odd-urls", async ($, env) => {
  env.server = (r) => ({ status: 200, body: "<i>" + r.url + "</i>", headers: "Content-Type: text/html" })
  env.doc.getElementById("root").innerHTML = '<div id="o1"></div>'
  for (const u of [undefined, null, 123, { toString() { return "obj.html" } }, "", " ", "a.html\t#x", "b.html  "]) {
    let r
    try { r = $("#o1").load(u).length } catch (e) { r = e }
    env.L("load", u === undefined ? "<undef>" : typeof u === "object" && u ? "obj" : u, r)
  }
  await env.wait(30)
  env.L("html", env.doc.getElementById("o1").innerHTML)
  env.L("sends", env.log.filter((e) => e[0] === "xhr.send").map((e) => e[2].url))
})

S("evalUrl-via-manipulation", async ($, env) => {
  env.server = (r) => ({ status: 200, body: "window.__ev=(window.__ev||[]);window.__ev.push(" + JSON.stringify(r.url) + ")", headers: "Content-Type: text/javascript", delay: r.async === false ? "sync" : 1 })
  globals($, env)
  $("#root").append('<script src="ev1.js"></script><script>window.__ev=(window.__ev||[]);window.__ev.push("inline")</script>')
  $("#root").html('<div><script src="ev2.js" nonce="n1"></script></div>')
  await env.wait(20)
  env.L("ev", env.win.__ev)
})

// ---------------------------------------------------------------- run
function normalize(entries) {
  const map = new Map()
  const s = JSON.stringify(entries).replace(/\d{10,}/g, (m) => {
    if (!map.has(m)) map.set(m, "#N" + map.size)
    return map.get(m)
  })
  return JSON.parse(s)
}

async function runOne(sc, which) {
  const env = await makeEnv(which, sc.envOpts)
  try {
    await sc.fn(env.$, env)
  } catch (e) {
    env.log.push(["SCENARIO-THREW", ser(e, env)])
  }
  await env.wait(5)
  try { env.win.close() } catch {}
  return normalize(env.log)
}

const results = {}
let pass = 0, fail = 0
for (const sc of scenarios) {
  if (only && sc.name !== only) continue
  const a = await runOne(sc, "official")
  const b = await runOne(sc, "lil")
  const diffs = []
  const n = Math.max(a.length, b.length)
  for (let i = 0; i < n; i++) {
    const x = JSON.stringify(a[i]), y = JSON.stringify(b[i])
    if (x !== y) diffs.push({ i, official: a[i], lil: b[i] })
  }
  if (dump) { console.log("== " + sc.name); for (let i = 0; i < a.length; i++) console.log("  O " + JSON.stringify(a[i]) + (JSON.stringify(a[i]) !== JSON.stringify(b[i]) ? "\n  L " + JSON.stringify(b[i]) : "")) }
  results[sc.name] = { entries: a.length, diffs: diffs.length, first: diffs.slice(0, verbose ? 50 : 3) }
  if (diffs.length) fail++
  else pass++
  console.log((diffs.length ? "DIFF " : "ok   ") + sc.name + " (" + a.length + " entries" + (diffs.length ? ", " + diffs.length + " differ" : "") + ")")
  if (diffs.length) for (const d of diffs.slice(0, verbose ? 50 : 3)) console.log("   #" + d.i + "\n     official: " + JSON.stringify(d.official) + "\n     lil:      " + JSON.stringify(d.lil))
}
console.log(`scenarios: ${pass + fail}, identical: ${pass}, differing: ${fail}`)
if (jsonOut) writeFileSync(jsonOut, JSON.stringify(results, null, 1))
process.exit(0)
