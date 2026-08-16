import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const page = await readFile(
  new URL("../libraries.html", import.meta.url),
  "utf8",
);
const benchmarkPage = await readFile(
  new URL("../benchmarks.html", import.meta.url),
  "utf8",
);
const data = JSON.parse(
  await readFile(
    new URL("../src/library-results.json", import.meta.url),
    "utf8",
  ),
);
const clientRuntime = JSON.parse(
  await readFile(
    new URL("../src/client-runtime-results.json", import.meta.url),
    "utf8",
  ),
);
const popular = JSON.parse(
  await readFile(
    new URL("../src/popular-library-results.json", import.meta.url),
    "utf8",
  ),
);
const config = await readFile(
  new URL("../vite.config.js", import.meta.url),
  "utf8",
);
const libraryScript = await readFile(
  new URL("../src/libraries.js", import.meta.url),
  "utf8",
);

test("library page is a Vite entry backed by generated results", () => {
  assert.match(config, /libraries: resolve/);
  assert.match(page, /data-library-results/);
  assert.match(page, /data-client-runtime/);
  assert.match(page, /data-popular-eligible/);
  assert.match(page, /data-popular-blocked/);
  assert.match(page, /data-popular-candidates/);
  assert.doesNotMatch(page, /data-popular-research/);
  assert.deepEqual(
    data.results.map((result) => result.id),
    [
      "micro-math",
      "js-levenshtein",
      "emotion-hash",
      "murmurhash-js",
      "robust-predicates",
    ],
  );
  const motion = data.diagnostics.find(
    (result) => result.id === "motion-easing",
  );
  assert.equal(motion.eligible, false);
  assert.ok(
    motion.workload.performance.ratio > data.metadata.materialRegressionLimit,
  );
  assert.equal(motion.blockers.length, 1);
  assert.match(motion.blockers[0], /^throughput ratio .* exceeds 1\.05$/);
  const stringHash = data.diagnostics.find(
    (result) => result.id === "string-hash",
  );
  assert.equal(stringHash.eligible, false);
  assert.ok(
    stringHash.workload.performance.ratio > data.metadata.materialRegressionLimit,
  );
  assert.match(stringHash.blockers[0], /^throughput ratio .* exceeds 1\.05$/);
});

test("library comparisons are Brotli-first, filterable, and selectable", () => {
  assert.match(page, /data-library-comparator/);
  assert.match(page, /option value="brotli">Brotli-11 bytes · primary/);
  assert.match(page, /option value="disposed-memory">Post-unmount heap/);
  assert.match(page, /data-compare-scope/);
  assert.match(page, /data-compare-search/);
  assert.match(page, /data-compare-picker/);
  assert.match(page, /data-select-winners/);
  assert.match(libraryScript, /new URLSearchParams\(location\.search\)/);
  assert.match(libraryScript, /history\.replaceState/);
  assert.match(libraryScript, /selected = new Set/);
  assert.match(libraryScript, /result\.exactSurface/);
  assert.match(libraryScript, /result\.eligible/);
  assert.match(libraryScript, /disposedMemory/);
  assert.match(
    libraryScript,
    /return `\$\{number\.format\(size\.brotli\)\} \/ \$\{number\.format\(size\.gzip\)\} \/ \$\{number\.format\(size\.raw\)\}`/,
  );
});

