import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const catalog = JSON.parse(
  await readFile(
    new URL("../src/benchmark-catalog.json", import.meta.url),
    "utf8",
  ),
);

test("catalog keys and artifact rows are unique and complete", () => {
  assert.equal(
    new Set(catalog.projects.map((project) => project.key)).size,
    catalog.projects.length,
  );
  const rows = catalog.projects.flatMap((project) =>
    project.artifacts.map((artifact) => `${project.key}:${artifact.id}`),
  );
  assert.equal(new Set(rows).size, rows.length);
  assert.equal(rows.length, catalog.metadata.artifactCount);
  for (const project of catalog.projects) {
    assert.ok(project.artifacts.length > 0, project.key);
    for (const artifact of project.artifacts) {
      assert.ok(
        Number.isInteger(artifact.raw) && artifact.raw > 0,
        `${project.key}/${artifact.id}/raw`,
      );
      assert.ok(
        Number.isInteger(artifact.gzip) && artifact.gzip > 0,
        `${project.key}/${artifact.id}/gzip`,
      );
      assert.ok(
        Number.isInteger(artifact.brotli) && artifact.brotli > 0,
        `${project.key}/${artifact.id}/brotli`,
      );
    }
  }
});

test("real scenarios expose the full fair lane matrix", () => {
  for (const id of ["login-risk", "animation-timeline", "geometry-hit-test"]) {
    const project = catalog.projects.find(
      (candidate) => candidate.key === `scenario:${id}`,
    );
    assert.ok(project, id);
    assert.deepEqual(
      new Set(project.artifacts.map((artifact) => artifact.id)),
      new Set([
        "vite-unminified",
        "vite-oxc",
        "vite-terser-properties",
        "closure-advanced",
        "lilscript-unmangled",
        "lilscript-public-safe",
        "lilscript-closed-world",
        "lilscript-vite-oxc",
      ]),
    );
    assert.equal(project.verification.native, true);
    assert.ok(
      project.sources.some((source) => source.language === "javascript"),
    );
    assert.ok(
      project.sources.some((source) => source.language === "lilscript"),
    );
  }
});

test("property stress proves a Brotli-objective property-mangling delta", () => {
  const project = catalog.projects.find(
    (candidate) => candidate.key === "scenario:property-ledger",
  );
  const safe = project.artifacts.find(
    (artifact) => artifact.id === "lilscript-public-safe",
  );
  const closed = project.artifacts.find(
    (artifact) => artifact.id === "lilscript-closed-world",
  );
  assert.ok(closed.brotli < safe.brotli);
  assert.equal(project.verification.native, false);
});

test("catalog publishes exact SolidLil surfaces with explicit Web scopes", () => {
  const surfaces = ["core", "store", "web-client", "web-full"].map((id) =>
    catalog.projects.find(
      (candidate) => candidate.key === `framework:solidlil-${id}`,
    ),
  );
  assert.ok(surfaces.every(Boolean));
  assert.deepEqual(
    surfaces.map(({ status }) => status),
    ["eligible", "eligible", "eligible", "blocked"],
  );
  for (const project of surfaces) {
    assert.deepEqual(
      project.artifacts.map(({ id }) => id),
      ["solid", "solidlil"],
    );
    assert.equal(project.verification.exactExports, true);
    assert.equal(project.verification.behaviorEquivalent, true);
    assert.equal(project.verification.boundary, "open-world-distribution");
    const solid = project.artifacts.find(({ id }) => id === "solid");
    const solidlil = project.artifacts.find(({ id }) => id === "solidlil");
    assert.equal(
      project.verification.objectiveSuperior ?? solidlil.brotli < solid.brotli,
      project.id !== "solidlil-web-full",
    );
  }
  assert.match(
    surfaces.find(({ id }) => id === "solidlil-web-client").summary,
    /SSR and hydration are explicitly outside this target/,
  );
  assert.match(
    surfaces.find(({ id }) => id === "solidlil-web-full").blockers[0],
    /Brotli-objective gate is open/,
  );
});

test("catalog publishes complete client LSX parity with explicit server exclusions", () => {
  const project = catalog.projects.find(
    (candidate) => candidate.key === "framework:solidlil-lsx",
  );
  assert.ok(project);
  assert.equal(project.status, "eligible");
  assert.deepEqual(project.blockers, []);
  assert.deepEqual(project.exclusions, ["Hydration", "SSR"]);
  assert.equal(project.verification.behaviorEquivalent, true);
  assert.equal(project.verification.unmountVerified, true);
  assert.equal(project.verification.resourceEligible, true);
  assert.ok(project.verification.timeRatio <= 1.05);
  assert.ok(project.verification.liveMemoryRatio <= 1.05);
  assert.ok(project.verification.disposedMemoryRatio <= 1.05);
  const baseline = project.artifacts.find(
    (artifact) => artifact.id === "solid-lsx-vite",
  );
  const candidate = project.artifacts.find(
    (artifact) => artifact.id === "solidlil-lsx-vite",
  );
  assert.ok(candidate.brotli < baseline.brotli);
});

test("detail and explorer pages keep project navigation in new tabs", async () => {
  const explorer = await readFile(
    new URL("../explorer.html", import.meta.url),
    "utf8",
  );
  const script = await readFile(
    new URL("../src/explorer.js", import.meta.url),
    "utf8",
  );
  const detail = await readFile(
    new URL("../benchmark-detail.html", import.meta.url),
    "utf8",
  );
  assert.match(explorer, /data-filter-category/);
  assert.match(explorer, /data-column-view/);
  assert.match(explorer, /value="core">Core comparison/);
  assert.match(explorer, /data-sort/);
  assert.match(explorer, /value="core">Core evidence first/);
  assert.match(script, /show-all-columns/);
  assert.match(script, /projectIndex - right\.projectIndex/);
  assert.match(script, /target="_blank"/);
  assert.match(script, /benchmark-detail\.html\?project=/);
  assert.match(detail, /data-project-detail/);
});

test("explorer explains aggregate compression rates and fair comparison", async () => {
  const explorer = await readFile(
    new URL("../explorer.html", import.meta.url),
    "utf8",
  );
  const script = await readFile(
    new URL("../src/explorer.js", import.meta.url),
    "utf8",
  );
  assert.match(explorer, /data-aggregate-summary/);
  assert.match(explorer, /Overall averages/);
  assert.match(explorer, /compression saved =/);
  assert.match(explorer, /Global averages are descriptive/);
  assert.match(explorer, /Compare inside one project/);
  assert.match(script, /weightedGzipReduction/);
  assert.match(script, /metric-rate/);
});
