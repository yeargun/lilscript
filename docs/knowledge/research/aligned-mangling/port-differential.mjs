#!/usr/bin/env node
/* Behavioural A/B for the three ports.

   `ports.mjs` finds a cheaper legal naming for an artifact. This decides
   whether it is really the same program: the mutant is written next to a copy
   of the baseline, both are imported, and every port is driven through its own
   battery. Any difference at all fails the row.

     jquerylil   — 37 jsdom observations
     markedlil   — every CommonMark 0.31.2 and GFM 0.29 spec case
     solidlil    — the reactive core: signals, memos, effects, batches, stores

   Usage: node port-differential.mjs [--only <substring>] */
import { readFileSync, writeFileSync, mkdirSync, existsSync } from "node:fs";
import { dirname, join, basename } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { analyze, rename, verify } from "./scope.mjs";
import { assign, adaptiveAlphabet, ALPHABETS } from "./mangle.mjs";
import { PORTS, analysePort } from "./ports.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const WORK = "/private/tmp/claude-501/-Users-yeargun-lilscript/68d4d12f-89ad-4d08-8494-1336a12a22e8/scratchpad/port-diff";
mkdirSync(WORK, { recursive: true });

const load = (path) => import(pathToFileURL(path).href + `?v=${Date.now()}${Math.random()}`);

/* --- batteries -------------------------------------------------------- */

async function markedBattery(path) {
  const module = await load(path);
  const parse = module.parse || module.marked || module.default;
  const specs = [
    "/Users/yeargun/markedlil/test/specs/commonmark.0.31.2.json",
    "/Users/yeargun/markedlil/test/specs/gfm.0.29.json",
  ].filter(existsSync);
  const out = [];
  for (const spec of specs) {
    const cases = JSON.parse(readFileSync(spec, "utf8"));
    for (const item of cases) {
      const markdown = item.markdown ?? item.md ?? item.input;
      if (typeof markdown !== "string") continue;
      try {
        out.push(String(parse(markdown, item.options || undefined)));
      } catch (e) {
        out.push(`THREW ${e && e.message}`);
      }
    }
  }
  return out;
}

/* The reactive core is the compiler's emit; solidlil/index.js is the
   hand-written wrapper that turns it into the SolidJS API. Copy the wrapper
   next to the artifact under test, point its one import at it, and drive the
   public API — that exercises the core the way the package does. */
async function solidBattery(path) {
  const wrapperSource = readFileSync(
    "/Users/yeargun/lilscript/labs/solid-client/packages/solidlil/index.js", "utf8");
  const wrapperPath = path.replace(/\.mjs$/, "-wrapper.mjs");
  writeFileSync(wrapperPath, wrapperSource.replace(
    /from\s+"\.\/reactive\.generated\.js"/, `from ${JSON.stringify("./" + basename(path))}`));
  const S = await load(wrapperPath);
  const log = [];
  const attempt = (label, fn) => {
    try { log.push(`${label}=${JSON.stringify(fn())}`); }
    catch (e) { log.push(`${label}=THREW ${e && e.message}`); }
  };
  const root = (fn) => S.createRoot(fn);

  attempt("signal", () => root(() => {
    const [get, set] = S.createSignal(1);
    const before = get();
    set(5);
    return [before, get()];
  }));
  attempt("signal-fn-update", () => root(() => {
    const [get, set] = S.createSignal(2);
    set((n) => n * 10);
    return get();
  }));
  attempt("memo", () => root(() => {
    const [get, set] = S.createSignal(2);
    const doubled = S.createMemo(() => get() * 2);
    const before = doubled();
    set(10);
    return [before, doubled()];
  }));
  attempt("computed-order", () => root(() => {
    const seen = [];
    const [get, set] = S.createSignal(0);
    S.createComputed(() => seen.push("c" + get()));
    set(1); set(2);
    return seen;
  }));
  attempt("render-effect", () => root(() => {
    const seen = [];
    const [get, set] = S.createSignal(0);
    S.createRenderEffect(() => seen.push(get()));
    set(1);
    return seen;
  }));
  attempt("batch", () => root(() => {
    const seen = [];
    const [a, setA] = S.createSignal(0);
    const [b, setB] = S.createSignal(0);
    S.createComputed(() => seen.push(a() + ":" + b()));
    S.batch(() => { setA(1); setB(1); });
    return seen;
  }));
  attempt("untrack", () => root(() => {
    const seen = [];
    const [a, setA] = S.createSignal(0);
    S.createComputed(() => seen.push(S.untrack(() => a())));
    setA(1);
    return seen;
  }));
  attempt("on", () => root(() => {
    if (!S.on) return "n/a";
    const seen = [];
    const [a, setA] = S.createSignal(0);
    S.createComputed(S.on(a, (v, prev) => seen.push([v, prev])));
    setA(3);
    return seen;
  }));
  attempt("cleanup", () => {
    const seen = [];
    S.createRoot((dispose) => {
      S.onCleanup(() => seen.push("cleaned"));
      dispose();
    });
    return seen;
  });
  attempt("nested-roots", () => root(() => {
    const [a, setA] = S.createSignal(1);
    let inner = null;
    S.createRoot(() => { inner = S.createMemo(() => a() + 1); });
    setA(4);
    return inner ? inner() : "none";
  }));
  attempt("context", () => root(() => {
    if (!S.createContext) return "n/a";
    const ctx = S.createContext("default");
    return [S.useContext(ctx), typeof ctx];
  }));
  attempt("selector", () => root(() => {
    if (!S.createSelector) return "n/a";
    const [get, set] = S.createSignal(1);
    const isSelected = S.createSelector(get);
    const before = [isSelected(1), isSelected(2)];
    set(2);
    return [before, [isSelected(1), isSelected(2)]];
  }));
  attempt("mapArray", () => root(() => {
    if (!S.mapArray) return "n/a";
    const [items] = S.createSignal([1, 2, 3]);
    const mapped = S.createMemo(S.mapArray(items, (n) => n * 2));
    return mapped();
  }));
  attempt("indexArray", () => root(() => {
    if (!S.indexArray) return "n/a";
    const [items] = S.createSignal(["a", "b"]);
    const mapped = S.createMemo(S.indexArray(items, (item, i) => item() + i));
    return mapped();
  }));
  attempt("children", () => root(() => {
    if (!S.children) return "n/a";
    const resolved = S.children(() => [1, 2, 3]);
    return resolved();
  }));
  attempt("owner", () => root(() => [typeof S.getOwner(), typeof S.runWithOwner]));
  attempt("uniqueId-shape", () => root(() => typeof S.createUniqueId()));
  attempt("api-surface", () => Object.keys(S).sort().join(","));
  return log;
}

