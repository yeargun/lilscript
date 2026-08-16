import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { spawnSync } from "node:child_process";
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, join, resolve } from "node:path";
import { build as esbuild } from "esbuild";
import { mkdirSync } from "node:fs";
import {
  JQUERY_LILSCRIPT_ARTIFACT_ENV,
  resolveJqueryLilscriptArtifact,
} from "./jquery-benchmark-artifact.mjs";

const labRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(labRoot, "../..");
const compiler = join(repoRoot, "target/release/lilscript");
const buildRoot = join(labRoot, "build");
const compiled = join(labRoot, "ports/jquery/jquery-lilscript.raw.js");
const defaultOutFile = join(buildRoot, "jquery-lilscript.js");
const selectedArtifact = Object.hasOwn(
  process.env,
  JQUERY_LILSCRIPT_ARTIFACT_ENV,
)
  ? resolveJqueryLilscriptArtifact({
      environment: process.env,
      workingDirectory: labRoot,
      defaultArtifactPath: defaultOutFile,
    })
  : null;
const outFile = selectedArtifact?.path ?? defaultOutFile;

mkdirSync(buildRoot, { recursive: true });

function run(program, args) {
  const result = spawnSync(program, args, {
    cwd: labRoot,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`${program} ${args.join(" ")}\n${result.stdout}${result.stderr}`);
  }
  return result.stdout.trim();
}

if (selectedArtifact === null) {
  run(compiler, [
    join(labRoot, "ports/jquery/entry.lil"),
    "--mode",
    "development",
    "--target",
    "js-module",
    "-o",
    compiled,
  ]);

  await esbuild({
    absWorkingDir: join(labRoot, "ports/jquery"),
    entryPoints: [compiled],
    outfile: outFile,
    bundle: true,
    format: "esm",
    platform: "neutral",
    write: true,
  });
}

const require = createRequire(import.meta.url);
const upstreamFactory = require("jquery");
const { JSDOM } = await import("jsdom");
const dom = new JSDOM("<!doctype html><html><body></body></html>");
globalThis.window = dom.window;
globalThis.document = dom.window.document;
const upstream = upstreamFactory(dom.window);
const lil = await import(pathToFileURL(outFile).href);
const $u = upstream;
const $l = lil.jQuery;

assert.equal($l.fn.jquery, "3.7.1");
assert.equal($u.fn.jquery, "3.7.1");

assert.equal($l.isPlainObject({ a: 1 }), true);
assert.equal($u.isPlainObject({ a: 1 }), true);
assert.equal($l.isPlainObject(null), $u.isPlainObject(null));
assert.equal($l.isPlainObject([]), $u.isPlainObject([]));
assert.equal($l.isPlainObject(Object.create(null)), $u.isPlainObject(Object.create(null)));

assert.equal($l.isEmptyObject({}), true);
assert.equal($u.isEmptyObject({}), true);
assert.equal($l.isEmptyObject({ a: 1 }), false);
assert.equal($u.isEmptyObject({ a: 1 }), false);

const eachU = [];
const eachL = [];
$u.each([1, 2, 3], function (i, v) {
  eachU.push([i, v, this]);
});
$l.each([1, 2, 3], function (i, v) {
  eachL.push([i, v, this]);
});
assert.deepEqual(eachL, eachU);

const eachObjU = [];
const eachObjL = [];
$u.each({ a: 1, b: 2 }, function (k, v) {
  eachObjU.push([k, v, this]);
});
$l.each({ a: 1, b: 2 }, function (k, v) {
  eachObjL.push([k, v, this]);
});
assert.deepEqual(eachObjL, eachObjU);

