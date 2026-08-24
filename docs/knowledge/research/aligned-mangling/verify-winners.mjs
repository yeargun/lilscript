#!/usr/bin/env node
/* Behavioural check on the search winners from libraries.mjs.
 *
 * A structural proof is not a semantic gate. Where a library has a real
 * battery — marked's spec suite, jQuery in jsdom, solid's reactive core — run
 * it. Where it does not, at least import both artifacts and compare their
 * exported surface: names, kinds, arities, and the values of anything plainly
 * inspectable. That is a smoke test and is labelled as one.
 *
 * Usage: node verify-winners.mjs
 */
import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const WORK = "/private/tmp/claude-501/-Users-yeargun-lilscript/68d4d12f-89ad-4d08-8494-1336a12a22e8/scratchpad/verify";
mkdirSync(WORK, { recursive: true });

const load = (path) => import(pathToFileURL(path).href + `?v=${Math.random()}`);

/* Generic smoke test: the module's exported surface. */
async function surface(path) {
  const module = await load(path);
  const out = [];
  for (const key of Object.keys(module).sort()) {
    let value;
    try { value = module[key]; } catch (e) { out.push(`${key}: THREW ${e.message}`); continue; }
    const kind = typeof value;
    if (kind === "function") out.push(`${key}: function/${value.length}`);
    else if (value && kind === "object") {
      const keys = Object.keys(value).sort().slice(0, 40).join(",");
      out.push(`${key}: object{${keys}}`);
    } else out.push(`${key}: ${kind} ${String(value).slice(0, 60)}`);
  }
  return out;
}

async function markedBattery(path) {
  const module = await load(path);
  const parse = module.parse || module.marked || module.default;
  const specs = [
    "/Users/yeargun/markedlil/test/specs/commonmark.0.31.2.json",
    "/Users/yeargun/markedlil/test/specs/gfm.0.29.json",
  ].filter(existsSync);
  const out = [];
  for (const spec of specs) {
    for (const item of JSON.parse(readFileSync(spec, "utf8"))) {
      const markdown = item.markdown ?? item.md ?? item.input;
      if (typeof markdown !== "string") continue;
      try { out.push(String(parse(markdown, item.options || undefined))); }
      catch (e) { out.push(`THREW ${e && e.message}`); }
    }
  }
  return out;
}

async function jqueryBattery(path) {
  const { JSDOM } = await import("/Users/yeargun/lilscript/benchmarks/popular/node_modules/jsdom/lib/api.js");
  const dom = new JSDOM(`<!doctype html><html><body>
    <div id="a" class="box one"><span class="inner">x</span><span class="inner">y</span></div>
    <ul><li>1</li><li>2</li><li>3</li></ul><form><input name="q" value="hi"></form></body></html>`);
  globalThis.window = dom.window; globalThis.document = dom.window.document;
  globalThis.navigator = dom.window.navigator;
  const module = await load(path);
  const $ = module.jQuery || module.default;
  const log = [];
  const go = (label, fn) => { try { log.push(label + "=" + JSON.stringify(fn())); } catch (e) { log.push(label + "=THREW " + e.message); } };
  go("version", () => $.fn.jquery);
  go("select", () => [$("#a").length, $(".inner").length, $("li").length]);
  go("text", () => $(".inner").text());
  go("html", () => $("#a").html());
  go("classes", () => { $("#a").addClass("z").removeClass("one"); return $("#a").attr("class"); });
  go("map", () => $("li").map(function () { return $(this).text(); }).get());
  go("append", () => { $("ul").append("<li>4</li>"); return $("li").length; });
  go("data", () => { $("#a").data("k", { n: 1 }); return $("#a").data("k"); });
  go("serialize", () => $("form").serialize());
  go("event", () => { let n = 0; $("#a").on("c", () => n++); $("#a").trigger("c"); return n; });
  go("deferred", () => { const o = []; const d = $.Deferred(); d.done((v) => o.push(v)); d.resolve(7); return o; });
  go("extend", () => $.extend({ a: 1 }, { b: 2 }));
  return log;
}