async function jqueryBattery(path) {
  const { JSDOM } = await import("/Users/yeargun/lilscript/benchmarks/popular/node_modules/jsdom/lib/api.js");
  const dom = new JSDOM(`<!doctype html><html><body>
    <div id="a" class="box one" data-k="v"><span class="inner">x</span><span class="inner">y</span></div>
    <ul><li>1</li><li>2</li><li>3</li></ul>
    <form><input name="q" value="hello"><input type="checkbox" checked></form>
  </body></html>`);
  globalThis.window = dom.window;
  globalThis.document = dom.window.document;
  globalThis.navigator = dom.window.navigator;
  const module = await load(path);
  const $ = module.jQuery || module.default;
  const log = [];
  const attempt = (label, fn) => {
    try { log.push(`${label}=${JSON.stringify(fn())}`); }
    catch (e) { log.push(`${label}=THREW ${e && e.message}`); }
  };
  attempt("version", () => $.fn.jquery);
  attempt("select", () => [$("#a").length, $(".inner").length, $("li").length]);
  attempt("text", () => $(".inner").text());
  attempt("html", () => $("#a").html());
  attempt("attr", () => $("#a").attr("data-k"));
  attempt("classes", () => { $("#a").addClass("z").removeClass("one"); return $("#a").attr("class"); });
  attempt("css", () => { $("#a").css("color", "red"); return $("#a").css("color"); });
  attempt("map", () => $("li").map(function () { return $(this).text(); }).get());
  attempt("each", () => { const o = []; $("li").each((i, el) => o.push(i + el.textContent)); return o; });
  attempt("append", () => { $("ul").append("<li>4</li>"); return $("li").length; });
  attempt("clone", () => $("#a").clone().find(".inner").length);
  attempt("data", () => { $("#a").data("x", { n: 1 }); return $("#a").data("x"); });
  attempt("val", () => $("input[name=q]").val());
  attempt("serialize", () => $("form").serialize());
  attempt("prop", () => $("input[type=checkbox]").prop("checked"));
  attempt("closest", () => $(".inner").closest("div").attr("id"));
  attempt("traverse", () => [$(".inner").parent().attr("id"), $("#a").children().length,
    $("li").eq(0).next().text(), $("li").eq(1).siblings().length]);
  attempt("extend", () => $.extend({ a: 1 }, { b: 2 }, { a: 3 }));
  attempt("utils", () => [$.isPlainObject({}), $.isPlainObject([]), $.grep([1, 2, 3, 4], (n) => n % 2 === 0),
    $.map([1, 2], (n) => n * 2)]);
  attempt("event", () => {
    let seen = 0;
    $("#a").on("click.ns", () => { seen++; });
    $("#a").trigger("click");
    $("#a").off("click.ns");
    $("#a").trigger("click");
    return seen;
  });
  attempt("delegated", () => {
    const seen = [];
    $("ul").on("click", "li", function () { seen.push(this.textContent); });
    $("li").eq(2).trigger("click");
    return seen;
  });
  attempt("deferred", () => {
    const out = [];
    const d = $.Deferred();
    d.done((v) => out.push(["done", v])).fail((v) => out.push(["fail", v])).progress((v) => out.push(["progress", v]));
    d.notify("n"); d.resolve(42);
    return out;
  });
  attempt("when", () => { const o = []; $.when(1, 2).done((...a) => o.push(a)); return o; });
  attempt("state", () => { const d = $.Deferred(); d.reject("e"); return d.state(); });
  attempt("wrap", () => { $(".inner").eq(0).wrap("<b></b>"); return $("#a").find("b .inner").length; });
  attempt("index", () => $("li").eq(2).index());
  attempt("param", () => $.param({ a: 1, b: [2, 3] }));
  attempt("dom", () => dom.window.document.body.innerHTML.length);
  return log;
}

