import assert from "node:assert/strict";
import { verify as verifyDataQueue } from "../data-queue/verify.mjs";

export async function verify(lil, js) {
  await verifyDataQueue(lil, js);

  const $l = lil.jQuery;
  const $j = js.jQuery;
  assert.equal(typeof $l.find, "function");
  assert.equal(typeof $j.find, "function");
  assert.equal(typeof $l.uniqueSort, "function");
  assert.equal(typeof $j.uniqueSort, "function");
  assert.equal(typeof $l.contains, "function");
  assert.equal(typeof $j.contains, "function");
  assert.equal(typeof $l.escapeSelector, "function");
  assert.equal(typeof $j.escapeSelector, "function");
  assert.equal(typeof $l.expr, "object");
  assert.equal(typeof $j.expr, "object");
  assert.equal($l.expr[":"], $l.expr.pseudos);
  assert.equal($j.expr[":"], $j.expr.pseudos);
  assert.equal(typeof $l.expr.match.needsContext.test, "function");
  assert.equal(typeof $j.expr.match.needsContext.test, "function");

  const fixture = mountFixture(document);
  try {
    assert.deepEqual(selectorBasics($l, fixture), selectorBasics($j, fixture), "find");
    assert.deepEqual(containsBasics($l, fixture), containsBasics($j, fixture), "contains");
    assert.deepEqual(escapeBasics($l), escapeBasics($j), "escapeSelector");
    assert.deepEqual(uniqueBasics($l, fixture), uniqueBasics($j, fixture), "uniqueSort");
  } finally {
    fixture.remove();
  }
}

function mountFixture(doc) {
  const root = doc.createElement("div");
  root.id = "sel-fixture";
  root.innerHTML = `
    <h1 id="title" class="hdr">Hello</h1>
    <ul class="list">
      <li class="item first" data-n="1">one</li>
      <li class="item" data-n="2">two</li>
      <li class="item last" data-n="3">three</li>
    </ul>
    <form>
      <input type="text" name="q" value="x" />
      <input type="checkbox" name="ok" checked />
      <button type="submit">Go</button>
    </form>
    <p class="note"><span>inner</span></p>
    <div class="empty"></div>
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

function selectorBasics($, root) {
  const out = [];
  const list = root.querySelector("ul.list");
  const selectors = [
    "div",
    "#title",
    ".item",
    "ul li",
    "ul > li",
    "li.item",
    "[data-n]",
    "[data-n=2]",
    "h1, p",
    ":header",
    "li:first",
    "li:last",
    "li:even",
    "li:eq(1)",
    "li:gt(0)",
    ":input",
    ":checkbox",
    ":checked",
    ":button",
    ":empty",
    "p:has(span)",
    "li:not(.first)",
    "p:contains(inner)",
    "form input",
    ".list .item.last",
  ];
  for (const selector of selectors) {
    out.push([selector, labels($.find(selector, root))]);
  }
  out.push(["ctx-li", labels($.find("li", list))]);
  const firstItem = root.querySelector("li.item");
  out.push([
    "matchesSelector",
    $.find.matchesSelector(firstItem, "li.item"),
    $.find.matchesSelector(firstItem, "p"),
  ]);
  return out;
}

function containsBasics($, root) {
  const title = root.querySelector("#title");
  const item = root.querySelector("li.item");
  return [
    ["self", $.contains(root, root)],
    ["child", $.contains(root, title)],
    ["nested", $.contains(root, item)],
    ["reverse", $.contains(title, root)],
  ];
}

function escapeBasics($) {
  return [
    $.escapeSelector("foo.bar"),
    $.escapeSelector("1id"),
    $.escapeSelector("-x"),
  ];
}

function uniqueBasics($, root) {
  const items = Array.from(root.querySelectorAll("li"));
  const mixed = [items[2], items[0], items[1], items[0]];
  return labels($.uniqueSort(mixed));
}