assert.deepEqual($l.merge([1], [2, 3]), $u.merge([1], [2, 3]));
assert.deepEqual($l.grep([1, 2, 3, 4], (n) => n % 2 === 0), $u.grep([1, 2, 3, 4], (n) => n % 2 === 0));
assert.deepEqual(
  $l.grep([1, 2, 3, 4], (n) => n % 2 === 0, true),
  $u.grep([1, 2, 3, 4], (n) => n % 2 === 0, true),
);
assert.deepEqual(
  $l.map([1, 2, 3], (n) => n * 2),
  $u.map([1, 2, 3], (n) => n * 2),
);
assert.deepEqual($l.makeArray("ab"), $u.makeArray("ab"));
assert.deepEqual($l.makeArray([1, 2]), $u.makeArray([1, 2]));
assert.equal($l.inArray(2, [1, 2, 3]), $u.inArray(2, [1, 2, 3]));
assert.equal($l.inArray(9, [1, 2, 3]), $u.inArray(9, [1, 2, 3]));

const deepU = $u.extend(true, { a: { b: 1 } }, { a: { c: 2 }, d: 3 });
const deepL = $l.extend(true, { a: { b: 1 } }, { a: { c: 2 }, d: 3 });
assert.deepEqual(deepL, deepU);

const shallowU = $u.extend({ a: 1 }, { b: 2 });
const shallowL = $l.extend({ a: 1 }, { b: 2 });
assert.deepEqual(shallowL, shallowU);

assert.equal($l.camelCase("font-size"), "fontSize");
assert.equal($l.camelCase("-ms-flex"), "msFlex");
assert.equal($l.type([]), "array");
assert.equal($l.type(null), "null");
assert.equal($l.type(1), "number");

const collection = $l(undefined);
assert.equal(collection.length, 0);
assert.deepEqual(collection.toArray(), []);
assert.equal(collection.eq(0).length, 0);
assert.equal(collection.end().length, 0);

let mapped = $l.merge($l(), [10, 20, 30]);
assert.deepEqual(
  mapped
    .map(function (i, v) {
      return v + 1;
    })
    .toArray(),
  [11, 21, 31],
);

function settle(ms = 20) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function deferredResolveReject($) {
  const out = [];
  const d1 = $.Deferred();
  d1.done(function (v) {
    out.push(["done", v, this]);
  });
  d1.resolve(42);
  assert.equal(d1.state(), "resolved");

  const d2 = $.Deferred();
  d2.fail(function (v) {
    out.push(["fail", v]);
  });
  d2.reject("err");
  assert.equal(d2.state(), "rejected");

  const d3 = $.Deferred();
  d3.progress(function (v) {
    out.push(["progress", v]);
  });
  d3.notify("n");
  d3.resolve("ok");
  return out;
}

assert.deepEqual(deferredResolveReject($l), deferredResolveReject($u));

function deferredAlwaysCatch($) {
  const out = [];
  const d = $.Deferred();
  d.always(function (v) {
    out.push(["always", v]);
  });
  d.resolve("A");

  const d2 = $.Deferred();
  d2.catch(function (v) {
    out.push(["catch", v]);
  });
  d2.reject("B");
  return out;
}

assert.deepEqual(deferredAlwaysCatch($l), deferredAlwaysCatch($u));

async function deferredThen($) {
  const out = [];
  const d = $.Deferred();
  d.then(
    function (v) {
      out.push(["then", v, this]);
      return v + 1;
    },
    null,
  ).then(function (v) {
    out.push(["then2", v]);
  });
  d.resolve(10);
  await settle();
  return out;
}

{
  const [u, l] = await Promise.all([deferredThen($u), deferredThen($l)]);
  assert.deepEqual(l, u);
}

async function deferredThenReject($) {
  const out = [];
  const d = $.Deferred();
  d.then(null, function (v) {
    out.push(["rej", v]);
    return "recovered";
  }).then(function (v) {
    out.push(["ok", v]);
  });
  d.reject("boom");
  await settle();
  return out;
}

{
  const [u, l] = await Promise.all([deferredThenReject($u), deferredThenReject($l)]);
  assert.deepEqual(l, u);
}