const BATTERIES = {
  "jquerylil-raw": jqueryBattery,
  "jquerylil-esm": jqueryBattery,
  "markedlil-raw": markedBattery,
  "markedlil-bytes": markedBattery,
  "markedlil-gzip": markedBattery,
  "markedlil-esm": markedBattery,
  "solidlil-reactive": solidBattery,
  /* The remaining solid rows are application bundles: they run a browser app
     on import, so there is no library surface to drive. Their naming rows are
     reported by ports.mjs and are not claimed as verified. */
};

function batteryFor(id) {
  return BATTERIES[id] || null;
}

/* --- run --------------------------------------------------------------- */
const flag = process.argv.indexOf("--only");
const filter = flag >= 0 ? process.argv[flag + 1] : null;
let failures = 0;

for (const [id, path] of Object.entries(PORTS)) {
  if (filter && !id.includes(filter)) continue;
  const battery = batteryFor(id);
  if (!battery || !existsSync(path)) continue;

  const source = readFileSync(path, "utf8");
  const analysis = analyze(source, { renameModuleTopLevel: true });
  const row = analysePort(id, path);
  const [order, alphabetName] = row.best.label.split("/");
  const alphabets = {
    abc: ALPHABETS.abc, etn: ALPHABETS.etn,
    adaptive: adaptiveAlphabet(analysis, { mode: "all" }),
    dialect: adaptiveAlphabet(analysis, { mode: "dialect" }),
    reversed: [...ALPHABETS.abc].reverse().join(""),
  };
  const mapping = assign(analysis, { order, alphabet: alphabets[alphabetName] });
  const mutantText = rename(analysis, mapping);
  const check = verify(analysis, mutantText, mapping);

  /* always .mjs: these are modules, and the scratchpad has no package.json */
  const baseFile = join(WORK, `${id.replace(/[^a-z0-9-]/gi, "_")}-base.mjs`);
  const mutantFile = join(WORK, `${id.replace(/[^a-z0-9-]/gi, "_")}-mutant.mjs`);
  writeFileSync(baseFile, source);
  writeFileSync(mutantFile, mutantText);

  let before, after, error = null;
  try {
    before = await battery(baseFile);
    after = await battery(mutantFile);
  } catch (e) { error = e; }

  if (error) {
    failures++;
    console.log(`${id.padEnd(20)} battery failed to run: ${error.message}`);
    continue;
  }
  let differences = 0;
  for (let i = 0; i < Math.max(before.length, after.length); i++) {
    if (before[i] !== after[i]) {
      differences++;
      if (differences <= 2) {
        console.log(`  DIFF #${i}\n    baseline: ${String(before[i]).slice(0, 160)}\n    mutant:   ${String(after[i]).slice(0, 160)}`);
      }
    }
  }
  if (differences) failures++;
  const throws = before.filter((l) => String(l).includes("THREW")).length;
  console.log(`${id.padEnd(20)} ${row.best.label.padEnd(18)} br11 ${row.best.br11 - row.base.br11 >= 0 ? "+" : ""}${row.best.br11 - row.base.br11}` +
    `  ${String(before.length).padStart(4)} observations  ${differences === 0 ? "identical" : differences + " DIFFERENCES"}` +
    `  ${check.ok ? "binding graph ok" : "GRAPH: " + check.why}${throws ? `  (${throws} baseline throws)` : ""}`);
}

console.log(failures ? `\n${failures} port(s) failed` : "\nevery port: identical behaviour");
process.exit(failures ? 1 : 0);
