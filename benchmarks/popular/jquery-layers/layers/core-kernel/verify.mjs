import assert from "node:assert/strict";

function installTestInit(jq) {
  if (typeof jq.fn.init === "function") {
    return;
  }
  function init() {
    return this;
  }
  jq.fn.init = init;
  init.prototype = jq.fn;
}

function collection(jq, els) {
  const c = Object.create(jq.fn);
  c.length = 0;
  jq.merge(c, els);
  return c;
}

function sameDeep(lilValue, jsValue, label) {
  assert.deepEqual(lilValue, jsValue, label);
}

export async function verify(lil, js) {
  const $l = lil.jQuery;
  const $j = js.jQuery;

  assert.equal(typeof $l, "function");
  assert.equal(typeof $j, "function");
  assert.equal($l.fn.jquery, $j.fn.jquery);
  assert.equal($l.fn.jquery, "3.7.1");
  assert.equal($l.fn.length, 0);
  assert.equal($j.fn.length, 0);
  assert.equal($l.fn.constructor, $l);
  assert.equal($j.fn.constructor, $j);
  assert.equal($l.fn.extend, $l.extend);
  assert.equal($j.fn.extend, $j.extend);
  if (!("readyWait" in $l) && !("readyWait" in $j)) {
    assert.equal($l.isReady, true);
    assert.equal($j.isReady, true);
  } else {
    assert.equal(typeof $l.isReady, "boolean");
    assert.equal($l.isReady, $j.isReady);
  }
  assert.equal($l.guid, 1);
  assert.equal($j.guid, 1);
  assert.equal(typeof $l.noop, "function");
  assert.equal(typeof $j.noop, "function");
  $l.noop();
  $j.noop();
  assert.match($l.expando, /^jQuery371\d+$/);
  assert.match($j.expando, /^jQuery371\d+$/);
  assert.equal(typeof $l.support, "object");
  assert.equal(typeof $j.support, "object");

  assert.throws(() => $l.error("boom"), (err) => err instanceof Error && err.message === "boom");
  assert.throws(() => $j.error("boom"), (err) => err instanceof Error && err.message === "boom");

  sameDeep($l.extend({}, { a: 1, b: 2 }), $j.extend({}, { a: 1, b: 2 }), "extend shallow");
  sameDeep(
    $l.extend(true, { a: { b: 1, c: 1 } }, { a: { c: 2, d: 3 } }),
    $j.extend(true, { a: { b: 1, c: 1 } }, { a: { c: 2, d: 3 } }),
    "extend deep",
  );
  sameDeep(
    $l.extend(true, { a: [1, 2] }, { a: [3] }),
    $j.extend(true, { a: [1, 2] }, { a: [3] }),
    "extend deep array",
  );
  const protoGuardL = { a: 1 };
  const protoGuardJ = { a: 1 };
  $l.extend(protoGuardL, JSON.parse('{"__proto__":{"x":1},"b":2}'));
  $j.extend(protoGuardJ, JSON.parse('{"__proto__":{"x":1},"b":2}'));
  assert.equal(protoGuardL.b, 2);
  assert.equal(protoGuardJ.b, 2);
  assert.equal(Object.hasOwn(protoGuardL, "__proto__"), false);
  assert.equal(Object.hasOwn(protoGuardJ, "__proto__"), false);

  const seenL = [];
  const seenJ = [];
  $l.each([10, 20], function (i, v) {
    seenL.push([i, v, this]);
  });
  $j.each([10, 20], function (i, v) {
    seenJ.push([i, v, this]);
  });
  sameDeep(seenL, seenJ, "each array");

  const objSeenL = [];
  const objSeenJ = [];
  $l.each({ a: 1, b: 2 }, function (k, v) {
    objSeenL.push([k, v, this]);
  });
  $j.each({ a: 1, b: 2 }, function (k, v) {
    objSeenJ.push([k, v, this]);
  });
  sameDeep(objSeenL, objSeenJ, "each object");

  const stopL = [];
  const stopJ = [];
  $l.each([1, 2, 3], (i, v) => {
    stopL.push(v);
    return v !== 2;
  });
  $j.each([1, 2, 3], (i, v) => {
    stopJ.push(v);
    return v !== 2;
  });
  sameDeep(stopL, stopJ, "each break");

  sameDeep($l.merge([1], [2, 3]), $j.merge([1], [2, 3]), "merge");
  sameDeep(
    $l.grep([1, 2, 3, 4], (n) => n % 2 === 0),
    $j.grep([1, 2, 3, 4], (n) => n % 2 === 0),
    "grep",
  );
  sameDeep(
    $l.grep([1, 2, 3, 4], (n) => n % 2 === 0, true),
    $j.grep([1, 2, 3, 4], (n) => n % 2 === 0, true),
    "grep invert",
  );
  sameDeep(
    $l.map([1, 2, 3], (n) => n * 2),
    $j.map([1, 2, 3], (n) => n * 2),
    "map array",
  );
  sameDeep(
    $l.map([1, 2], (n) => [n, n + 10]),
    $j.map([1, 2], (n) => [n, n + 10]),
    "map flatten",
  );
  sameDeep(
    $l.map({ a: 1, b: 2 }, (v, k) => k + v),
    $j.map({ a: 1, b: 2 }, (v, k) => k + v),
    "map object",
  );
  sameDeep($l.makeArray("ab"), $j.makeArray("ab"), "makeArray string");
  sameDeep($l.makeArray([1, 2]), $j.makeArray([1, 2]), "makeArray array");
  sameDeep($l.makeArray(7), $j.makeArray(7), "makeArray scalar");
  sameDeep($l.makeArray([1], [0]), $j.makeArray([1], [0]), "makeArray results");
  assert.equal($l.inArray(2, [1, 2, 3]), $j.inArray(2, [1, 2, 3]));
  assert.equal($l.inArray(9, [1, 2, 3]), $j.inArray(9, [1, 2, 3]));
  assert.equal($l.inArray(1, null), $j.inArray(1, null));

  const plains = [{}, Object.create(null), new Date(0), [], null, Object.create({ a: 1 })];
  for (const value of plains) {
    assert.equal($l.isPlainObject(value), $j.isPlainObject(value), "isPlainObject");
  }
  assert.equal($l.isEmptyObject({}), $j.isEmptyObject({}));
  assert.equal($l.isEmptyObject({ a: 1 }), $j.isEmptyObject({ a: 1 }));

  const textNode = { nodeType: 3, nodeValue: "hi" };
  const elem = { nodeType: 1, textContent: "x" };
  const doc = { nodeType: 9, documentElement: { textContent: "d" } };
  assert.equal($l.text(textNode), $j.text(textNode));
  assert.equal($l.text(elem), $j.text(elem));
  assert.equal($l.text(doc), $j.text(doc));
  assert.equal($l.text([textNode, { nodeType: 3, nodeValue: "!" }]), $j.text([textNode, { nodeType: 3, nodeValue: "!" }]));

  const htmlish = {
    namespaceURI: "http://www.w3.org/1999/xhtml",
    ownerDocument: { documentElement: { nodeName: "HTML" } },
  };
  const xmlish = {
    namespaceURI: "http://example.com",
    ownerDocument: { documentElement: { nodeName: "root" } },
  };
  const namespaced = {
    namespaceURI: "http://example.com/foo",
    ownerDocument: { documentElement: { nodeName: "HTML" } },
  };
  assert.equal($l.isXMLDoc(htmlish), $j.isXMLDoc(htmlish));
  assert.equal($l.isXMLDoc(xmlish), $j.isXMLDoc(xmlish));
  assert.equal($l.isXMLDoc(namespaced), $j.isXMLDoc(namespaced));
  assert.equal($j.isXMLDoc(namespaced), true);

  installTestInit($l);
  installTestInit($j);

  const colL = collection($l, ["a", "b", "c", "d"]);
  const colJ = collection($j, ["a", "b", "c", "d"]);
  sameDeep(colL.toArray(), colJ.toArray(), "fn.toArray");
  sameDeep(colL.get(), colJ.get(), "fn.get all");
  assert.equal(colL.get(1), colJ.get(1));
  assert.equal(colL.get(-1), colJ.get(-1));

  const eachL = [];
  const eachJ = [];
  colL.each(function (i, v) {
    eachL.push([i, v, this]);
  });
  colJ.each(function (i, v) {
    eachJ.push([i, v, this]);
  });
  sameDeep(eachL, eachJ, "fn.each");

  sameDeep(colL.eq(1).toArray(), colJ.eq(1).toArray(), "fn.eq");
  sameDeep(colL.eq(-1).toArray(), colJ.eq(-1).toArray(), "fn.eq neg");
  sameDeep(colL.first().toArray(), colJ.first().toArray(), "fn.first");
  sameDeep(colL.last().toArray(), colJ.last().toArray(), "fn.last");
  sameDeep(colL.even().toArray(), colJ.even().toArray(), "fn.even");
  sameDeep(colL.odd().toArray(), colJ.odd().toArray(), "fn.odd");
  sameDeep(colL.slice(1, 3).toArray(), colJ.slice(1, 3).toArray(), "fn.slice");
  sameDeep(
    colL.map(function (i, v) {
      return v + i;
    }).toArray(),
    colJ.map(function (i, v) {
      return v + i;
    }).toArray(),
    "fn.map",
  );

  const stackedL = colL.eq(2);
  const stackedJ = colJ.eq(2);
  sameDeep(stackedL.end().toArray(), stackedJ.end().toArray(), "fn.end");
  sameDeep(stackedL.pushStack(["z"]).toArray(), stackedJ.pushStack(["z"]).toArray(), "fn.pushStack");
  assert.equal(stackedL.pushStack(["z"]).prevObject, stackedL);
  assert.equal(stackedJ.pushStack(["z"]).prevObject, stackedJ);
}