function deferredWhenValues($) {
  const out = [];
  $.when(1, 2).done(function (a, b) {
    out.push(["multi", a, b, this]);
  });
  $.when().done(function () {
    out.push(["empty", arguments.length]);
  });
  $.when(7).done(function (v) {
    out.push(["single", v]);
  });
  return out;
}

assert.deepEqual(deferredWhenValues($l), deferredWhenValues($u));

async function deferredWhenThenable($) {
  const out = [];
  const d = $.Deferred();
  $.when(d).done(function (v) {
    out.push(v);
  });
  d.resolve("w");
  await settle();
  return out;
}

{
  const [u, l] = await Promise.all([deferredWhenThenable($u), deferredWhenThenable($l)]);
  assert.deepEqual(l, u);
}

function deferredPipe($) {
  const out = [];
  const d = $.Deferred();
  d.pipe(
    function (v) {
      return v * 2;
    },
    null,
  ).done(function (v) {
    out.push(v);
  });
  d.resolve(5);
  return out;
}

assert.deepEqual(deferredPipe($l), deferredPipe($u));

function deferredCtor($) {
  const out = [];
  $.Deferred(function (d) {
    out.push(d.state());
    d.resolve("ctor");
  }).done(function (v) {
    out.push(v);
  });
  return out;
}

assert.deepEqual(deferredCtor($l), deferredCtor($u));

console.log("jquery-upstream:core:ok");
console.log("jquery-upstream:deferred:ok");

function dataBasics($) {
  const out = [];
  const obj = {};
  $.data(obj, "a", 1);
  out.push(["get", $.data(obj, "a")]);
  $.data(obj, { b: 2, "c-d": 3 });
  out.push(["multi", $.data(obj, "b"), $.data(obj, "c-d"), $.data(obj, "cD")]);
  out.push(["has", $.hasData(obj)]);
  $.removeData(obj, "a");
  out.push(["removed", $.data(obj, "a"), $.hasData(obj)]);
  $.removeData(obj);
  out.push(["cleared", $.hasData(obj)]);

  const el = dom.window.document.createElement("div");
  el.setAttribute("data-foo-bar", "42");
  el.setAttribute("data-flag", "true");
  const col = $.merge($(), [el]);
  out.push(["attrNum", col.data("fooBar")]);
  out.push(["attrBool", col.data("flag")]);
  col.data("x", "y");
  out.push(["inst", col.data("x"), $.data(el, "x")]);
  col.removeData("x");
  out.push(["instRemoved", col.data("x")]);
  return out;
}

assert.deepEqual(dataBasics($l), dataBasics($u));

function queueBasics($) {
  const out = [];
  const el = dom.window.document.createElement("div");
  const col = $.merge($(), [el]);

  out.push(["empty", $.queue(el)]);
  col.queue(function (next) {
    out.push(["step1", this === el]);
    next();
  });
  out.push(["queued", $.queue(el).length]);
  col.dequeue();
  out.push(["after1", $.queue(el).length, $.hasData(el)]);

  const el2 = dom.window.document.createElement("span");
  const col2 = $.merge($(), [el2]);
  let ran = 0;
  col2.queue("fx", function (next) {
    ran += 1;
    next();
  }).queue(function (next) {
    ran += 10;
    next();
  });
  out.push(["autoDequeue", ran, $.queue(el2).length]);

  const el3 = dom.window.document.createElement("p");
  $.queue(el3, "custom", function (next) {
    out.push("custom");
    next();
  });
  out.push(["customLen", $.queue(el3, "custom").length]);
  $.dequeue(el3, "custom");
  out.push(["customDone", $.queue(el3, "custom").length]);

  return out;
}

assert.deepEqual(queueBasics($l), queueBasics($u));

console.log("jquery-upstream:data:ok");
console.log("jquery-upstream:queue:ok");

