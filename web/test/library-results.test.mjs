import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const page = await readFile(new URL("../libraries.html", import.meta.url), "utf8");
const benchmarkPage = await readFile(new URL("../benchmarks.html", import.meta.url), "utf8");
const data = JSON.parse(await readFile(new URL("../src/library-results.json", import.meta.url), "utf8"));
const clientRuntime = JSON.parse(
  await readFile(new URL("../src/client-runtime-results.json", import.meta.url), "utf8"),
);
const popular = JSON.parse(
  await readFile(new URL("../src/popular-library-results.json", import.meta.url), "utf8"),
);
const config = await readFile(new URL("../vite.config.js", import.meta.url), "utf8");

test("library page is a Vite entry backed by generated results", () => {
  assert.match(config, /libraries: resolve/);
  assert.match(page, /data-library-results/);
  assert.match(page, /data-client-runtime/);
  assert.match(page, /data-popular-eligible/);
  assert.match(page, /data-popular-blocked/);
  assert.doesNotMatch(page, /data-popular-research/);
  assert.deepEqual(data.results.map((result) => result.id), [
    "motion-easing",
    "micro-math",
    "string-hash",
    "js-levenshtein",
    "emotion-hash",
    "murmurhash-js",
    "robust-predicates",
  ]);
});

test("popular matrix publishes only exact entrypoints that pass every gate", () => {
  assert.deepEqual(popular.results.map((result) => result.id), [
    "nanoid",
    "mitt",
    "clsx",
    "immer",
    "redux-toolkit",
    "zod",
    "acorn",
    "preact",
    "solid-js",
    "gl-matrix",
  ]);
  assert.deepEqual(
    popular.results.filter((result) => result.eligible).map((result) => result.id),
    ["nanoid", "mitt", "clsx", "gl-matrix"],
  );
  assert.deepEqual(
    popular.results.filter((result) => result.exactSurface).map((result) => result.id),
    ["nanoid", "mitt", "clsx", "gl-matrix"],
  );
  assert.equal(popular.results.find((result) => result.id === "nanoid").sizeGate, true);
  assert.equal(popular.results.find((result) => result.id === "mitt").sizeGate, true);
  assert.equal(popular.results.find((result) => result.id === "clsx").sizeGate, true);
  assert.equal(popular.results.find((result) => result.id === "gl-matrix").sizeGate, true);
  assert.equal(popular.results.find((result) => result.id === "clsx").performanceGate, true);
  assert.equal(popular.results.find((result) => result.id === "redux-toolkit").closureLevel, "SIMPLE");
  assert.equal(popular.results.find((result) => result.id === "zod").closureLevel, "SIMPLE");
  assert.equal(popular.results.find((result) => result.id === "preact").closureLevel, "SIMPLE");
  const mitt = popular.results.find((result) => result.id === "mitt");
  assert.equal(mitt.lilscriptVite.raw, 595);
  assert.equal(mitt.vite.raw, 595);
  assert.equal(mitt.lilscriptVite.brotli, 300);
  assert.equal(mitt.vite.brotli, 300);
  assert.ok(mitt.performance.performance.ratio <= 1.05);
  assert.ok(mitt.performance.retainedMemory.ratio <= 1.05);
  assert.ok(
    popular.results
      .filter((result) => result.status.includes("subset") || result.status === "partial-external")
      .every((result) => result.eligible === false && result.exactSurface === false),
  );
});

test("partial Solid evidence reports its compatibility denominator", () => {
  assert.equal(clientRuntime.status, "partial");
  assert.equal(clientRuntime.port.adaptedCasesPassed, 109);
  assert.equal(clientRuntime.port.adaptedCasesTotal, 469);
  assert.equal(clientRuntime.port.executions, 654);
  assert.equal(clientRuntime.upstream.testsPassed, 469);
  assert.ok(clientRuntime.notPorted.length > 0);
  const solid = clientRuntime.sizes.find((artifact) => artifact.id === "solid-todolist");
  const solidlil = clientRuntime.sizes.find((artifact) => artifact.id === "solidlil-lsx");
  assert.ok(solidlil.brotli < solid.brotli);
  assert.ok(clientRuntime.runtime.lsxTimeRatio <= 1.05);
  assert.ok(clientRuntime.runtime.lsxMemoryRatio <= 1.05);
});

test("every published LilScript library row passed native and C gates", () => {
  for (const result of data.results) {
    assert.deepEqual(result.artifacts.map((artifact) => artifact.id), ["vite", "closure", "lilscript"]);
    assert.deepEqual(
      result.surfaceArtifacts.map((artifact) => artifact.id),
      ["vite", "closure", "lilscript"],
    );
    const lilscript = result.artifacts.at(-1);
    assert.equal(lilscript.nativeVerified, true);
    assert.equal(lilscript.cEmitted, true);
    const viteSurface = result.surfaceArtifacts[0];
    const closureSurface = result.surfaceArtifacts[1];
    const lilscriptSurface = result.surfaceArtifacts[2];
    assert.ok(lilscriptSurface.raw <= viteSurface.raw);
    assert.ok(lilscriptSurface.raw <= closureSurface.raw);
    assert.ok(
      lilscriptSurface[data.metadata.selectedCodec] <= viteSurface[data.metadata.selectedCodec],
    );
    assert.ok(
      lilscriptSurface[data.metadata.selectedCodec] <= closureSurface[data.metadata.selectedCodec],
    );
  }
});

test("the page does not turn partial Motion support into a full claim", () => {
  assert.match(benchmarkPage, /complete measured port/);
  assert.match(benchmarkPage, /does not imply compatibility with the current Motion DOM engine/);
  assert.ok(data.auditedButIneligible.some((item) => item.package === "motion"));
});
