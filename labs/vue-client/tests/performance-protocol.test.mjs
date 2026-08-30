import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { resolve } from "node:path";
import { buildInventory, inventoryPath } from "../scripts/audit-scope.mjs";
import { validateBenchmarkManifest } from "../scripts/benchmark.mjs";
import { sha256 } from "../scripts/check-complete.mjs";
import {
  pairedRatioStatistics,
  performanceEvidenceFailures,
} from "../scripts/performance-protocol.mjs";

const root = resolve(import.meta.dirname, "..");
const manifestPath = resolve(root, "benchmarks/benchmark-inputs.json");
const reportPath = resolve(root, "artifacts/performance-report.json");

test("benchmark inputs pin current VueLil against all required Vue 3.5.42 packages", () => {
  const inventory = buildInventory();
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  const workloads = validateBenchmarkManifest(manifest, inventory, manifestPath);
  assert.deepEqual(
    new Set(workloads.map(({ category }) => category)),
    new Set(["browser", "compiler", "reactivity", "ssr"]),
  );
  assert.ok(workloads.every(({ candidatePackage }) =>
    candidatePackage.version === "3.5.42-vuelil"));
  assert.ok(workloads.every(({ upstreamPackage }) => upstreamPackage.version === "3.5.42"));
});

test("paired bootstrap upper bound is deterministic and detects regression", () => {
  const upstream = Array.from({ length: 21 }, (_, index) => 100 + (index % 5));
  const candidate = upstream.map((value) => value * 1.01);
  const options = { bootstrapIterations: 10_000, margin: 1.03, seed: "fixed" };
  const first = pairedRatioStatistics(upstream, candidate, options);
  const second = pairedRatioStatistics(upstream, candidate, options);
  assert.deepEqual(first, second);
  assert.equal(first.confidenceInterval.upper95, 1.01);
  assert.equal(first.nonInferior, true);
  assert.equal(
    pairedRatioStatistics(upstream, upstream.map((value) => value * 1.04), options)
      .nonInferior,
    false,
  );
});

test("report validation fails closed on order, checksum, and statistics tampering", () => {
  const inventoryBytes = readFileSync(inventoryPath);
  const inventory = JSON.parse(inventoryBytes);
  const observations = Array.from({ length: 21 }, (_, block) => ({
    block,
    order: block % 2 === 0
      ? ["candidate", "upstream"]
      : ["upstream", "candidate"],
    candidate: { durationMs: 99, operations: 1_000, checksum: "same" },
    upstream: { durationMs: 100, operations: 1_000, checksum: "same" },
  }));
  const evidence = (path) => ({ path, sha256: "a".repeat(64), bytes: 1 });
  const graph = (module) => ({
    sha256: sha256(Buffer.from(`${JSON.stringify([module])}\n`)),
    modules: [module],
  });
  const categories = {
    browser: ["vue", "node_modules/vue/index.js"],
    compiler: ["@vue/compiler-dom", "node_modules/@vue/compiler-dom/index.js"],
    reactivity: ["@vue/reactivity", "node_modules/@vue/reactivity/index.js"],
    ssr: ["@vue/server-renderer", "node_modules/@vue/server-renderer/index.js"],
  };
  const protocol = {
    design: "paired isolated blocks with candidate/upstream execution order alternating every block",
    seed: "report-test",
    samples: 21,
    warmupRuns: 1,
    bootstrapIterations: 1_000,
    nonInferiorityMarginRatio: 1.03,
    confidenceLevel: 0.95,
    confidenceBound: "one-sided upper",
    sampleAdequacyOverride: false,
  };
  const report = {
    schemaVersion: 1,
    upstream: {
      version: inventory.upstream.version,
      revision: inventory.upstream.revision,
    },
    inventory: { sha256: sha256(inventoryBytes) },
    implementations: {
      candidate: {
        package: "vue",
        version: "3.5.42-vuelil",
        packageManifest: evidence("packages/vuelil/package.json"),
      },
      upstream: Object.entries(categories).map(([, [name]]) => ({
        package: name,
        version: "3.5.42",
        packageManifest: evidence(`node_modules/${name}/package.json`),
      })),
    },
    protocol,
    workloads: Object.entries(categories).map(([category, [, upstreamPath]]) => ({
      id: category,
      category,
      nodeEnv: category === "compiler" ? "development" : "production",
      description: `Meaningful ${category} workload with enough detail for evidence.`,
      metric: "durationMs",
      runner: evidence(`benchmarks/runners/${category}.mjs`),
      candidateArtifact: evidence(`packages/vuelil/${category}.js`),
      upstreamArtifact: evidence(upstreamPath),
      candidatePackageManifest: evidence("packages/vuelil/package.json"),
      upstreamPackageManifest: evidence(`node_modules/${categories[category][0]}/package.json`),
      candidateGraph: graph(evidence(`packages/vuelil/${category}.js`)),
      upstreamGraph: graph(evidence(upstreamPath)),
      environment: category === "browser"
        ? { engine: "Chromium", version: "1" }
        : { engine: "Node.js", version: "v24" },
      statistics: {
        ratio: pairedRatioStatistics(
          observations.map(({ upstream }) => upstream.durationMs),
          observations.map(({ candidate }) => candidate.durationMs),
          {
            bootstrapIterations: protocol.bootstrapIterations,
            margin: protocol.nonInferiorityMarginRatio,
            seed: `${protocol.seed}:${category}:bootstrap`,
          },
        ),
      },
      observations: structuredClone(observations),
      passed: true,
    })),
    passed: true,
  };
  assert.deepEqual(
    performanceEvidenceFailures(report, inventory, sha256(inventoryBytes)),
    [],
  );

  const tampered = structuredClone(report);
  tampered.workloads[0].observations[1].order = ["candidate", "upstream"];
  tampered.workloads[1].observations[0].candidate.checksum = "different";
  tampered.workloads[2].statistics.ratio.confidenceInterval.upper95 = 0.5;
  const failures = performanceEvidenceFailures(tampered, inventory, sha256(inventoryBytes));
  assert.ok(failures.some((failure) => failure.includes("browser")));
  assert.ok(failures.some((failure) => failure.includes("compiler")));
  assert.ok(failures.some((failure) => failure.includes("reactivity")));
});

test("generated performance evidence is structurally valid and scope follows its result", () => {
  const inventoryBytes = readFileSync(inventoryPath);
  const inventory = JSON.parse(inventoryBytes);
  const report = JSON.parse(readFileSync(reportPath, "utf8"));
  const scope = JSON.parse(
    readFileSync(resolve(root, "compatibility/scope.json"), "utf8"),
  );
  assert.deepEqual(
    performanceEvidenceFailures(report, inventory, sha256(inventoryBytes), {
      requirePassing: false,
    }),
    [],
  );
  assert.equal(report.passed, report.workloads.every(({ passed }) => passed));
  assert.equal(scope.gates.performance, report.passed ? "passed" : "pending");
});
