// Differential harness for the event group (event.js, event/trigger.js, deprecated/event.js).
// usage (Node 24): PATH=~/.nvm/versions/node/v24.11.1/bin:$PATH node diff.mjs <lil-esm-1> [<lil-esm-2> ...]
//   [--only=substr] [--verbose | --ctx]; DIFF_JSON=out.json also writes the mismatching scenario names.
// Typical: node diff.mjs base.esm.js variant.esm.js  (every variant mismatch must also be a base mismatch)
// Every scenario runs in a fresh jsdom window per library: official jQuery 3.7.1 (dist/jquery.js)
// and each LilScript build (dist/jquery.esm.js, loaded as a strict script). Results are
// serialized and compared against official.
import { createRequire } from "node:module"
import { readFileSync } from "node:fs"
import { scenarios } from "./scenarios.mjs"

const require = createRequire("/home/azureuser/jquerylil/package.json")
const { JSDOM, VirtualConsole } = require("jsdom")

const args = process.argv.slice(2)
const only = (args.find((a) => a.startsWith("--only=")) || "").slice(7)
const verbose = args.includes("--verbose")
const libs = args.filter((a) => !a.startsWith("--"))
const officialSrc = readFileSync("/home/azureuser/jquerylil/node_modules/jquery/dist/jquery.js", "utf8")

export const HTML =
  "<!doctype html><html><head></head><body>" +
  '<div id="outer" class="o"><div id="mid" class="m">' +
  '<p id="p1" class="x">text<span id="s1" class="y">span</span></p>' +
  '<p id="p2" class="x"><b id="b1">bold</b></p>' +
  '<a id="a1" href="#x">link</a>' +
  '<input id="cb" type="checkbox"><input id="rd" type="radio" name="r"><input id="txt" type="text" value="v">' +
  '<button id="btn" disabled>b</button><button id="btn2">c</button>' +
  '<form id="f1" action="javascript:void 0"><input id="fi" name="fi"></form>' +
  "</div></div>" +
  '<div id="other"><ul id="ul"><li id="li1">1</li><li id="li2">2</li></ul></div>' +
  "</body></html>"

function esmToScript(src) {
  const body = src.replace(/;?export\s*\{[^}]*\}\s*;?\s*export default \w+;?\s*$/, "")
  if (body === src) throw new Error("unexpected ESM tail")
  return '"use strict";' + body + ";window.__jq=jQuery;"
}

function makeEnv(kind, code) {
  const vc = new VirtualConsole()
  const dom = new JSDOM(HTML, { runScripts: "outside-only", pretendToBeVisual: true, url: "http://localhost/", virtualConsole: vc })
  const w = dom.window
  w.eval(code)
  const $ = kind === "official" ? w.jQuery : w.__jq
  return { $, w, d: w.document, dom }
}

function idOf(env, n) {
  if (n === undefined) return "<u>"
  if (n === null) return "<null>"
  if (n === env.w) return "win"
  if (n === env.d) return "doc"
  if (typeof n === "object" && n.nodeType) return n.id || n.nodeName
  if (typeof n === "object") return "obj" + (n.name ? ":" + n.name : "")
  return String(n)
}

function ser(env, x, depth = 0, seen = new Set()) {
  if (x === undefined) return "<u>"
  if (x === null || typeof x === "boolean" || typeof x === "string") return x
  if (typeof x === "number") return Number.isNaN(x) ? "<NaN>" : x
  if (typeof x === "function") return "<fn>"
  if (typeof x !== "object") return "<" + typeof x + ">"
  if (x === env.w) return "<win>"
  if (x === env.d) return "<doc>"
  if (typeof x.nodeType === "number" && typeof x.nodeName === "string") return "<node " + (x.id || x.nodeName) + ">"
  const tag = Object.prototype.toString.call(x)
  if (tag === "[object RegExp]") return "<re " + String(x) + ">"
  if (x.jquery && typeof x.length === "number") {
    return { jq: Array.from({ length: x.length }, (_, i) => ser(env, x[i], depth + 1, seen)) }
  }
  if (seen.has(x)) return "<cycle>"
  if (depth > 4) return "<deep>"
  seen.add(x)
  let out
  if (Array.isArray(x)) out = x.map((v) => ser(env, v, depth + 1, seen))
  else {
    out = {}
    for (const k of Object.keys(x)) {
      const nk = /^jQuery\d+$/.test(k) ? "<expando>" : k
      out[nk] = k === "timeStamp" ? "<ts>" : ser(env, x[k], depth + 1, seen)
    }
  }
  seen.delete(x)
  return out
}

function runScenario(kind, code, fn) {
  const env = makeEnv(kind, code)
  const log = []
  env.log = log
  env.id = (n) => idOf(env, n)
  env.ser = (v) => ser(env, v)
  // a logging handler factory: records name, this, and a digest of the event
  env.H = (name, ret) =>
    function (e, ...rest) {
      log.push([
        name,
        idOf(env, this),
        e && e.type,
        e && e.namespace,
        e && idOf(env, e.target),
        e && idOf(env, e.currentTarget),
        e && idOf(env, e.delegateTarget),
        e && e.data === undefined ? "<u>" : ser(env, e && e.data),
        rest.length ? ser(env, rest) : "",
      ])
      return typeof ret === "function" ? ret.call(this, e) : ret
    }
  let result
  try {
    result = { ret: ser(env, fn(env)), log: ser(env, log) }
  } catch (e) {
    result = { err: (e && e.name) || String(e), log: ser(env, log) }
    if (verbose) result.msg = String(e && e.message)
  }
  env.w.close()
  return result
}

const officialCode = officialSrc
const libCodes = libs.map((p) => esmToScript(readFileSync(p, "utf8")))
const selected = scenarios.filter(([name]) => !only || name.includes(only))
const mismatches = libs.map(() => [])
for (const [name, fn] of selected) {
  const want = JSON.stringify(runScenario("official", officialCode, fn))
  libCodes.forEach((code, i) => {
    const got = JSON.stringify(runScenario("lil", code, fn))
    if (got !== want) mismatches[i].push([name, want, got])
  })
}
libs.forEach((p, i) => {
  console.log(`== ${p}: ${selected.length} scenarios, ${mismatches[i].length} mismatches vs official`)
  for (const [name, want, got] of mismatches[i]) {
    console.log(`  MISMATCH ${name}`)
    if (verbose) {
      console.log(`    official: ${want}`)
      console.log(`    lil:      ${got}`)
    } else if (args.includes("--ctx")) {
      let k = 0
      while (k < want.length && want[k] === got[k]) k++
      console.log(`    @${k} official: ...${want.slice(Math.max(0, k - 60), k + 60)}`)
      console.log(`    @${k} lil:      ...${got.slice(Math.max(0, k - 60), k + 60)}`)
    }
  }
})
// machine-readable summary for comparing base vs variant
if (process.env.DIFF_JSON) {
  const { writeFileSync } = await import("node:fs")
  writeFileSync(process.env.DIFF_JSON, JSON.stringify(libs.map((p, i) => ({ lib: p, total: selected.length, mismatches: mismatches[i].map((m) => m[0]) })), null, 1))
}
