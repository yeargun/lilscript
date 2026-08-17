import assert from "node:assert/strict";
import { verify as verifyEffects } from "../effects/verify.mjs";

export async function verify(lil, js) {
  await verifyEffects(lil, js);

  const $l = lil.jQuery;
  const $j = js.jQuery;
  assert.equal($l.fn.jquery, "3.7.1");
  assert.equal($j.fn.jquery, "3.7.1");
  assert.equal(typeof $l.fn.offset, "function");
  assert.equal(typeof $j.fn.offset, "function");
  assert.equal(typeof $l.fn.width, "function");
  assert.equal(typeof $j.fn.width, "function");
  assert.equal(typeof $l.fn.delay, "function");
  assert.equal(typeof $j.fn.delay, "function");
  if (lil.$ !== undefined) {
    assert.equal(lil.$, $l);
  }
}
