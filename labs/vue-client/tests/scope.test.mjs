import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  buildInventory,
  inventoryPath,
  scopePath,
  serializeInventory,
} from "../scripts/audit-scope.mjs";
import {
  evaluateCompletion,
  expectedBundleScenarioIds,
  expectedFormatKeys,
  sha256,
} from "../scripts/check-complete.mjs";
import { candidatePathFor } from "../scripts/audit-source-parity.mjs";
import { pairedRatioStatistics } from "../scripts/performance-protocol.mjs";

const scope = JSON.parse(readFileSync(scopePath, "utf8"));
const inventory = JSON.parse(readFileSync(inventoryPath, "utf8"));

function jsonDigest(value) {
  return sha256(Buffer.from(`${JSON.stringify(value, null, 2)}\n`));
}

test("inventory is an exact audit of the pinned clean Vue checkout", () => {
  const live = buildInventory();
  assert.equal(serializeInventory(live), readFileSync(inventoryPath, "utf8"));
  assert.equal(live.upstream.revision, scope.upstream.revision);
  assert.equal(live.upstream.tag, scope.upstream.tag);
  assert.equal(live.upstream.clean, true);
  assert.equal(live.totals.packages, 12);
  assert.equal(live.totals.sourceFiles, 234);
  assert.equal(live.totals.upstreamTestFiles, 196);
  assert.equal(live.totals.declarationTestFiles, 20);

  const sourceFiles = live.packages.flatMap((entry) => entry.sourceFiles);
  const testFiles = live.packages.flatMap((entry) => entry.testFiles);
  assert.equal(new Set(sourceFiles).size, sourceFiles.length);
  assert.equal(new Set(testFiles).size, testFiles.length);
  assert.ok(sourceFiles.every((path) => path.startsWith("packages/")));
  assert.ok(testFiles.every((path) => /\.(?:spec|test)\.[cm]?[jt]sx?$/u.test(path)));
  assert.equal(
    new Set(live.declarationTestFiles).size,
    live.declarationTestFiles.length,
  );
});

test("inventory derives representative public exports through re-export graphs", () => {
  const exportsFor = (name) =>
    inventory.packages.find((entry) => entry.name === name).publicExports;
  assert.ok(exportsFor("@vue/shared").includes("makeMap"));
  assert.ok(exportsFor("@vue/reactivity").includes("reactive"));
  assert.ok(exportsFor("@vue/runtime-core").includes("createRenderer"));
  assert.ok(exportsFor("@vue/runtime-dom").includes("createApp"));
  assert.ok(exportsFor("@vue/compiler-core").includes("baseCompile"));
  assert.ok(exportsFor("@vue/compiler-dom").includes("baseCompile"));
  assert.deepEqual(exportsFor("@vue/compiler-ssr"), ["compile"]);
  assert.ok(exportsFor("@vue/server-renderer").includes("renderToString"));
  assert.deepEqual(exportsFor("@vue/compat"), ["default"]);
  assert.ok(exportsFor("vue").includes("compile"));
  for (const entry of inventory.packages) {
    assert.equal(new Set(entry.publicExports).size, entry.publicExports.length);
  }
});

test("scope test-file totals match complete and test-host evidence", () => {
  const completePackages = new Set(
    scope.packages
      .filter(entry =>
        entry.status === "complete" ||
        entry.status === "runtime-complete" ||
        entry.status === "test-host-only"
      )
      .map(entry => entry.name),
  );
  const passed = inventory.packages
    .filter(entry => completePackages.has(entry.name))
    .reduce((total, entry) => total + entry.testFiles.length, 0);
  const declarationPassed = scope.gates.declarations === "passed"
    ? inventory.totals.declarationTestFiles
    : 0;
  assert.equal(scope.gates.runtimeTestFilesPassed, passed);
  assert.equal(scope.gates.declarationTestFilesPassed, declarationPassed);
  assert.equal(scope.gates.candidatePassed, passed + declarationPassed);
  assert.equal(scope.gates.candidateFailed, 0);
  assert.equal(
    scope.gates.candidatePending,
    inventory.totals.upstreamTestFiles - passed,
  );
  assert.equal(
    scope.packages.find(entry => entry.name === "@vue/runtime-dom")?.status,
    "complete",
  );
  assert.equal(
    scope.packages.find(entry => entry.name === "@vue/runtime-test")?.status,
    "test-host-only",
  );
});

test("current completion state fails closed", () => {
  const result = evaluateCompletion({ scope, inventory });
  assert.equal(result.complete, false);
  assert.ok(result.failures.some((failure) => failure.includes("source-parity evidence is missing")));
  assert.ok(result.failures.some((failure) => failure.includes("compatibility evidence is missing")));
  assert.ok(result.failures.some((failure) => failure.includes("size evidence is missing")));
  assert.ok(result.failures.some((failure) => failure.includes("runtime-only-client")));
  assert.ok(result.failures.some((failure) => failure.includes("Pages evidence is missing")));
});

