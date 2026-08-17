import assert from "node:assert/strict";
import { verify as verifyEvent } from "../event/verify.mjs";

export async function verify(lil, js) {
  await verifyEvent(lil, js);

  const $l = lil.jQuery;
  const $j = js.jQuery;
  assert.equal(typeof $l.fn.append, "function");
  assert.equal(typeof $j.fn.append, "function");
  assert.equal(typeof $l.parseHTML, "function");
  assert.equal(typeof $j.parseHTML, "function");

  const fixture = mountFixture(document);
  try {
    assert.deepEqual(manipBasics($l, fixture), manipBasics($j, fixture), "manipulation");
  } finally {
    fixture.remove();
  }
}

function mountFixture(doc) {
  const root = doc.createElement("div");
  root.id = "manip-root";
  root.innerHTML = `<ul class="list"><li class="keep">keep</li></ul>`;
  doc.body.appendChild(root);
  return root;
}

function manipBasics($, root) {
  const list = $(".list", root);
  list.append("<li class='added'>x</li>");
  const out = [["html", list.html().replace(/\s+/g, " ").trim()]];
  list.find(".added").remove();
  out.push(["afterRemove", list.children().length]);
  const parsed = $.parseHTML("<p class='p'>hi</p>");
  out.push(["parse", parsed[0].nodeName, parsed[0].className, parsed[0].textContent]);
  return out;
}
