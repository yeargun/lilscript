import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const page = await readFile(new URL("../benchmarks.html", import.meta.url), "utf8");
const data = JSON.parse(
  await readFile(new URL("../src/benchmark-results.json", import.meta.url), "utf8"),
);
const packageJson = JSON.parse(
  await readFile(new URL("../package.json", import.meta.url), "utf8"),
);

test("benchmark page binds every generated compiler result", () => {
  assert.equal(data.results.length, 5);
  for (const result of data.results) {
    assert.match(page, new RegExp(`data-compiler-table="${result.name}"`));
    if (result.ecosystem) {
      assert.match(page, new RegExp(`data-ecosystem="${result.name}"`));
    }
  }
});

test("published data uses the installed Vite and omits timing samples", () => {
  assert.equal(data.metadata.vite, packageJson.devDependencies.vite);
  for (const result of data.results) {
    for (const artifact of result.artifacts) assert.equal("samplesMs" in artifact, false);
    if (result.ecosystem) assert.equal("samplesMs" in result.ecosystem, false);
  }
});

test("static copy makes no Motion implementation claim", () => {
  assert.match(page, /Motion 13 is not implemented yet/);
  assert.doesNotMatch(page, /Motion value pipeline/);
  assert.doesNotMatch(page, /LilScript is \d+ bytes smaller/);
});
