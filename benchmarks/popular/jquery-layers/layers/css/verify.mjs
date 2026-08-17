import assert from "node:assert/strict";
import { verify as verifyManipulation } from "../manipulation/verify.mjs";

export async function verify(lil, js) {
  await verifyManipulation(lil, js);

  const $l = lil.jQuery;
  const $j = js.jQuery;
  assert.equal(typeof $l.fn.css, "function");
  assert.equal(typeof $j.fn.css, "function");
  assert.equal(typeof $l.fn.show, "function");
  assert.equal(typeof $j.fn.show, "function");
  assert.equal(typeof $l.fn.hide, "function");
  assert.equal(typeof $j.fn.hide, "function");

  const fixture = mountFixture(document);
  try {
    assert.deepEqual(cssBasics($l, fixture), cssBasics($j, fixture), "css");
  } finally {
    fixture.remove();
  }
}

function mountFixture(doc) {
  const root = doc.createElement("div");
  root.innerHTML = `<div id="box" style="width:40px;height:20px;display:block;color:red"></div>`;
  doc.body.appendChild(root);
  return root;
}

function cssBasics($, root) {
  const box = $("#box", root);
  const out = [];
  out.push(["color", String(box.css("color"))]);
  box.css("width", "50px");
  out.push(["widthSet", box.css("width")]);
  box.hide();
  out.push(["hidden", box.css("display")]);
  box.show();
  out.push(["shown", box.css("display")]);
  return out;
}
