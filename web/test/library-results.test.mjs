import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const page = await readFile(new URL("../libraries.html", import.meta.url), "utf8");
const benchmarkPage = await readFile(new URL("../benchmarks.html", import.meta.url), "utf8");
const data = JSON.parse(await readFile(new URL("../src/library-results.json", import.meta.url), "utf8"));
const config = await readFile(new URL("../vite.config.js", import.meta.url), "utf8");

test("library page is a Vite entry backed by generated results", () => {
  assert.match(config, /libraries: resolve/);
  assert.match(page, /data-library-results/);
  assert.deepEqual(data.results.map((result) => result.id), [
    "motion-easing",
    "micro-math",
    "string-hash",
    "js-levenshtein",
    "emotion-hash",
    "murmurhash-js",
  ]);
});

test("every published LilScript library row passed native and C gates", () => {
  for (const result of data.results) {
    assert.deepEqual(result.artifacts.map((artifact) => artifact.id), ["vite", "closure", "lilscript"]);
    const lilscript = result.artifacts.at(-1);
    assert.equal(lilscript.nativeVerified, true);
    assert.equal(lilscript.cEmitted, true);
  }
});

test("the page does not turn partial Motion support into a full claim", () => {
  assert.match(benchmarkPage, /complete measured port/);
  assert.match(benchmarkPage, /does not imply compatibility with the current Motion DOM engine/);
  assert.ok(data.auditedButIneligible.some((item) => item.package === "motion"));
});