function attrBasics($) {
  const out = [];
  const el = dom.window.document.createElement("div");
  const col = $.merge($(), [el]);

  col.attr("data-x", "1");
  out.push(["attr-get", col.attr("data-x")]);
  col.attr({ "data-y": "2", "data-z": "3" });
  out.push(["attr-multi", col.attr("data-y"), col.attr("data-z")]);
  col.removeAttr("data-y data-z");
  out.push(["attr-removed", col.attr("data-y"), col.attr("data-z")]);

  const checkbox = dom.window.document.createElement("input");
  checkbox.type = "checkbox";
  const checkboxCol = $.merge($(), [checkbox]);
  out.push(["checked-before", checkboxCol.attr("checked")]);
  checkboxCol.attr("checked", "checked");
  out.push(["checked-after", checkboxCol.attr("checked"), checkbox.checked]);
  checkboxCol.removeAttr("checked");
  out.push(["checked-removed", checkboxCol.attr("checked")]);

  return out;
}

assert.deepEqual(attrBasics($l), attrBasics($u));

function propBasics($) {
  const out = [];
  const input = dom.window.document.createElement("input");
  input.type = "text";
  const col = $.merge($(), [input]);

  out.push(["disabled-default", col.prop("disabled")]);
  col.prop("disabled", true);
  out.push(["disabled-set", col.prop("disabled"), input.disabled]);
  col.removeProp("disabled");
  out.push(["disabled-removed", input.disabled]);
  out.push(["tabIndex", col.prop("tabIndex")]);

  return out;
}

assert.deepEqual(propBasics($l), propBasics($u));

function classBasics($) {
  const out = [];
  const el = dom.window.document.createElement("div");
  const col = $.merge($(), [el]);

  col.addClass("a b");
  out.push(["addClass", el.className]);
  out.push(["hasClass", col.hasClass("a"), col.hasClass("c")]);
  col.toggleClass("c", true);
  out.push(["toggleClass-on", el.className]);
  col.toggleClass("c", false);
  out.push(["toggleClass-off", el.className]);
  col.removeClass("a");
  out.push(["removeClass", el.className]);

  return out;
}

assert.deepEqual(classBasics($l), classBasics($u));

function valBasics($) {
  const out = [];
  const input = dom.window.document.createElement("input");
  input.type = "text";
  const col = $.merge($(), [input]);

  col.val("hello");
  out.push(["text-val", col.val(), input.value]);

  const select = dom.window.document.createElement("select");
  const opt1 = dom.window.document.createElement("option");
  opt1.value = "one";
  const opt2 = dom.window.document.createElement("option");
  opt2.value = "two";
  select.appendChild(opt1);
  select.appendChild(opt2);
  const selCol = $.merge($(), [select]);
  selCol.val("two");
  out.push(["select-val", selCol.val(), select.selectedIndex]);

  return out;
}

assert.deepEqual(valBasics($l), valBasics($u));

function cssStyleBoundaryBasics($) {
  const out = [];
  const el = dom.window.document.createElement("div");
  el.style.width = "10px";

  out.push(["getter", $.style(el, "width")]);
  out.push(["null-result", $.style(el, "width", null), el.style.width]);
  out.push(["nan-result", $.style(el, "width", Number.NaN), el.style.width]);

  const previous = $.cssHooks.probe;
  const calls = [];
  $.cssHooks.probe = {
    get(elem, computed, extra) {
      calls.push(["get", elem === el, computed, extra]);
      return "hooked";
    },
    set(elem, value, extra) {
      calls.push(["set", elem === el, value, extra]);
      return undefined;
    },
  };
  out.push(["hook-get", $.style(el, "probe")]);
  out.push(["hook-set", $.style(el, "probe", "value"), el.style.probe || ""]);
  out.push(["hook-calls", calls]);
  if (previous === undefined) {
    delete $.cssHooks.probe;
  } else {
    $.cssHooks.probe = previous;
  }
  return out;
}

