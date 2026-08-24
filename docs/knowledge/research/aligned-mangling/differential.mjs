#!/usr/bin/env node
/* Behavioural A/B for a renaming mutation.

   The structural check in scope.mjs proves the binding graph survived; this
   proves the program does. Both artifacts are loaded into their own jsdom and
   driven through the same battery, and every observation must match. It
   compares the mutant against the *baseline artifact*, not against upstream
   jQuery, so a pre-existing port difference cannot be mistaken for a renaming
   bug.

   Usage: node differential.mjs <baseline.js> <mutant.js> */
import { pathToFileURL } from "node:url";
import { resolve } from "node:path";

async function observe(path) {
  const { JSDOM } = await import("/Users/yeargun/lilscript/benchmarks/popular/node_modules/jsdom/lib/api.js");
  const dom = new JSDOM(`<!doctype html><html><body>
    <div id="a" class="box one" data-k="v"><span class="inner">x</span><span class="inner">y</span></div>
    <ul><li>1</li><li>2</li><li>3</li></ul>
    <form><input name="q" value="hello"><input type="checkbox" checked></form>
  </body></html>`);
  globalThis.window = dom.window;
  globalThis.document = dom.window.document;
  globalThis.navigator = dom.window.navigator;
  const module = await import(pathToFileURL(resolve(path)).href + `?v=${Math.random()}`);
  const $ = module.jQuery || module.default;
  const log = [];
  const say = (label, value) => log.push(`${label}=${JSON.stringify(value)}`);
  const attempt = (label, fn) => {
    try { say(label, fn()); } catch (e) { say(label, `THREW ${e && e.message}`); }
  };

  attempt("version", () => $.fn.jquery);
  attempt("select-id", () => $("#a").length);
  attempt("select-class", () => $(".inner").length);
  attempt("text", () => $(".inner").text());
  attempt("html", () => $("#a").html());
  attempt("attr", () => $("#a").attr("data-k"));
  attempt("hasClass", () => [$("#a").hasClass("box"), $("#a").hasClass("nope")]);
  attempt("addRemoveClass", () => { $("#a").addClass("z").removeClass("one"); return $("#a").attr("class"); });
  attempt("css", () => { $("#a").css("color", "red"); return $("#a").css("color"); });
  attempt("map", () => $("li").map(function () { return $(this).text(); }).get());
  attempt("filter", () => $("li").filter(":first").text());
  attempt("each", () => { const out = []; $("li").each((i, el) => out.push(i + el.textContent)); return out; });
  attempt("append", () => { $("ul").append("<li>4</li>"); return $("li").length; });
  attempt("clone", () => $("#a").clone().find(".inner").length);
  attempt("data", () => { $("#a").data("x", { n: 1 }); return $("#a").data("x"); });
  attempt("val", () => $("input[name=q]").val());
  attempt("serialize", () => $("form").serialize());
  attempt("prop", () => $("input[type=checkbox]").prop("checked"));
  attempt("closest", () => $(".inner").closest("div").attr("id"));
  attempt("parentChildren", () => [$(".inner").parent().attr("id"), $("#a").children().length]);
  attempt("siblingsNext", () => [$("li").eq(0).next().text(), $("li").eq(1).siblings().length]);
  attempt("extend", () => $.extend({ a: 1 }, { b: 2 }, { a: 3 }));
  attempt("isPlainObject", () => [$.isPlainObject({}), $.isPlainObject([]), $.isPlainObject(null)]);
  attempt("type-checks", () => [$.isArray ? $.isArray([]) : "n/a", typeof $.each, typeof $.grep]);
  attempt("grepMapUtil", () => [$.grep([1, 2, 3, 4], (n) => n % 2 === 0), $.map([1, 2], (n) => n * 2)]);
  attempt("trimLike", () => $.trim ? $.trim("  pad  ") : "n/a");
  attempt("event", () => {
    let seen = 0;
    $("#a").on("click.ns", () => { seen++; });
    $("#a").trigger("click");
    $("#a").off("click.ns");
    $("#a").trigger("click");
    return seen;
  });
  attempt("delegated", () => {
    let seen = [];
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
  attempt("when", () => {
    const out = [];
    $.when(1, 2).done((...args) => out.push(args));
    return out;
  });
  attempt("promiseState", () => { const d = $.Deferred(); d.reject("e"); return d.state(); });
  attempt("queue", () => { const el = $("#a"); el.queue("fx", () => {}); return el.queue("fx").length; });
  attempt("wrapUnwrap", () => { $(".inner").eq(0).wrap("<b></b>"); return $("#a").find("b .inner").length; });
  attempt("index", () => $("li").eq(2).index());
  attempt("isFunction", () => typeof $.fn.init);
  attempt("param", () => $.param({ a: 1, b: [2, 3] }));
  attempt("nodeText", () => dom.window.document.body.innerHTML.length);
  return log;
}

const [, , basePath, mutantPath] = process.argv;
if (!basePath || !mutantPath) {
  console.error("usage: node differential.mjs <baseline.js> <mutant.js>");
  process.exit(2);
}
const before = await observe(basePath);
const after = await observe(mutantPath);
let differences = 0;
for (let i = 0; i < Math.max(before.length, after.length); i++) {
  if (before[i] !== after[i]) {
    differences++;
    console.log(`DIFF\n  baseline: ${before[i]}\n  mutant:   ${after[i]}`);
  }
}
const threw = before.filter((l) => l.includes("THREW")).length;
console.log(`${before.length} observations, ${differences} differences, ${threw} baseline throws`);
process.exit(differences ? 1 : 0);