test("status edits alone cannot manufacture completion", () => {
  const claimed = structuredClone(scope);
  claimed.packages = claimed.packages.map((entry) => ({
    ...entry,
    status: entry.name === "@vue/runtime-test" ? "test-host-only" : "complete",
  }));
  const candidateTotal =
    inventory.totals.upstreamTestFiles + inventory.totals.declarationTestFiles;
  claimed.gates = {
    ...claimed.gates,
    candidatePassed: candidateTotal,
    candidateFailed: 0,
    candidatePending: 0,
    projectSizeRequiredPassed: 4,
    projectSize: "passed",
    performance: "passed",
    pages: "passed",
  };
  claimed.bundleScenarios = claimed.bundleScenarios.map((entry) => ({
    ...entry,
    status: "passed",
  }));
  const result = evaluateCompletion({ scope: claimed, inventory });
  assert.equal(result.complete, false);
  assert.ok(result.failures.some((failure) => failure.includes("compatibility evidence is missing")));
  assert.ok(result.failures.some((failure) => failure.includes("size evidence is missing")));
  assert.ok(result.failures.some((failure) => failure.includes("performance evidence is missing")));
});

test("completion evaluator can pass only with exhaustive backing evidence", () => {
  const completed = structuredClone(scope);
  completed.packages = completed.packages.map((entry) => ({
    ...entry,
    status: entry.name === "@vue/runtime-test" ? "test-host-only" : "complete",
  }));
  const tests = [
    ...inventory.packages.flatMap((entry) => entry.testFiles),
    ...inventory.declarationTestFiles,
  ];
  completed.gates = {
    ...completed.gates,
    sourceParityRequired: inventory.totals.sourceFiles,
    sourceParitySatisfied: inventory.totals.sourceFiles,
    sourceParityMissing: 0,
    sourceParityLegacyMapped: 0,
    sourceParityLegacyNonconforming: 0,
    sourceParity: "passed",
    candidatePassed: tests.length,
    candidateFailed: 0,
    candidatePending: 0,
    projectSizeRequiredPassed: 4,
    projectSize: "passed",
    performance: "passed",
    pages: "passed",
  };
  completed.bundleScenarios = completed.bundleScenarios.map((entry) => ({
    ...entry,
    status: "passed",
  }));
  const inventoryDigest = jsonDigest(inventory);
  const scopeDigest = jsonDigest(completed);
  const sourceParity = {
    schemaVersion: 1,
    upstream: {
      revision: inventory.upstream.revision,
      tree: inventory.upstream.tree,
      clean: true,
    },
    totals: {
      upstreamFiles: inventory.totals.sourceFiles,
      satisfiedFiles: inventory.totals.sourceFiles,
      mappedFiles: inventory.totals.sourceFiles,
      declarationOnlyFiles: 0,
      missingFiles: 0,
      legacyMappedFiles: 0,
      legacyNonconformingFiles: 0,
      mappingConflicts: 0,
      physicalManyToOneMappings: 0,
      unmappedCandidateFiles: 0,
    },
    files: inventory.packages.flatMap((entry) =>
      entry.sourceFiles.map((upstreamPath) => ({
        upstreamPath,
        upstreamSha256: "f".repeat(64),
        candidatePath: candidatePathFor(upstreamPath),
        candidate: { sha256: "e".repeat(64), bytes: 1 },
        classification: "algorithm",
        handling: "mapped",
        issues: [],
      })),
    ),
    hostAdapters: [],
    legacyNonconforming: [],
    complete: true,
    failures: [],
  };
  const formatRows = [...expectedFormatKeys(inventory)].map((key) => {
    const separator = key.lastIndexOf(":");
    return {
      package: key.slice(0, separator),
      format: key.slice(separator + 1),
      status: "passed",
    };
  });
  const projectBuildConfig = {
    mode: "production",
    target: "es2022",
    minify: "oxc",
    treeShaking: true,
    defines: {
      __VUE_OPTIONS_API__: "false",
      __VUE_PROD_DEVTOOLS__: "false",
      __VUE_PROD_HYDRATION_MISMATCH_DETAILS__: "false",
      "process.env.NODE_ENV": '"production"',
    },
  };
  const graph = (modules) => ({
    sha256: sha256(Buffer.from(`${JSON.stringify(modules)}\n`)),
    modules,
  });
  const scenarioRows = completed.bundleScenarios
    .filter(({ completionRequired }) => completionRequired)
    .map(({ id, completionRequired }) => {
    const candidateModules = [
      { id: `apps/${id}/src/main.js`, renderedBytes: 1 },
      {
        id: id === "runtime-compiler-client"
          ? "packages/vuelil/production/vue.js"
          : "packages/vuelil/production/vue.runtime.js",
        renderedBytes: 2,
      },
    ];
    const upstreamModules = [
      { id: `apps/${id}/src/main.js`, renderedBytes: 1 },
      { id: "node_modules/vue/dist/vue.runtime.esm-bundler.js", renderedBytes: 2 },
    ];
    return {
      id,
      completionRequired,
      status: "passed",
      input: { importSpecifier: "vue", sha256: "a".repeat(64) },
      execution: { matching: true, deterministicChecksum: "b".repeat(64) },
      build: {
        onlyModuleResolutionChanged: true,
        reusableProductionPackage: true,
        viteDownstreamTreeShaking: true,
      },
      moduleAudit: {
        candidate: {
          noUpstreamRuntime: true,
          includesCandidateModule: true,
          noScenarioSpecificCandidateModules: true,
          scenarioSpecificCandidateModules: [],
          reusableProductionPackage: true,
          viteDownstreamTreeShaking: true,
          upstreamRuntimeModules: [],
          adapters: [{ path: "src/runtime-core/host.js" }],
          allEmittedAdapterCodeCounted: true,
          graph: graph(candidateModules),
        },
        upstream: {
          upstreamRuntimeModules: [
            "node_modules/vue/dist/vue.runtime.esm-bundler.js",
          ],
          graph: graph(upstreamModules),
        },
        passed: true,
      },
      candidate: {
        path: `artifacts/generated/project-comparison/${id}/candidate/app.js`,
        sha256: "c".repeat(64),
        sizes: { raw: 10, gzip9: 8, brotli11: 6 },
      },
      upstream: {
        path: `artifacts/generated/project-comparison/${id}/upstream/app.js`,
        sha256: "d".repeat(64),
        sizes: { raw: 12, gzip9: 10, brotli11: 8 },
      },
      comparison: { smallerBrotli11: true },
      passed: true,
    };
  });
  const performanceProtocol = {
    design: "paired isolated blocks with candidate/upstream execution order alternating every block",
    seed: "scope-test",
    samples: 21,
    warmupRuns: 1,
    bootstrapIterations: 1_000,
    nonInferiorityMarginRatio: 1.03,
    confidenceLevel: 0.95,
    confidenceBound: "one-sided upper",
    sampleAdequacyOverride: false,
  };
  const performancePaths = {
    browser: ["vue", "vue.runtime.js", "node_modules/vue/index.js"],
    compiler: ["@vue/compiler-dom", "compiler-dom.js", "node_modules/@vue/compiler-dom/index.js"],
    reactivity: ["@vue/reactivity", "reactivity.js", "node_modules/@vue/reactivity/index.js"],
    ssr: ["@vue/server-renderer", "server-renderer.js", "node_modules/@vue/server-renderer/index.js"],
  };
  const fakeFile = (path, character = "a") => ({
    path,
    sha256: character.repeat(64),
    bytes: 1,
  });
  const fakeGraph = (module) => ({
    sha256: sha256(Buffer.from(`${JSON.stringify([module])}\n`)),
    modules: [module],
  });
  const performanceWorkloads = Object.entries(performancePaths).map(
    ([category, [upstreamPackage, candidateName, upstreamPath]]) => {
      const observations = Array.from({ length: 21 }, (_, block) => ({
        block,
        order:
          block % 2 === 0
            ? ["candidate", "upstream"]
            : ["upstream", "candidate"],
        candidate: { durationMs: 1, operations: 1_000, checksum: category },
        upstream: { durationMs: 1, operations: 1_000, checksum: category },
      }));
      const candidateArtifact = fakeFile(`packages/vuelil/${candidateName}`, "b");
      const upstreamArtifact = fakeFile(upstreamPath, "c");
      return {
        id: category,
        category,
        nodeEnv: category === "compiler" ? "development" : "production",
        description: `Meaningful ${category} workload with enough detail for evidence.`,
        metric: "durationMs",
        runner: fakeFile(`benchmarks/runners/${category}.mjs`, "d"),
        candidateArtifact,
        upstreamArtifact,
        candidatePackageManifest: fakeFile("packages/vuelil/package.json", "e"),
        upstreamPackageManifest: fakeFile(
          `node_modules/${upstreamPackage}/package.json`,
          "f",
        ),
        candidateGraph: fakeGraph(candidateArtifact),
        upstreamGraph: fakeGraph(upstreamArtifact),
        environment: category === "browser"
          ? { engine: "Chromium", version: "1" }
          : { engine: "Node.js", version: "v24" },
        statistics: {
          ratio: pairedRatioStatistics(
            observations.map(({ upstream }) => upstream.durationMs),
            observations.map(({ candidate }) => candidate.durationMs),
            {
              bootstrapIterations: performanceProtocol.bootstrapIterations,
              margin: performanceProtocol.nonInferiorityMarginRatio,
              seed: `${performanceProtocol.seed}:${category}:bootstrap`,
            },
          ),
        },
        observations,
        passed: true,
      };
    },
  );
  const evidence = {
    compatibility: {
      schemaVersion: 1,
      upstream: { revision: inventory.upstream.revision },
      inventory: { sha256: inventoryDigest },
      noUpstreamImplementationImports: true,
      tests: tests.map((path) => ({ path, status: "passed" })),
      packages: inventory.packages.map(({ name }) => ({
        name,
        status: "passed",
        entrypoints: "passed",
        publicExports: "passed",
        publicExportNames: inventory.packages.find((entry) => entry.name === name)
          .publicExports,
        packageEntrypoints: inventory.packages.find((entry) => entry.name === name)
          .packageEntrypoints,
      })),
      formats: formatRows,
    },
    size: {
      schemaVersion: 2,
      upstream: { revision: inventory.upstream.revision },
      scope: { sha256: scopeDigest },
      inventory: { sha256: inventoryDigest },
      methodology:
        "Every candidate uses packages/vuelil/production; no scenario-specific candidate module path is allowed and Vite performs downstream tree shaking.",
      toolchain: {
        vite: { version: "8.2.1" },
        bundler: { name: "rolldown", version: "1.2.6" },
        minifier: { name: "oxc", version: "0.147.0" },
        config: projectBuildConfig,
        commonConfigSha256: sha256(
          Buffer.from(`${JSON.stringify(projectBuildConfig)}\n`),
        ),
      },
      scenarios: scenarioRows,
      codecs: {
        implementation: "lilscript-codec",
        schemaVersion: 1,
        scorer: { sha256: "e".repeat(64) },
        gzip9: {
          libraryVersion: "1.3.1",
          level: 9,
          mtime: 0,
        },
        brotli11: {
          libraryVersion: "1.1.0",
          quality: 11,
          lgwin: 22,
          mode: "generic",
        },
      },
      requiredScenarios: {
        ids: [...expectedBundleScenarioIds(completed, true)],
        passedIds: [...expectedBundleScenarioIds(completed, true)],
        missingIds: [],
        passed: true,
      },
      passed: true,
    },
    performance: {
      schemaVersion: 1,
      upstream: {
        version: inventory.upstream.version,
        revision: inventory.upstream.revision,
      },
      inventory: { sha256: inventoryDigest },
      implementations: {
        candidate: {
          package: "vue",
          version: "3.5.42-vuelil",
          packageManifest: fakeFile("packages/vuelil/package.json", "e"),
        },
        upstream: Object.keys(performancePaths).map((category) => ({
          package: performancePaths[category][0],
          version: "3.5.42",
          packageManifest: fakeFile(
            `node_modules/${performancePaths[category][0]}/package.json`,
            "f",
          ),
        })),
      },
      protocol: performanceProtocol,
      workloads: performanceWorkloads,
      passed: true,
    },
    pages: {
      schemaVersion: 1,
      upstream: { revision: inventory.upstream.revision },
      inventory: { sha256: inventoryDigest },
      scope: { sha256: scopeDigest },
      reports: {},
      complete: true,
      blockers: [],
    },
  };
  const evidenceDigests = {
    compatibility: jsonDigest(evidence.compatibility),
    size: jsonDigest(evidence.size),
    performance: jsonDigest(evidence.performance),
  };
  evidence.pages.reports = Object.fromEntries(
    Object.entries(evidenceDigests).map(([name, digest]) => [
      name,
      { present: true, sha256: digest },
    ]),
  );
  const result = evaluateCompletion({
    scope: completed,
    inventory,
    sourceParity,
    evidence,
    evidenceDigests,
  });
  assert.deepEqual(result, { complete: true, failures: [] });

  const libraryArtifactClaim = structuredClone(evidence);
  libraryArtifactClaim.size.scenarios.find(
    ({ id }) => id === "runtime-only-client",
  ).upstream.path = "node_modules/vue/dist/vue.runtime.esm-bundler.js";
  const rejected = evaluateCompletion({
    scope: completed,
    inventory,
    evidence: libraryArtifactClaim,
    evidenceDigests,
  });
  assert.equal(rejected.complete, false);
  assert.ok(
    rejected.failures.some((failure) =>
      failure.includes("runtime-only-client has invalid project-bundle paths"),
    ),
  );
});