assert.deepEqual(cssStyleBoundaryBasics($l), cssStyleBoundaryBasics($u));

console.log("jquery-upstream:attributes:ok");

function eventOnOffBasics($) {
  const out = [];
  const el = dom.window.document.createElement("div");
  const col = $.merge($(), [el]);

  let plainCount = 0;
  function plainHandler() {
    plainCount += 1;
  }
  col.on("click", plainHandler);
  col.trigger("click");
  col.trigger("click");
  out.push(["plain", plainCount]);

  col.off("click", plainHandler);
  col.trigger("click");
  out.push(["off", plainCount]);

  let nsCount = 0;
  col.on("custom.foo", () => {
    nsCount += 1;
  });
  col.on("custom.bar", () => {
    nsCount += 100;
  });
  col.trigger("custom.foo");
  out.push(["namespaced", nsCount]);
  col.off(".foo");
  col.trigger("custom");
  out.push(["namespaced-off", nsCount]);

  let oneCount = 0;
  col.one("ping", () => {
    oneCount += 1;
  });
  col.trigger("ping");
  col.trigger("ping");
  out.push(["one", oneCount]);

  let dataSeen = null;
  col.on("withdata", (e, payload) => {
    dataSeen = [e.type, payload];
  });
  col.trigger("withdata", ["abc"]);
  out.push(["withdata", dataSeen]);

  return out;
}

assert.deepEqual(eventOnOffBasics($l), eventOnOffBasics($u));

function eventDelegationBasics($) {
  const out = [];
  const parent = dom.window.document.createElement("div");
  const child = dom.window.document.createElement("span");
  child.className = "target";
  parent.appendChild(child);
  const col = $.merge($(), [parent]);

  let delegatedCount = 0;
  let directCount = 0;
  col.on("click", ".target", function () {
    delegatedCount += 1;
    out.push(["this-is-child", this === child]);
  });
  col.on("click", function () {
    directCount += 1;
  });

  $.merge($(), [child]).trigger("click");
  out.push(["delegated", delegatedCount, directCount]);

  return out;
}

assert.deepEqual(eventDelegationBasics($l), eventDelegationBasics($u));

function eventTriggerHandlerBasics($) {
  const out = [];
  const el = dom.window.document.createElement("div");
  const col = $.merge($(), [el]);

  let seen = 0;
  col.on("scoped", () => {
    seen += 1;
    return false;
  });
  const result = col.triggerHandler("scoped");
  out.push(["triggerHandler-result", result, seen]);

  return out;
}

assert.deepEqual(eventTriggerHandlerBasics($l), eventTriggerHandlerBasics($u));

function eventPlainObjectBasics($) {
  const object = {};
  const col = $(object);
  const out = [];
  let seen = 0;
  function handler(event, value) {
    seen += value;
    out.push([event.type, event.target === object, seen]);
  }
  col.on("plain.ns", handler);
  col.trigger("plain", [3]);
  col.off("plain.ns", handler);
  col.trigger("plain", [5]);
  out.push(["final", seen]);
  return out;
}

assert.deepEqual(eventPlainObjectBasics($l), eventPlainObjectBasics($u));

function eventClickCheckboxBasics($) {
  const out = [];
  const checkbox = dom.window.document.createElement("input");
  checkbox.type = "checkbox";
  dom.window.document.body.appendChild(checkbox);
  const col = $.merge($(), [checkbox]);

  let clicked = 0;
  col.on("click", () => {
    clicked += 1;
  });
  col.trigger("click");
  out.push(["native-click", checkbox.checked, clicked]);
  checkbox.remove();

  return out;
}

assert.deepEqual(eventClickCheckboxBasics($l), eventClickCheckboxBasics($u));

console.log(
  selectedArtifact === null
    ? "jquery-upstream:events:ok"
    : `jquery-upstream:events:ok artifact=${selectedArtifact.sha256}`,
);
