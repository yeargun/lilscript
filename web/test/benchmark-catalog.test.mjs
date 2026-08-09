import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const catalog = JSON.parse(await readFile(new URL("../src/benchmark-catalog.json", import.meta.url), "utf8"));

test("catalog keys and artifact rows are unique and complete", () => {
  assert.equal(new Set(catalog.projects.map((project) => project.key)).size, catalog.projects.length);
  const rows = catalog.projects.flatMap((project) => project.artifacts.map((artifact) => `${project.key}:${artifact.id}`));
  assert.equal(new Set(rows).size, rows.length);
  assert.equal(rows.length, catalog.metadata.artifactCount);
  for (const project of catalog.projects) {
    assert.ok(project.artifacts.length > 0, project.key);
    for (const artifact of project.artifacts) {
      assert.ok(Number.isInteger(artifact.raw) && artifact.raw > 0, `${project.key}/${artifact.id}/raw`);
      assert.ok(Number.isInteger(artifact.gzip) && artifact.gzip > 0, `${project.key}/${artifact.id}/gzip`);
      assert.ok(Number.isInteger(artifact.brotli) && artifact.brotli > 0, `${project.key}/${artifact.id}/brotli`);
    }
  }
});

test("real scenarios expose the full fair lane matrix", () => {
  for (const id of ["login-risk", "animation-timeline", "geometry-hit-test"]) {
    const project = catalog.projects.find((candidate) => candidate.key === `scenario:${id}`);
    assert.ok(project, id);
    assert.deepEqual(new Set(project.artifacts.map((artifact) => artifact.id)), new Set([
      "vite-unminified", "vite-oxc", "vite-terser-properties", "closure-advanced",
      "lilscript-unmangled", "lilscript-public-safe", "lilscript-closed-world", "lilscript-vite-oxc",
    ]));
    assert.equal(project.verification.native, true);
    assert.ok(project.sources.some((source) => source.language === "javascript"));
    assert.ok(project.sources.some((source) => source.language === "lilscript"));
  }
});

test("property stress proves a real LilScript property-mangling delta", () => {
  const project = catalog.projects.find((candidate) => candidate.key === "scenario:property-ledger");
  const safe = project.artifacts.find((artifact) => artifact.id === "lilscript-public-safe");
  const closed = project.artifacts.find((artifact) => artifact.id === "lilscript-closed-world");
  assert.ok(closed.raw < safe.raw);
  assert.ok(closed.brotli < safe.brotli);
  assert.equal(project.verification.native, false);
});

test("detail and explorer pages keep project navigation in new tabs", async () => {
  const explorer = await readFile(new URL("../explorer.html", import.meta.url), "utf8");
  const script = await readFile(new URL("../src/explorer.js", import.meta.url), "utf8");
  const detail = await readFile(new URL("../benchmark-detail.html", import.meta.url), "utf8");
  assert.match(explorer, /data-filter-category/);
  assert.match(explorer, /data-sort/);
  assert.match(script, /target="_blank"/);
  assert.match(script, /benchmark-detail\.html\?project=/);
  assert.match(detail, /data-project-detail/);
});
