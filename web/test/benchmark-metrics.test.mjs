import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { formatBytes, percentageSaved, summarizeArtifacts } from "../src/benchmark-metrics.js";

const catalog = JSON.parse(await readFile(new URL("../src/benchmark-catalog.json", import.meta.url), "utf8"));
const artifacts = catalog.projects.flatMap((project) => project.artifacts);
const closeTo = (actual, expected) => assert.ok(Math.abs(actual - expected) < 0.000_001, `${actual} != ${expected}`);

test("compression metrics distinguish equal-row and byte-weighted averages", () => {
  const summary = summarizeArtifacts([
    { raw: 100, gzip: 50, brotli: 40 },
    { raw: 900, gzip: 810, brotli: 720 },
  ]);
  assert.equal(summary.count, 2);
  closeTo(summary.meanRaw, 500);
  closeTo(summary.meanGzipReduction, 30);
  closeTo(summary.meanBrotliReduction, 40);
  closeTo(summary.meanBrotliEdge, 15.555_555_555_555_557);
  closeTo(summary.weightedGzipReduction, 14);
  closeTo(summary.weightedBrotliReduction, 24);

  const catalogSummary = summarizeArtifacts(artifacts);
  assert.equal(catalogSummary.count, catalog.metadata.artifactCount);
  assert.ok(Number.isFinite(catalogSummary.meanGzipReduction));
  assert.ok(Number.isFinite(catalogSummary.weightedGzipReduction));
});

test("compression helpers handle rate and display boundaries", () => {
  assert.equal(percentageSaved(1_000, 250), 75);
  assert.equal(percentageSaved(0, 10), 0);
  assert.equal(summarizeArtifacts([]), null);
  assert.equal(formatBytes(999), "999 B");
  assert.equal(formatBytes(1_550), "1.6 kB");
  assert.equal(formatBytes(1_550_000), "1.6 MB");
});
