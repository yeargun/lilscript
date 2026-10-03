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

test("the current Solid library row states its experimental compatibility boundary", async () => {
  const releases=JSON.parse(await readFile(new URL("../src/library-releases.json",import.meta.url),"utf8"));
  const solid=releases.libraries.find(row=>row.name==="solidlil");
  assert.match(solid.scope,/Experimental Solid 2 compatibility/);
  assert.match(solid.scope,/JavaScript providers/);
  assert.equal(solid.objectives.length,3);
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

test("legacy explorer and detail routes expose the current comparison index", async () => {
  for (const name of ["explorer.html", "benchmark-detail.html"]) {
    const page = await readFile(new URL(`../${name}`, import.meta.url), "utf8");
    assert.match(page, /Current LilScript versus the minified original/);
    assert.match(page, /id="ports"/);
    assert.match(page, /id="build-times"/);
    assert.match(page, /library-releases.json/);
    assert.doesNotMatch(page, /previous compiler|previous version/);
  }
});
