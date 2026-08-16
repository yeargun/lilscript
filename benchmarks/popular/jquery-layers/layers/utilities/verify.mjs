import assert from "node:assert/strict";

export async function verify(lil, js) {
  const values = [
    null,
    undefined,
    true,
    1,
    "x",
    {},
    [],
    function named() {},
    () => 1,
    new Date(0),
    /a/,
    new Error("e"),
    Symbol.for("k"),
  ];

  for (const value of values) {
    assert.equal(lil.toType(value), js.toType(value), `toType(${String(value)})`);
    assert.equal(lil.isFunction(value), js.isFunction(value), `isFunction(${String(value)})`);
    assert.equal(lil.isWindow(value), js.isWindow(value), `isWindow(${String(value)})`);
  }

  const notFunction = { nodeType: 1 };
  assert.equal(lil.isFunction(notFunction), false);
  assert.equal(js.isFunction(notFunction), false);
  const collectionish = { item() {} };
  assert.equal(lil.isFunction(collectionish), false);
  assert.equal(js.isFunction(collectionish), false);

  const windowLike = {};
  windowLike.window = windowLike;
  assert.equal(lil.isWindow(windowLike), true);
  assert.equal(js.isWindow(windowLike), true);

  assert.equal(lil.camelCase("foo-bar"), js.camelCase("foo-bar"));
  assert.equal(lil.camelCase("-ms-transform"), js.camelCase("-ms-transform"));
  assert.equal(lil.camelCase("alreadyCamel"), js.camelCase("alreadyCamel"));

  assert.equal(lil.nodeName({ nodeName: "DIV" }, "div"), true);
  assert.equal(js.nodeName({ nodeName: "DIV" }, "div"), true);
  assert.equal(lil.nodeName({ nodeName: "SPAN" }, "div"), false);
  assert.equal(js.nodeName({ nodeName: "SPAN" }, "div"), false);

  assert.equal(lil.stripAndCollapse("  a   b  "), js.stripAndCollapse("  a   b  "));
  assert.equal(lil.stripAndCollapse("\tfoo\nbar "), js.stripAndCollapse("\tfoo\nbar "));
  assert.equal(lil.stripAndCollapse(""), js.stripAndCollapse(""));
}
