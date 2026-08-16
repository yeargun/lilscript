import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import {
  JQUERY_LILSCRIPT_ARTIFACT_ENV,
  JQUERY_LILSCRIPT_ARTIFACT_SHA256_ENV,
  resolveJqueryLilscriptArtifact,
} from "./jquery-benchmark-artifact.mjs";

const root = dirname(fileURLToPath(import.meta.url));
const worker = join(root, "benchmark-jquery-worker.mjs");
const implementations = ["npm", "lilscript"];
const workloads = process.env.BENCH_WORKLOADS?.split(",") ?? ["core", "events", "deferred"];
const modes = ["performance", "memory"];
const rounds = Number(process.env.BENCH_ROUNDS ?? 7);
const selectedArtifact = resolveJqueryLilscriptArtifact({
  defaultArtifactPath: join(
    root,
    "build/jquery-config-audit/lean-balanced.terser.js",
  ),
});
const workerEnvironment = {
  ...process.env,
  [JQUERY_LILSCRIPT_ARTIFACT_ENV]: selectedArtifact.path,
  [JQUERY_LILSCRIPT_ARTIFACT_SHA256_ENV]: selectedArtifact.sha256,
};

function median(values) {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.floor(sorted.length / 2)];
}

function sample(implementation, mode, workload) {
  const result = spawnSync(
    process.execPath,
    ["--expose-gc", worker, implementation, mode, workload],
    {
      cwd: root,
      encoding: "utf8",
      env: workerEnvironment,
      maxBuffer: 16 * 1024 * 1024,
    },
  );
  if (result.status !== 0) throw new Error(result.stderr || result.stdout);
  return JSON.parse(result.stdout);
}

const report = {};
for (const workload of workloads) {
  const samples = Object.fromEntries(modes.map((mode) => [mode, { npm: [], lilscript: [] }]));
  for (const mode of modes) {
    for (let round = 0; round < rounds; round += 1) {
      const order = round & 1 ? [...implementations].reverse() : implementations;
      for (const implementation of order) {
        samples[mode][implementation].push(sample(implementation, mode, workload));
      }
    }
    assert.deepEqual(
      samples[mode].lilscript.map(({ checksum }) => checksum),
      samples[mode].npm.map(({ checksum }) => checksum),
      `${workload}/${mode} workload results differ`,
    );
  }
  const npmMs = median(samples.performance.npm.map(({ milliseconds }) => milliseconds));
  const lilscriptMs = median(samples.performance.lilscript.map(({ milliseconds }) => milliseconds));
  const npmBytes = median(samples.memory.npm.map(({ bytes }) => bytes));
  const lilscriptBytes = median(samples.memory.lilscript.map(({ bytes }) => bytes));
  report[workload] = {
    performance: { npmMs, lilscriptMs, ratio: lilscriptMs / npmMs },
    retainedMemory: { npmBytes, lilscriptBytes, ratio: lilscriptBytes / npmBytes },
    samples,
  };
}

console.log(JSON.stringify({
  rounds,
  artifact: selectedArtifact,
  report,
}, null, 2));
