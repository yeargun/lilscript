import assert from "node:assert/strict";
import { verify as verifyDeferred } from "../deferred/verify.mjs";

export async function verify(lil, js) {
  await verifyDeferred(lil, js);

  const $l = lil.jQuery;
  const $j = js.jQuery;
  assert.equal(typeof $l.data, "function");
  assert.equal(typeof $j.data, "function");
  assert.equal(typeof $l.queue, "function");
  assert.equal(typeof $j.queue, "function");
  assert.equal(typeof $l.fn.data, "function");
  assert.equal(typeof $j.fn.data, "function");
  assert.equal(typeof $l.fn.queue, "function");
  assert.equal(typeof $j.fn.queue, "function");

  assert.deepEqual(dataBasics($l), dataBasics($j), "data");
  assert.deepEqual(queueBasics($l), queueBasics($j), "queue");
}

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

  const el = document.createElement("div");
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

function queueBasics($) {
  const out = [];
  const el = document.createElement("div");
  const col = $.merge($(), [el]);

  out.push(["empty", $.queue(el)]);
  col.queue(function (next) {
    out.push(["step1", this === el]);
    next();
  });
  out.push(["queued", $.queue(el).length]);
  col.dequeue();
  out.push(["after1", $.queue(el).length, $.hasData(el)]);

  const el2 = document.createElement("span");
  const col2 = $.merge($(), [el2]);
  let ran = 0;
  col2
    .queue("fx", function (next) {
      ran += 1;
      next();
    })
    .queue(function (next) {
      ran += 10;
      next();
    });
  out.push(["autoDequeue", ran, $.queue(el2).length]);

  const el3 = document.createElement("p");
  $.queue(el3, "custom", function (next) {
    out.push("custom");
    next();
  });
  out.push(["customLen", $.queue(el3, "custom").length]);
  $.dequeue(el3, "custom");
  out.push(["customDone", $.queue(el3, "custom").length]);

  return out;
}