test("popular matrix publishes only exact entrypoints that pass every gate", () => {
  assert.deepEqual(
    popular.results.map((result) => result.id),
    [
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
      "motion",
      "jquery",
    ],
  );
  assert.deepEqual(
    popular.results
      .filter((result) => result.eligible)
      .map((result) => result.id),
    ["nanoid", "mitt", "gl-matrix"],
  );
  assert.deepEqual(
    popular.results
      .filter((result) => result.exactSurface)
      .map((result) => result.id),
    ["nanoid", "mitt", "clsx", "gl-matrix"],
  );
  assert.equal(
    popular.results.find((result) => result.id === "nanoid").sizeGate,
    true,
  );
  assert.equal(
    popular.results.find((result) => result.id === "mitt").sizeGate,
    true,
  );
  assert.equal(
    popular.results.find((result) => result.id === "clsx").sizeGate,
    false,
  );
  assert.equal(
    popular.results.find((result) => result.id === "gl-matrix").sizeGate,
    true,
  );
  assert.equal(
    popular.results.find((result) => result.id === "clsx").performanceGate,
    true,
  );
  assert.equal(
    popular.results.find((result) => result.id === "redux-toolkit")
      .closureLevel,
    "SIMPLE",
  );
  assert.equal(
    popular.results.find((result) => result.id === "zod").closureLevel,
    "SIMPLE",
  );
  assert.equal(
    popular.results.find((result) => result.id === "preact").closureLevel,
    "SIMPLE",
  );
  const mitt = popular.results.find((result) => result.id === "mitt");
  assert.equal(mitt.lilscriptVite.raw, 598);
  assert.equal(mitt.vite.raw, 595);
  assert.equal(mitt.lilscriptVite.brotli, 300);
  assert.equal(mitt.vite.brotli, 300);
  assert.ok(mitt.performance.performance.ratio <= 1.05);
  assert.ok(mitt.performance.retainedMemory.ratio <= 1.05);
  const motion = popular.results.find((result) => result.id === "motion");
  assert.equal(motion.eligible, false);
  assert.equal(motion.exactSurface, false);
  assert.equal(motion.status, "candidate-selected-surface");
  assert.ok(motion.lilscriptVite.brotli < motion.vite.brotli);
  const jquery = popular.results.find((result) => result.id === "jquery");
  assert.equal(jquery.eligible, false);
  assert.equal(jquery.exactSurface, false);
  assert.equal(jquery.status, "candidate-full-library");
  assert.equal(jquery.lilscriptVite.brotli, 35176);
  assert.equal(jquery.terser.brotli, 27445);
  assert.ok(
    popular.results
      .filter(
        (result) =>
          result.status.includes("subset") ||
          result.status === "partial-external" ||
          result.status === "candidate-selected-surface",
      )
      .every(
        (result) => result.eligible === false && result.exactSurface === false,
      ),
  );
});

