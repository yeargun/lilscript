import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
import test from "node:test";
const read = name => readFile(new URL(`../${name}`, import.meta.url), "utf8");
const data = JSON.parse(await read("src/library-releases.json"));
test("every comparison can be traced to its published source and exact artifact hashes", () => {
  assert.match(data.verifiedAt, /^\d{4}-\d{2}-\d{2}T/);
  for (const row of data.libraries) {
    assert.match(row.evidenceSha256, /^[a-f0-9]{64}$/);
    assert.match(row.compilerSha256, /^[a-f0-9]{64}$/);
    assert.ok(row.upstream.includes("@"));
    assert.ok(row.timingScope.includes("Original") || row.name === "posthoglil");
    for (const objective of row.objectives) {
      assert.ok(objective.originalArtifact.startsWith(row.homepage));
      assert.ok(objective.lilscriptArtifact.startsWith(row.homepage));
      assert.match(objective.originalSha256, /^[a-f0-9]{64}$/);
      assert.match(objective.lilscriptSha256, /^[a-f0-9]{64}$/);
    }
  }
});
test("benchmark route explains objective selection, timings and scope", async () => {
  const page = await read("benchmarks.html");
  assert.match(page, /independently|own optimization objective/);
  assert.match(page, /shared public exports/);
  assert.match(page, /exclude building the upstream repository/);
  assert.match(page, /Download size does not establish parse speed or runtime speed/);
});
