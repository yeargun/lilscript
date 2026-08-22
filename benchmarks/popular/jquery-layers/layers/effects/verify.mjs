import assert from "node:assert/strict";
import { verify as verifyAjax } from "../ajax/verify.mjs";

export async function verify(lil, js) {
  await verifyAjax(lil, js);

  const $l = lil.jQuery;
  const $j = js.jQuery;
  assert.equal(typeof $l.fn.animate, "function");
  assert.equal(typeof $j.fn.animate, "function");
  assert.equal(typeof $l.fn.fadeIn, "function");
  assert.equal(typeof $j.fn.fadeIn, "function");
  assert.equal(typeof $l.expr.pseudos.animated, "function");
  assert.equal(typeof $j.expr.pseudos.animated, "function");
  assert.equal(typeof $l.fx, "function");
  assert.equal(typeof $j.fx, "function");
  assert.equal(typeof $l.fx.speeds, "object");
  assert.equal(typeof $j.fx.speeds, "object");
}
