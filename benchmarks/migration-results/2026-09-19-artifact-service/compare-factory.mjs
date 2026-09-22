import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const directory = dirname(fileURLToPath(import.meta.url));
const output = join(directory, "shared-recipe-factory-comparison");
mkdirSync(output);
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const filter = "semantic_program::search_tests::structural_discovery_matches_independent_twelve_state_oracle_and_measured_union";
const arms = [
  { label: "before", binary: "/tmp/lilscript-artifact-service-baseline-20260919/libtest", receipt: "run-2026-09-19T13-05-45.258Z/receipt.json" },
  { label: "after", binary: "/tmp/lilscript-session-descriptor-baseline-20260919/lilscript-c737e884cfca95f9", receipt: "run-2026-09-19T13-32-24.596Z/receipt.json" },
];
const results = [];
for (const arm of arms) {
  const receiptBytes = readFileSync(join(directory, arm.receipt));
  const qualification = JSON.parse(receiptBytes);
  const binarySha256 = hash(readFileSync(arm.binary));
  assert(qualification.passed && qualification.inputsStable);
  assert(qualification.testBinaries.some(binary => binary.sha256 === binarySha256));
  const started = performance.now();
  const run = spawnSync(arm.binary, [filter, "--exact", "--nocapture"], { encoding: "utf8", timeout: 60_000, maxBuffer: 8 * 1024 * 1024 });
  writeFileSync(join(output, `${arm.label}.stdout`), run.stdout ?? "");
  writeFileSync(join(output, `${arm.label}.stderr`), run.stderr ?? "");
  assert.equal(run.status, 0, run.stderr);
  assert.equal(hash(readFileSync(arm.binary)), binarySha256);
  const lines = run.stderr.split("\n");
  const observations = lines.filter(line => line.startsWith("search-artifact ")).map(line => JSON.parse(line.slice("search-artifact ".length)));
  const summary = lines.find(line => line.startsWith("search-summary "));
  assert(summary && observations.length === 72);
  results.push({ ...arm, receiptSha256: hash(receiptBytes), compilerInputsSha256: qualification.inputSha256,
    binarySha256, command: [arm.binary, filter, "--exact", "--nocapture"], status: run.status,
    diagnosticMilliseconds: performance.now() - started,
    summary: JSON.parse(summary.slice("search-summary ".length)), observations });
}
const stable = observations => observations.map(({ recipe_fingerprint, ...actual }) => actual);
assert.deepEqual(stable(results[0].observations), stable(results[1].observations));
assert.deepEqual(results[0].summary, results[1].summary);
const receipt = {
  schema: 1, passed: true, node: process.version, harnessSha256: hash(readFileSync(fileURLToPath(import.meta.url))),
  scope: "One fixed 12-state factory and 36 named artifacts across the shared recipe/session change",
  exactArtifactSequenceUnchanged: true, independentWinnerSizes: results[1].summary.best_sizes, arms: results,
  limitations: ["Fresh recipe fingerprints intentionally change with resource/schedule identity; they are not artifact-byte equality.",
    "One debug sample per arm establishes neither speed improvement nor a fleet-wide no-regression claim."]
};
writeFileSync(join(output, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n");
console.log(JSON.stringify({ passed: receipt.passed, sizes: receipt.independentWinnerSizes, summary: results[1].summary }));
