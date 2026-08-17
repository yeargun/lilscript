import assert from "node:assert/strict";

function snapshot(api, tree) {
  const lineCount = api.getLineCount(tree);
  const lines = [];
  for (let i = 1; i <= lineCount; i++) {
    lines.push(api.getLineContent(tree, i));
  }
  return {
    value: api.getValue(tree),
    length: api.getLength(tree),
    lineCount,
    lines,
    offsets: Array.from({ length: lineCount }, (_, i) => api.getOffsetAt(tree, i + 1, 1)),
  };
}

export async function verify(lil, js) {
  const seed = 42;
  let s = seed;
  const rand = () => {
    s = (s * 1664525 + 1013904223) >>> 0;
    return s / 0x100000000;
  };

  const start = "function hello() {\n  return 1;\n}\n";
  const lilTree = lil.create(start, "\n");
  const jsTree = js.create(start, "\n");
  assert.deepEqual(snapshot(lil, lilTree), snapshot(js, jsTree), "initial");

  for (let i = 0; i < 400; i++) {
    const len = js.getLength(jsTree);
    if (rand() < 0.55 || len === 0) {
      const offset = Math.floor(rand() * (len + 1));
      const chunk = rand() < 0.3 ? "\n" : `x${i}\nmore`;
      lil.insert(lilTree, offset, chunk);
      js.insert(jsTree, offset, chunk);
    } else {
      const offset = Math.floor(rand() * len);
      const cnt = Math.min(len - offset, 1 + Math.floor(rand() * 8));
      lil.deleteRange(lilTree, offset, cnt);
      js.deleteRange(jsTree, offset, cnt);
    }
    if (i % 20 === 19) {
      assert.deepEqual(snapshot(lil, lilTree), snapshot(js, jsTree), `op ${i}`);
    }
  }
  assert.deepEqual(snapshot(lil, lilTree), snapshot(js, jsTree), "final");
}