async function solidBattery(path) {
  const wrapper = readFileSync("/Users/yeargun/lilscript/labs/solid-client/packages/solidlil/index.js", "utf8");
  const wrapperPath = path.replace(/\.mjs$/, "-wrapper.mjs");
  writeFileSync(wrapperPath, wrapper.replace(/from\s+"\.\/reactive\.generated\.js"/,
    `from ${JSON.stringify("./" + path.split("/").pop())}`));
  const S = await load(wrapperPath);
  const log = [];
  const go = (label, fn) => { try { log.push(label + "=" + JSON.stringify(fn())); } catch (e) { log.push(label + "=THREW " + e.message); } };
  go("signal", () => S.createRoot(() => { const [g, s] = S.createSignal(1); const b = g(); s(5); return [b, g()]; }));
  go("memo", () => S.createRoot(() => { const [g, s] = S.createSignal(2); const m = S.createMemo(() => g() * 2); const b = m(); s(10); return [b, m()]; }));
  go("computed", () => S.createRoot(() => { const seen = []; const [g, s] = S.createSignal(0); S.createComputed(() => seen.push(g())); s(1); s(2); return seen; }));
  go("batch", () => S.createRoot(() => { const seen = []; const [a, sa] = S.createSignal(0); const [b, sb] = S.createSignal(0); S.createComputed(() => seen.push(a() + ":" + b())); S.batch(() => { sa(1); sb(1); }); return seen; }));
  go("cleanup", () => { const seen = []; S.createRoot((d) => { S.onCleanup(() => seen.push("c")); d(); }); return seen; });
  go("mapArray", () => S.createRoot(() => { const [i] = S.createSignal([1, 2, 3]); return S.createMemo(S.mapArray(i, (n) => n * 2))(); }));
  go("surface", () => Object.keys(S).sort().join(","));
  return log;
}

const BATTERIES = {
  "markedlil/marked.raw.js": markedBattery,
  "jquerylil/jquery.esm.js": jqueryBattery,
  "solidlil/reactive.generated.js": solidBattery,
};

const rows = JSON.parse(readFileSync(join(here, "libraries.json"), "utf8"));
let failures = 0, checked = 0;
for (const row of rows) {
  if (row.delta.br11 >= 0) continue;
  const id = `${row.project}/${row.name}`;
  const basePath = join(WORK, `${row.project}-${row.name.replace(/\.js$/, "")}-base.mjs`);
  const winPath = join(WORK, `${row.project}-${row.name.replace(/\.js$/, "")}-win.mjs`);
  const original = row.project === "markedlil" ? "/Users/yeargun/markedlil/dist/marked.raw.js"
    : row.project === "jquerylil" ? "/Users/yeargun/jquerylil/dist/jquery.esm.js"
    : row.project === "solidlil" ? "/Users/yeargun/lilscript/labs/solid-client/packages/solidlil/reactive.generated.js"
    : row.project === "motionlil" ? `/Users/yeargun/motionlil/dist/${row.name}`
    : `/Users/yeargun/posthoglil/dist/${row.name}`;
  writeFileSync(basePath, readFileSync(original, "utf8"));
  writeFileSync(winPath, readFileSync(join(
    "/private/tmp/claude-501/-Users-yeargun-lilscript/68d4d12f-89ad-4d08-8494-1336a12a22e8/scratchpad/lib",
    `${row.project}-${row.name}`), "utf8"));

  const battery = BATTERIES[id] || surface;
  const kind = BATTERIES[id] ? "battery" : "export-surface smoke test";
  let before, after, error = null;
  try { before = await battery(basePath); after = await battery(winPath); }
  catch (e) { error = e; }
  checked++;
  if (error) { failures++; console.log(`${id.padEnd(34)} could not run: ${error.message}`); continue; }
  let differences = 0;
  for (let i = 0; i < Math.max(before.length, after.length); i++) {
    if (before[i] !== after[i]) {
      differences++;
      if (differences <= 2) console.log(`  DIFF ${id} #${i}\n    base: ${String(before[i]).slice(0, 140)}\n    win:  ${String(after[i]).slice(0, 140)}`);
    }
  }
  if (differences) failures++;
  console.log(`${id.padEnd(34)} ${String(row.delta.br11).padStart(6)} br11   ${String(before.length).padStart(4)} observations` +
    `   ${differences === 0 ? "identical" : differences + " DIFFERENCES"}   (${kind})`);
}
console.log(failures ? `\n${failures} of ${checked} failed` : `\nall ${checked} winners behave identically`);
process.exit(failures ? 1 : 0);
