import assert from "node:assert/strict";
import { verify as verifyCss } from "../css/verify.mjs";

export async function verify(lil, js) {
  await verifyCss(lil, js);

  const $l = lil.jQuery;
  const $j = js.jQuery;
  assert.equal(typeof $l.ajax, "function");
  assert.equal(typeof $j.ajax, "function");
  assert.equal(typeof $l.param, "function");
  assert.equal(typeof $j.param, "function");
  assert.equal(typeof $l.parseXML, "function");
  assert.equal(typeof $j.parseXML, "function");

  assert.deepEqual(ajaxBasics($l), ajaxBasics($j), "ajax");
}

function ajaxBasics($) {
  return [
    ["param", $.param({ a: 1, b: ["x", "y"] })],
    ["paramTrad", $.param({ a: [1, 2] }, true)],
    ["parseXML", $.parseXML("<root><n>1</n></root>").documentElement.nodeName],
  ];
}
