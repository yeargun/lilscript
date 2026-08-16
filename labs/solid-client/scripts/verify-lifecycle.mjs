import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { root } from "./project.mjs";

const cycles = Number(process.env.LILSCRIPT_SOLID_LIFECYCLE_CYCLES ?? 5000);
const worker = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "benchmark-lifecycle-memory-worker.mjs",
);
const variants = {
  solid: {
    module: resolve(root, "node_modules/solid-js/dist/solid.js"),
  },
  solidlil: {
    module: resolve(root, "packages/solidlil/index.js"),
    diagnostics: resolve(root, "packages/solidlil/diagnostics.js"),
  },
};

function run(name, variant) {
  const args = ["--expose-gc", worker, name, variant.module, String(cycles)];
  if (variant.diagnostics) args.push(variant.diagnostics);
  const result = spawnSync(process.execPath, args, {
    cwd: root,
    encoding: "utf8",
    env: process.env,
  });
  if (result.status !== 0) {
    throw new Error(
      result.stderr || result.stdout || `${name} lifecycle verification failed`,
    );
  }
  return JSON.parse(result.stdout);
}

const solid = run("solid", variants.solid);
const solidlil = run("solidlil", variants.solidlil);
for (const field of [
  "cycles",
  "collectionCycles",
  "resourceCycles",
  "roots",
  "collections",
  "resources",
  "staleDisposer",
]) {
  assert.deepEqual(solidlil[field], solid[field], `${field} parity`);
}
assert.deepEqual(solidlil.slots, solidlil.warmSlots, "stable slot high-water");
assert.equal(solidlil.slots.owners, solidlil.slots.freeOwners);
assert.equal(solidlil.slots.effects, solidlil.slots.freeEffects);
assert.equal(solidlil.slots.pendingEffects, 0);

const report = {
  generatedAt: new Date().toISOString(),
  definition:
    "Solid and SolidLil execute identical ownership/disposal workloads; retained-heap eligibility is measured separately with repeated isolated samples.",
  cycles,
  collectionCycles: solidlil.collectionCycles,
  resourceCycles: solidlil.resourceCycles,
  workloads: {
    roots: solidlil.roots,
    collections: solidlil.collections,
    resources: solidlil.resources,
    staleDisposer: solidlil.staleDisposer,
  },
  solidRetainedBytesDiagnostic: solid.bytes,
  solidlilRetainedBytesDiagnostic: solidlil.bytes,
  warmSlots: solidlil.warmSlots,
  slots: solidlil.slots,
  behaviorEquivalent: true,
  stableHighWater: true,
  allSlotsReleased: true,
};
writeFileSync(
  resolve(root, "artifacts/lifecycle-parity.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);
writeFileSync(
  resolve(root, "artifacts/lifecycle-parity.md"),
  `# SolidLil lifecycle parity\n\n${report.definition}\n\n- ${cycles.toLocaleString("en-US")} root/signal/memo/effect/cleanup cycles\n- ${report.collectionCycles.toLocaleString("en-US")} keyed and indexed collection cycles\n- ${report.resourceCycles.toLocaleString("en-US")} resources resolved after root disposal\n- stale disposer after slot reuse: pass\n- SolidLil owner/effect high-water: ${report.slots.owners}/${report.slots.effects}\n- all slots released and pending queue empty: pass\n`,
);

console.log(
  `Lifecycle parity passed: ${cycles.toLocaleString("en-US")} root cycles, ` +
    `${solidlil.collectionCycles.toLocaleString("en-US")} collection cycles, ` +
    `${solidlil.resourceCycles.toLocaleString("en-US")} disposed resources; ` +
    `${solidlil.slots.owners}/${solidlil.slots.effects} stable owner/effect slots.`,
);
