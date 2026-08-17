import assert from "node:assert/strict";
import { verify as verifySelector } from "../selector/verify.mjs";

export async function verify(lil, js) {
  await verifySelector(lil, js);

  const $l = lil.jQuery;
  const $j = js.jQuery;
  assert.equal(typeof $l.fn.init, "function");
  assert.equal(typeof $j.fn.init, "function");
  assert.equal(typeof $l.fn.find, "function");
  assert.equal(typeof $j.fn.find, "function");
  assert.equal(typeof $l.fn.ready, "function");
  assert.equal(typeof $j.fn.ready, "function");

  const fixture = mountFixture(document);
  try {
    assert.deepEqual(initBasics($l, fixture), initBasics($j, fixture), "init");
    assert.deepEqual(traverseBasics($l, fixture), traverseBasics($j, fixture), "traversing");
  } finally {
    fixture.remove();
  }
}

function mountFixture(doc) {
  const root = doc.createElement("div");
  root.id = "dom-fixture";
  root.innerHTML = `
    <h1 id="title" class="hdr">Hello</h1>
    <ul class="list">
      <li class="item first" data-n="1">one</li>
      <li class="item" data-n="2">two</li>
      <li class="item last" data-n="3">three</li>
    </ul>
    <p class="note"><span>inner</span></p>
  `;
  doc.body.appendChild(root);
  return root;
}

function labels(nodes) {
  return Array.from(nodes).map((node) => {
    if (!node || !node.nodeType) {
      return String(node);
    }
    const id = node.id ? `#${node.id}` : "";
    const className =
      typeof node.className === "string" && node.className.trim()
        ? `.${node.className.trim().split(/\s+/).join(".")}`
        : "";
    return `${node.nodeName}${id}${className}`;
  });
}

function initBasics($, root) {
  const title = root.querySelector("#title");
  return [
    ["empty", $(undefined).length, $("").length],
    ["id", labels($("#title", root))],
    ["class", labels($(".item", root))],
    ["element", labels($(title))],
    ["array", labels($([title, root.querySelector("p.note")]))],
  ];
}

function traverseBasics($, root) {
  const items = $(".item", root);
  return [
    ["find", labels($(".list", root).find("li"))],
    ["filter", labels(items.filter(".first"))],
    ["not", labels(items.not(".first"))],
    ["is", items.is(".item"), items.is("p")],
    ["eq", labels(items.eq(1))],
    ["first", labels(items.first())],
    ["last", labels(items.last())],
    ["parent", labels(items.first().parent())],
    ["children", labels($(".list", root).children())],
    ["closest", labels(items.first().closest("ul"))],
  ];
}
