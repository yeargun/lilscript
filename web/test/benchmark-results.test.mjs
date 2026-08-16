import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const page = await readFile(
  new URL("../benchmarks.html", import.meta.url),
  "utf8",
);
const script = await readFile(
  new URL("../src/benchmarks.js", import.meta.url),
  "utf8",
);
const data = JSON.parse(
  await readFile(
    new URL("../src/benchmark-results.json", import.meta.url),
    "utf8",
  ),
);
const comparisonSummary = JSON.parse(
  await readFile(
    new URL("../src/comparison-summary.json", import.meta.url),
    "utf8",
  ),
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
    for (const artifact of result.artifacts)
      assert.equal("samplesMs" in artifact, false);
    if (result.ecosystem) assert.equal("samplesMs" in result.ecosystem, false);
  }
});

test("static copy scopes the Motion candidate without making a full claim", () => {
  assert.match(page, /Motion 13 is a candidate surface, not a full port/);
  assert.doesNotMatch(page, /Motion value pipeline/);
  assert.doesNotMatch(page, /LilScript is \d+ bytes smaller/);
});

test("comparison page explains averages by scale before exposing technical tables", () => {
  assert.match(page, /Where Lilscript wins—and where it mostly ties/);
  assert.match(page, /data-overall-verdict/);
  assert.match(page, /id="small-scripts"/);
  assert.match(page, /id="package-projects"/);
  assert.match(page, /id="framework-runtimes"/);
  assert.match(page, /id="ui-projects"/);
  assert.match(page, /Selection warning/);
  assert.match(page, /<details class="evidence-disclosure">/);
  assert.match(script, /comparison-summary\.json/);
  assert.doesNotMatch(script, /library-results\.json/);
  assert.doesNotMatch(script, /popular-library-results\.json/);
  assert.equal(comparisonSummary.overall.count, 28);
  assert.equal(comparisonSummary.overall.wins, 25);
  assert.equal(comparisonSummary.overall.ties, 2);
  assert.equal(comparisonSummary.overall.losses, 1);
  assert.deepEqual(
    comparisonSummary.frameworkRuntime.rows.map(({ id }) => id),
    [
      "core",
      "store",
      "web-client",
      "web-full",
      "app-vite",
      "app-closure",
      "lsx-client-app",
    ],
  );
  assert.deepEqual(
    comparisonSummary.frameworkRuntime.rows.map(({ boundary }) => boundary),
    [
      "open-world-distribution",
      "open-world-distribution",
      "open-world-distribution",
      "open-world-distribution",
      "closed-world-application",
      "closed-world-application",
      "closed-world-application",
    ],
  );
  assert.match(
    script,
    /Different categories use different fair controls|category breakdown is the honest result/,
  );
  assert.match(script, /gzip ties/);
  assert.match(script, /Complete client parity fixture/i);
  assert.match(script, /21 in-scope client-rendering families/i);
  assert.match(
    script,
    /Hydration and SSR remain explicit server-coupled exclusions/i,
  );
});