test("motion lab examples are openable from the libraries page", async () => {
  assert.match(page, /id="motion-lab-examples"/);
  assert.match(page, /data-motion-lab-examples/);
  const motionLab = JSON.parse(
    await readFile(
      new URL("../src/motion-lab-results.json", import.meta.url),
      "utf8",
    ),
  );
  assert.equal(motionLab.examples.length, 16);
  assert.equal(motionLab.wins, 16);
  for (const example of motionLab.examples) {
    assert.match(example.npmUrl, /^\/motion-lab\//);
    assert.match(example.lilUrl, /^\/motion-lab\//);
    assert.ok(example.brotliRatio < 1);
  }
});

test("Solid evidence separates complete client LSX parity from server exclusions", () => {
  assert.equal(clientRuntime.status, "runtime-exact-client-lsx-complete");
  assert.equal(clientRuntime.evidenceStatus, "integrated-runtime");
  assert.equal(clientRuntime.reproducibleFromIntegratedLab, true);
  assert.equal(clientRuntime.curatedCompatibility.casesPassed, 112);
  assert.equal(clientRuntime.curatedCompatibility.casesTotal, 112);
  assert.equal(clientRuntime.curatedCompatibility.executions, 224);
  assert.equal(clientRuntime.upstream.candidateTestsPassed, 469);
  assert.equal(clientRuntime.apiParity.verified, 135);
  assert.equal(clientRuntime.schemaVersion, 6);
  assert.equal(clientRuntime.surfaces.length, 4);
  assert.equal(
    clientRuntime.surfaces.filter(({ status }) => status === "eligible").length,
    3,
  );
  assert.deepEqual(
    clientRuntime.surfaces.map(({ id }) => id),
    ["core", "store", "web-client", "web-full"],
  );
  for (const surface of clientRuntime.surfaces) {
    assert.equal(surface.boundary, "open-world-distribution");
    assert.equal(
      surface.objectiveSuperior ?? surface.brotliRatio < 1,
      surface.id !== "web-full",
    );
    if (clientRuntime.schemaVersion >= 5) {
      assert.equal(surface.costModel, "brotli");
      assert.deepEqual(surface.crossMetricsAreDiagnostic, ["raw", "gzip9"]);
    }
  }
  assert.equal(
    clientRuntime.surfaces.find(({ id }) => id === "web-full").status,
    "optimization-gap",
  );
  assert.deepEqual(
    clientRuntime.closedWorldSurfaces.map(({ id }) => id),
    ["app-vite", "app-closure", "lsx-client-app"],
  );
  assert.equal(clientRuntime.comparisons.length, 7);
  for (const surface of clientRuntime.closedWorldSurfaces) {
    assert.equal(surface.boundary, "closed-world-application");
    assert.equal(surface.exportCount, null);
    assert.equal(surface.contractVerified, true);
    assert.equal(surface.resourceEquivalent, true);
    assert.equal(surface.status, "eligible");
    assert.ok(surface.brotliRatio < 1);
  }
  assert.match(
    clientRuntime.surfaces.find(({ id }) => id === "web-client").scope,
    /SSR and hydration excluded/,
  );
  assert.equal(clientRuntime.lifecycle.allSlotsReleased, true);
  assert.equal(clientRuntime.lsx.complete, true);
  assert.deepEqual(clientRuntime.remainingLsxFamilies, []);
  assert.deepEqual(clientRuntime.excludedServerFamilies, ["Hydration", "SSR"]);
  assert.equal(clientRuntime.lsxApplication.status, "eligible");
  assert.equal(clientRuntime.lsxApplication.behaviorEquivalent, true);
  assert.equal(clientRuntime.lsxApplication.unmountVerified, true);
  assert.equal(clientRuntime.lsxApplication.resourceEligible, true);
  const lsxBaseline = clientRuntime.lsxApplication.artifacts.find(
    (artifact) => artifact.id === "solid-lsx-vite",
  );
  const lsxCandidate = clientRuntime.lsxApplication.artifacts.find(
    (artifact) => artifact.id === "solidlil-lsx-vite",
  );
  assert.ok(lsxCandidate.brotli < lsxBaseline.brotli);
  assert.ok(clientRuntime.lsxApplication.performance.ratio <= 1.05);
  assert.ok(clientRuntime.lsxApplication.performance.memoryRatio <= 1.05);
  assert.ok(
    clientRuntime.lsxApplication.performance.disposedMemoryRatio <= 1.05,
  );
  assert.equal(
    clientRuntime.lsxApplication.performance.statistics.metrics.warmCpu
      .comparison.nonInferior,
    true,
  );
  assert.equal(
    clientRuntime.lsxApplication.performance.memoryStatistics.phases.disposed
      .heapUsed.comparison.nonInferior,
    true,
  );
  const solid = clientRuntime.appSnapshot.sizes.find(
    (artifact) => artifact.id === "solid-todolist",
  );
  const solidlil = clientRuntime.appSnapshot.sizes.find(
    (artifact) => artifact.id === "solidlil-lsx",
  );
  assert.ok(solidlil.brotli < solid.brotli);
  assert.equal(clientRuntime.appSnapshot.runtime, null);
});

test("every published LilScript library row passed native and C gates", () => {
  for (const result of data.results) {
    assert.deepEqual(
      result.artifacts.map((artifact) => artifact.id),
      ["vite", "closure", "lilscript"],
    );
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
      lilscriptSurface[data.metadata.selectedCodec] <=
        viteSurface[data.metadata.selectedCodec],
    );
    assert.ok(
      lilscriptSurface[data.metadata.selectedCodec] <=
        closureSurface[data.metadata.selectedCodec],
    );
  }
});

test("the page does not turn partial Motion support into a full claim", () => {
  assert.match(
    benchmarkPage,
    /Motion 13 is a candidate surface, not a full port/,
  );
  assert.match(benchmarkPage, /full published root contract remain outside/);
  assert.ok(
    data.auditedButIneligible.some((item) => item.package === "motion"),
  );
});
