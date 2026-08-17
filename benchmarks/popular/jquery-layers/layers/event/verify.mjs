import assert from "node:assert/strict";
import { verify as verifyAttributes } from "../attributes/verify.mjs";

export async function verify(lil, js) {
  await verifyAttributes(lil, js);

  const $l = lil.jQuery;
  const $j = js.jQuery;
  assert.equal(typeof $l.fn.on, "function");
  assert.equal(typeof $j.fn.on, "function");
  assert.equal(typeof $l.fn.trigger, "function");
  assert.equal(typeof $j.fn.trigger, "function");

  const fixture = mountFixture(document);
  try {
    assert.deepEqual(eventBasics($l, fixture), eventBasics($j, fixture), "event");
  } finally {
    fixture.remove();
  }
}

function mountFixture(doc) {
  const root = doc.createElement("div");
  root.innerHTML = `<button id="go" type="button">Go</button>`;
  doc.body.appendChild(root);
  return root;
}

function eventBasics($, root) {
  const btn = $("#go", root);
  const seen = [];
  btn.on("click.layer", function (event) {
    seen.push(["click", this.id, event.type]);
  });
  btn.trigger("click");
  btn.off("click.layer");
  btn.trigger("click");
  return seen;
}
