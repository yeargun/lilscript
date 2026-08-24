import assert from "node:assert/strict";
import test from "node:test";

import { minifyJqueryBundle } from "../jquery-measurement-lanes.mjs";

const source = `
export function addOne(needlesslyLongArgumentName) {
  const unusedValue = 42;
  return {
    publicValue: needlesslyLongArgumentName + 1,
    publicMethod() { return this.publicValue; },
  };
}
`;

test("all diagnostic jQuery minifiers consume the same linked ESM source", async () => {
  const outputs = await minifyJqueryBundle(source, "jquery-fixture.bundle.js");

  assert.deepEqual(Object.keys(outputs), ["esbuild", "terser", "oxc"]);
  for (const [lane, code] of Object.entries(outputs)) {
    assert.ok(code.length < source.length, `${lane} should minify the fixture`);
    const module = await import(
      `data:text/javascript;charset=utf-8,${encodeURIComponent(code)}`
    );
    assert.deepEqual(Object.keys(module), ["addOne"], `${lane} export surface`);
    const value = module.addOne(4);
    assert.deepEqual(
      Object.keys(value).sort(),
      ["publicMethod", "publicValue"],
      `${lane} public object keys`,
    );
    assert.equal(value.publicMethod(), 5, `${lane} public method contract`);
  }
});

test("minifier lane input identity is explicit", async () => {
  await assert.rejects(
    minifyJqueryBundle("", "jquery-fixture.bundle.js"),
    /non-empty string/u,
  );
  await assert.rejects(minifyJqueryBundle(source, ""), /filename/u);
});
