import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
import test from "node:test";
const read = name => readFile(new URL(`../${name}`, import.meta.url), "utf8");
test("the demo directory links every verified library and its current examples", async () => {
  const page = await read("demos.html");
  const data = JSON.parse(await read("src/library-releases.json"));
  for (const library of data.libraries) assert.ok(page.includes(library.homepage), library.name);
  assert.equal((page.match(/class="lib-card"/g) ?? []).length, 26);
  assert.match(page, /marketplace.html/);
  assert.match(page, /Application experiments/);
  assert.doesNotMatch(page, /previous compiler|previous version|demos-catalog|src\/demos.js/);
});
