import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import test from "node:test";

const report = JSON.parse(
  await readFile(new URL("../build/results.json", import.meta.url), "utf8"),
);
const motionCompatibility = JSON.parse(
  await readFile(new URL("../compatibility/motion-v13.json", import.meta.url), "utf8"),
);
const comparableIds = ["reference", "esbuild", "closure", "hand", "lilscript"];

test("compiler totals have one fixed comparable scope", () => {
  for (const result of report.results) {
    assert.deepEqual(
      result.artifacts.slice(0, comparableIds.length).map((artifact) => artifact.id),
      comparableIds,
    );
    assert.equal(
      result.artifacts.some((artifact) => artifact.id === "ecosystem"),
      false,
    );
  }
});

test("LilScript never exceeds Closure size on a comparable workload", () => {
  for (const result of report.results) {
    const closure = result.artifacts.find((artifact) => artifact.id === "closure");
    const lilscript = result.artifacts.find((artifact) => artifact.id === "lilscript");
    for (const metric of ["raw", "gzip", "brotli"]) {
      assert.ok(
        lilscript[metric] <= closure[metric],
        `${result.name} ${metric}: LilScript ${lilscript[metric]} > Closure ${closure[metric]}`,
      );
    }
  }
});

test("LilScript full-sample runtime stays within five percent of Closure", () => {
  if (report.metadata.samples < 20) return;
  for (const result of report.results) {
    const closure = result.artifacts.find((artifact) => artifact.id === "closure");
    const lilscript = result.artifacts.find((artifact) => artifact.id === "lilscript");
    assert.ok(
      lilscript.medianMs / closure.medianMs <= 1.05,
      `${result.name} runtime ratio ${(lilscript.medianMs / closure.medianMs).toFixed(3)} > 1.05`,
    );
  }
});

test("Closure receives the exact readable JavaScript reference", async () => {
  for (const result of report.results) {
    const root = new URL(`../build/${result.name}/`, import.meta.url);
    const reference = await readFile(new URL("js-reference.js", root), "utf8");
    const closureInput = await readFile(new URL("closure-input.js", root), "utf8");
    assert.equal(closureInput, reference, result.name);
  }
});

test("real package builds stay in context-only Vite records", () => {
  const contexts = report.results.filter((result) => result.ecosystem);
  assert.deepEqual(
    contexts.map((result) => result.name),
    ["reactive-store", "event-pipeline", "motion-values"],
  );
  for (const result of contexts) {
    assert.match(result.ecosystem.label, /via Vite$/);
    assert.ok(result.ecosystem.files.some((path) => path.endsWith(".js")));
  }
});

test("specialized LilScript is a diagnostic, not a corpus-total lane", () => {
  for (const result of report.results) {
    const specialized = result.artifacts.find(
      (artifact) => artifact.id === "lilscript-specialized",
    );
    assert.equal(Boolean(specialized), result.name === "motion-values");
  }
  assert.equal(comparableIds.includes("lilscript-specialized"), false);
});

test("incomplete Motion compatibility cannot enter comparison totals", () => {
  assert.equal(motionCompatibility.status, "not-implemented");
  assert.equal(motionCompatibility.benchmarkEligibility, "context-only");
  assert.equal(motionCompatibility.implementedRootRuntimeExports, 0);
  assert.equal(motionCompatibility.testStatus.upstreamUnit, "not-run");
  assert.equal(motionCompatibility.testStatus.upstreamBrowser, "not-run");

  const motion = report.results.find((result) => result.name === "motion-values");
  assert.ok(motion.ecosystem);
  assert.equal(motion.artifacts.some((artifact) => artifact.id === "motion"), false);
});

test("Motion compatibility inventory matches the pinned package", async () => {
  const motionPackage = JSON.parse(
    await readFile(new URL("../node_modules/motion/package.json", import.meta.url), "utf8"),
  );
  const motion = await import("motion");
  assert.equal(motionPackage.version, motionCompatibility.version);
  assert.equal(Object.keys(motion).length, motionCompatibility.rootRuntimeExports);

  let publishedBindings = 0;
  const uniqueNames = new Set();
  for (const entrypoint of motionCompatibility.publishedEntrypoints) {
    const runtime = await import(entrypoint.specifier);
    const names = Object.keys(runtime).sort();
    const sha256 = createHash("sha256").update(names.join("\n")).digest("hex");
    assert.deepEqual(names, entrypoint.runtimeExportNames, entrypoint.specifier);
    assert.equal(sha256, entrypoint.exportNameSha256, entrypoint.specifier);
    assert.equal(entrypoint.implementedRuntimeExports, 0, entrypoint.specifier);
    publishedBindings += names.length;
    for (const name of names) uniqueNames.add(name);
  }
  assert.equal(publishedBindings, motionCompatibility.publishedRuntimeBindings);
  assert.equal(uniqueNames.size, motionCompatibility.uniqueRuntimeExportNames);
});
