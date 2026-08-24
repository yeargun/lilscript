import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import test from "node:test";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const directory = dirname(fileURLToPath(import.meta.url));
const root = resolve(directory, "../..");
const reportPath = join(directory, "report.html");

function readReport() {
  const html = readFileSync(reportPath, "utf8");
  const match = html.match(
    /<script id="atlas-data" type="application\/json">(.*?)<\/script>/su,
  );
  assert.ok(match, "embedded atlas payload");
  return { html, data: JSON.parse(match[1]) };
}

test("checked-in report matches canonical measurements", () => {
  const result = spawnSync(
    process.execPath,
    [join(directory, "build.mjs"), "--check"],
    { cwd: root, encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /Verified 103 races \/ 336 variants/u);
});

test("report is self-contained and its UI script parses", () => {
  const { html } = readReport();
  assert.match(html, /^<!doctype html>/u);
  assert.doesNotMatch(html, /<script[^>]+src=/iu);
  assert.doesNotMatch(html, /<link[^>]+(?:stylesheet|preload)/iu);
  const scripts = [...html.matchAll(/<script>([\s\S]*?)<\/script>/gu)];
  assert.equal(scripts.length, 1, "one executable inline UI script");
  assert.doesNotThrow(() => new Function(scripts[0][1]));
});

test("payload preserves semantic and codec contracts", () => {
  const { data } = readReport();
  assert.equal(data.schemaVersion, 1);
  assert.equal(data.counts.races, 103);
  assert.equal(data.counts.variants, 336);
  assert.equal(data.repeatCount, 32);
  assert.equal(data.codecs.gzip9.libraryVersion, "1.3.1");
  assert.equal(data.codecs.brotli11.libraryVersion, "1.1.0");
  assert.equal(data.context.sizes.raw, data.context.bytes);
  assert.match(data.context.sha256, /^[a-f0-9]{64}$/u);
  assert.ok(data.summary.disagreements.repeated.rawVsBrotli > 0);
  assert.ok(data.summary.disagreements.context.rawVsBrotli > 0);
  assert.ok(data.summary.laneFlips.brotli.singleVsContext > 0);

  for (const race of data.races) {
    assert.ok(race.contract.length > 0, `${race.id} contract`);
    assert.ok(race.caveat.length > 0, `${race.id} caveat`);
    assert.ok(race.variants.some((variant) => variant.safety !== "trap"));
    for (const variant of race.variants) {
      assert.deepEqual(Object.keys(variant.sizes).sort(), ["context", "repeated", "single"]);
      assert.equal(variant.sizes.single.raw, Buffer.byteLength(variant.code));
      for (const lane of Object.values(variant.sizes)) {
        for (const value of Object.values(lane)) assert.ok(Number.isSafeInteger(value));
      }
    }
  }
});
