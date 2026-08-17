import assert from "node:assert/strict";
import { verify as verifyDomCore } from "../dom-core/verify.mjs";

export async function verify(lil, js) {
  await verifyDomCore(lil, js);

  const $l = lil.jQuery;
  const $j = js.jQuery;
  assert.equal(typeof $l.fn.attr, "function");
  assert.equal(typeof $j.fn.attr, "function");
  assert.equal(typeof $l.fn.prop, "function");
  assert.equal(typeof $j.fn.prop, "function");
  assert.equal(typeof $l.fn.val, "function");
  assert.equal(typeof $j.fn.val, "function");
  assert.equal(typeof $l.fn.addClass, "function");
  assert.equal(typeof $j.fn.addClass, "function");

  assert.deepEqual(withFixture($l), withFixture($j), "attributes");
}

function withFixture($) {
  const fixture = mountFixture(document);
  try {
    return attrBasics($, fixture);
  } finally {
    fixture.remove();
  }
}

function mountFixture(doc) {
  const root = doc.createElement("div");
  root.innerHTML = `
    <a id="link" href="/x" class="nav">Go</a>
    <input id="q" type="text" value="hi" />
    <input id="ok" type="checkbox" checked />
  `;
  doc.body.appendChild(root);
  return root;
}

function attrBasics($, root) {
  const link = $("#link", root);
  const input = $("#q", root);
  const box = $("#ok", root);
  const out = [];
  out.push(["href", link.attr("href")]);
  link.attr("title", "t");
  out.push(["setTitle", link.attr("title")]);
  out.push(["propHref", link.prop("href").toString().endsWith("/x")]);
  out.push(["val", input.val()]);
  input.val("bye");
  out.push(["setVal", input.val()]);
  out.push(["checked", box.prop("checked")]);
  box.prop("checked", false);
  out.push(["unchecked", box.prop("checked")]);
  link.addClass("on");
  out.push(["class", link.attr("class")]);
  link.removeClass("nav");
  out.push(["class2", link.attr("class")]);
  return out;
}
