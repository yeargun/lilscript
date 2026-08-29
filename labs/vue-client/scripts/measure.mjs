import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  canonicalCodecMeasurementsForFiles,
  canonicalCodecProvenance,
} from "../../../benchmarks/codec-contract.mjs";
import {
  buildInventory,
  inventoryPath,
  projectRoot,
  scopePath,
  serializeInventory,
} from "./audit-scope.mjs";
import { sha256 } from "./check-complete.mjs";
import { staticImportedNamesSync } from "../src/reactivity/build-entry.mjs";

const defaultBuildReportPath = resolve(
  projectRoot,
  "artifacts/generated/project-comparison/reactivity-app/build-report.json",
);
const reportPath = resolve(projectRoot, "artifacts/project-size-report.json");

function artifactPath(value, label) {
  if (typeof value !== "string" || value === "") {
    throw new Error(`${label} must be a non-empty path`);
  }
  const path = resolve(projectRoot, value);
  if (!existsSync(path)) throw new Error(`${label} does not exist: ${path}`);
  if (!statSync(path).isFile()) throw new Error(`${label} is not a file: ${path}`);
  return path;
}

function artifactEvidence(path, measurement, buildArtifact) {
  const bytes = readFileSync(path);
  const digest = createHash("sha256").update(bytes).digest("hex");
  if (digest !== measurement.sha256 || bytes.length !== measurement.raw) {
    throw new Error(`${path} changed during canonical codec measurement`);
  }
  return {
    path: relative(projectRoot, path),
    sha256: digest,
    sizes: {
      raw: measurement.raw,
      gzip9: measurement.gzip,
      brotli11: measurement.brotli,
    },
    buildSha256: buildArtifact.sha256,
  };
}

function sourceManifestHash(files) {
  return sha256(
    Buffer.from(
      files
        .map(({ path, bytes, sha256: digest }) => `${path}\0${bytes}\0${digest}\n`)
        .join(""),
    ),
  );
}

function moduleGraphHash(graph) {
  return sha256(Buffer.from(`${JSON.stringify(graph.modules)}\n`));
}

function validateFileEvidence(evidence, label) {
  const path = artifactPath(evidence?.path, label);
  const bytes = readFileSync(path);
  if (evidence.bytes !== bytes.length || evidence.sha256 !== sha256(bytes)) {
    throw new Error(`${label} provenance is invalid`);
  }
  return path;
}

export function validateProjectBuildReport(report, scope) {
  if (report?.schemaVersion !== 1) {
    throw new Error("project build report schemaVersion must be 1");
  }
  if (report.upstream?.revision !== scope.upstream.revision) {
    throw new Error(
      `project build report must name upstream revision ${scope.upstream.revision}`,
    );
  }
  const scenario = scope.bundleScenarios?.find(
    ({ id }) => id === report.scenario?.id,
  );
  if (!scenario) {
    throw new Error(`project build report names unknown scenario ${report.scenario?.id}`);
  }
  if (scenario.completionRequired !== false || report.scenario.completionRequired !== false) {
    throw new Error("the reactivity project must remain a non-final diagnostic scenario");
  }
  if (
    report.toolchain?.vite?.version !== "8.2.1" ||
    report.toolchain?.bundler?.name !== "rolldown" ||
    report.toolchain?.minifier?.name !== "oxc" ||
    report.toolchain?.config?.mode !== "production" ||
    report.toolchain?.config?.target !== scope.bundleComparison?.target ||
    report.toolchain?.config?.minify !== "oxc"
  ) {
    throw new Error("project build report does not identify the pinned Vite 8/Oxc configuration");
  }
  const expectedDefines = {
    __VUE_OPTIONS_API__: "false",
    __VUE_PROD_DEVTOOLS__: "false",
    __VUE_PROD_HYDRATION_MISMATCH_DETAILS__: "false",
    "process.env.NODE_ENV": '"production"',
  };
  if (JSON.stringify(report.toolchain.config.defines) !== JSON.stringify(expectedDefines)) {
    throw new Error("project build report has unexpected production defines");
  }
  if (!Array.isArray(report.input?.files) || report.input.files.length === 0) {
    throw new Error("project build report has no hashed input files");
  }
  const inputFiles = report.input.files.map((entry) => {
    const path = artifactPath(entry.path, "project input");
    const bytes = readFileSync(path);
    const digest = sha256(bytes);
    if (entry.bytes !== bytes.length || entry.sha256 !== digest) {
      throw new Error(`project input changed after build: ${entry.path}`);
    }
    return entry;
  });
  if (sourceManifestHash(inputFiles) !== report.input.sha256) {
    throw new Error("project input source hash is invalid");
  }
  if (report.input.importSpecifier !== "vue") {
    throw new Error("project source must import the public vue specifier");
  }
  if (
    report.comparison?.sameInput !== true ||
    report.comparison?.sameBuildConfig !== true ||
    report.comparison?.onlyModuleResolutionChanged !== true ||
    report.comparison?.onlyResolutionAndBuildIntegrationChanged !== true ||
    report.comparison?.matchingExecution !== true ||
    report.comparison?.candidateNoUpstreamRuntime !== true ||
    report.comparison?.passed !== true
  ) {
    throw new Error("paired project build invariants did not pass");
  }
  const variants = {};
  for (const name of ["candidate", "upstream"]) {
    const variant = report.variants?.[name];
    const path = artifactPath(variant?.artifact?.path, `${name} project bundle`);
    const bytes = readFileSync(path);
    if (
      variant.artifact.bytes !== bytes.length ||
      variant.artifact.sha256 !== sha256(bytes) ||
      variant.execution?.expected !== true ||
      variant.execution?.checksum !== report.comparison.deterministicChecksum
    ) {
      throw new Error(`${name} project bundle provenance or execution is invalid`);
    }
    if (
      !Array.isArray(variant.moduleGraph?.modules) ||
      variant.moduleGraph.sha256 !== moduleGraphHash(variant.moduleGraph)
    ) {
      throw new Error(`${name} project module graph hash is invalid`);
    }
    variants[name] = { ...variant, artifactPath: path };
  }
  if (
    report.variants.candidate.resolution?.target !==
      "artifacts/reactivity.esm-browser.prod.js" ||
    report.variants.candidate.audit?.noUpstreamRuntime !== true ||
    report.variants.candidate.audit?.includesCandidateModule !== true ||
    report.variants.candidate.audit?.upstreamRuntimeModules?.length !== 0
  ) {
    throw new Error("candidate module graph did not prove independence from Vue runtime code");
  }
  const integration = report.variants.candidate.resolution.integration;
  const expectedNames = staticImportedNamesSync(
    readFileSync(artifactPath(report.input.source, "project application source"), "utf8"),
    report.input.importSpecifier,
  );
  if (
    integration?.kind !== "lilscript-static-export-selection" ||
    integration.applicationSource !== report.input.source ||
    JSON.stringify(integration.staticExportNames) !== JSON.stringify(expectedNames)
  ) {
    throw new Error("candidate production exports were not derived from the application source");
  }
  validateFileEvidence(integration.selectedArtifact, "selected reactivity artifact");
  validateFileEvidence(integration.reusableArtifact, "reusable reactivity artifact");
  validateFileEvidence(integration.lilscriptSource, "complete reactivity source");
  validateFileEvidence(integration.selectedLilscriptSource, "selected reactivity source");
  validateFileEvidence(integration.productionConfig, "reactivity production config");
  validateFileEvidence(integration.hostAdapter, "reactivity host adapter");
  validateFileEvidence(integration.parser?.packageJson, "static import parser package");
  const accounting = report.variants.candidate.audit.hostAdapterAccounting;
  const selectedModule = report.variants.candidate.moduleGraph.modules.find(
    ({ id }) => id === report.variants.candidate.resolution.target,
  );
  if (
    accounting?.sourceBytes !== integration.hostAdapter.bytes ||
    accounting.selectedModuleRenderedBytes !== selectedModule?.renderedBytes ||
    accounting.emittedJavaScriptBytes !== report.variants.candidate.artifact.bytes ||
    accounting.emittedJavaScriptChunks !== 1 ||
    accounting.adapterExternalized !== false ||
    accounting.allEmittedAdapterCodeCounted !== true
  ) {
    throw new Error("candidate host-adapter byte accounting is invalid");
  }
  if (
    !Array.isArray(report.variants.upstream.audit?.upstreamRuntimeModules) ||
    report.variants.upstream.audit.upstreamRuntimeModules.length === 0
  ) {
    throw new Error("upstream module graph does not contain the installed Vue runtime");
  }
  const reusableCandidate = report.diagnostics?.reusableCandidate;
  const reusableCandidatePath = artifactPath(
    reusableCandidate?.artifact?.path,
    "reusable candidate diagnostic bundle",
  );
  const reusableBytes = readFileSync(reusableCandidatePath);
  if (
    reusableCandidate.artifact.bytes !== reusableBytes.length ||
    reusableCandidate.artifact.sha256 !== sha256(reusableBytes) ||
    reusableCandidate.execution?.checksum !== report.comparison.deterministicChecksum ||
    reusableCandidate.audit?.includesCandidateModule !== true ||
    reusableCandidate.audit?.noUpstreamRuntime !== true ||
    reusableCandidate.moduleGraph?.sha256 !== moduleGraphHash(reusableCandidate.moduleGraph)
  ) {
    throw new Error("reusable candidate retained-code diagnostic is invalid");
  }
  const candidateStat = statSync(variants.candidate.artifactPath);
  const upstreamStat = statSync(variants.upstream.artifactPath);
  if (
    realpathSync(variants.candidate.artifactPath) ===
      realpathSync(variants.upstream.artifactPath) ||
    (candidateStat.dev === upstreamStat.dev && candidateStat.ino === upstreamStat.ino)
  ) {
    throw new Error("candidate and upstream resolve to the same project bundle");
  }
  return {
    scenario,
    variants,
    reusableCandidate: { ...reusableCandidate, artifactPath: reusableCandidatePath },
  };
}

function delta(candidate, upstream) {
  return {
    raw: candidate.sizes.raw - upstream.sizes.raw,
    gzip9: candidate.sizes.gzip9 - upstream.sizes.gzip9,
    brotli11: candidate.sizes.brotli11 - upstream.sizes.brotli11,
  };
}

function ratio(candidate, upstream) {
  return {
    raw: candidate.sizes.raw / upstream.sizes.raw,
    gzip9: candidate.sizes.gzip9 / upstream.sizes.gzip9,
    brotli11: candidate.sizes.brotli11 / upstream.sizes.brotli11,
  };
}

export function measure() {
  const buildReportPath = process.env.VUE_LAB_PROJECT_BUILD_REPORT
    ? resolve(process.cwd(), process.env.VUE_LAB_PROJECT_BUILD_REPORT)
    : defaultBuildReportPath;
  if (!existsSync(buildReportPath)) {
    throw new Error(
      `Project build evidence is not ready: ${buildReportPath} is missing. ` +
        "Run npm run build:project-comparison first.",
    );
  }

  const scopeBytes = readFileSync(scopePath);
  const scope = JSON.parse(scopeBytes);
  const inventoryBytes = readFileSync(inventoryPath);
  const inventory = JSON.parse(inventoryBytes);
  const liveInventory = buildInventory();
  if (serializeInventory(liveInventory) !== inventoryBytes.toString()) {
    throw new Error("compatibility/inventory.json is stale; run npm run audit:scope");
  }
  const buildReportBytes = readFileSync(buildReportPath);
  const buildReport = JSON.parse(buildReportBytes);
  const { scenario, variants, reusableCandidate } = validateProjectBuildReport(
    buildReport,
    scope,
  );
  const measurements = canonicalCodecMeasurementsForFiles(
    [
      variants.candidate.artifactPath,
      variants.upstream.artifactPath,
      reusableCandidate.artifactPath,
    ],
    "VueLil paired actual-project bundles",
  );
  const candidate = artifactEvidence(
    variants.candidate.artifactPath,
    measurements[0],
    variants.candidate.artifact,
  );
  const upstream = artifactEvidence(
    variants.upstream.artifactPath,
    measurements[1],
    variants.upstream.artifact,
  );
  const reusable = artifactEvidence(
    reusableCandidate.artifactPath,
    measurements[2],
    reusableCandidate.artifact,
  );
  const sizePassed = candidate.sizes.brotli11 < upstream.sizes.brotli11;
  const scenarioResult = {
    id: scenario.id,
    completionRequired: scenario.completionRequired,
    status: sizePassed ? "passed" : "failed",
    input: buildReport.input,
    execution: {
      deterministicChecksum: buildReport.comparison.deterministicChecksum,
      matching: true,
    },
    build: {
      commonConfigSha256: buildReport.toolchain.commonConfigSha256,
      onlyModuleResolutionChanged: true,
    },
    moduleAudit: {
      candidate: {
        ...buildReport.variants.candidate.audit,
        graph: buildReport.variants.candidate.moduleGraph,
      },
      upstream: {
        upstreamRuntimeModules:
          buildReport.variants.upstream.audit.upstreamRuntimeModules,
        graph: buildReport.variants.upstream.moduleGraph,
      },
      passed: true,
    },
    candidate,
    upstream,
    comparison: {
      deltaBytes: delta(candidate, upstream),
      ratio: ratio(candidate, upstream),
      smallerBrotli11: sizePassed,
    },
    retainedCodeDiagnosis: {
      reusableCandidate: reusable,
      selectedCandidate: candidate,
      removedBytes: {
        raw: reusable.sizes.raw - candidate.sizes.raw,
        gzip9: reusable.sizes.gzip9 - candidate.sizes.gzip9,
        brotli11: reusable.sizes.brotli11 - candidate.sizes.brotli11,
      },
      reusableModuleGraph: reusableCandidate.moduleGraph,
      selectedModuleGraph: buildReport.variants.candidate.moduleGraph,
    },
    passed: sizePassed,
  };
  const requiredIds = scope.bundleScenarios
    .filter(({ completionRequired }) => completionRequired)
    .map(({ id }) => id);
  const passedIds = scenarioResult.completionRequired && scenarioResult.passed
    ? [scenarioResult.id]
    : [];
  const missingIds = requiredIds.filter((id) => id !== scenarioResult.id);
  const requiredPassed =
    missingIds.length === 0 && requiredIds.every((id) => passedIds.includes(id));
  const report = {
    schemaVersion: 2,
    generatedBy: "scripts/measure.mjs",
    upstream: {
      version: inventory.upstream.version,
      tag: inventory.upstream.tag,
      revision: inventory.upstream.revision,
    },
    scope: {
      path: relative(projectRoot, scopePath),
      sha256: sha256(scopeBytes),
    },
    inventory: {
      path: relative(projectRoot, inventoryPath),
      sha256: sha256(inventoryBytes),
    },
    buildEvidence: {
      path: relative(projectRoot, buildReportPath),
      sha256: sha256(buildReportBytes),
    },
    methodology:
      "Paired production application builds use one unchanged source tree and identical Vite/Oxc settings. The candidate resolution derives static vue imports and compiles that LilScript-owned surface before Vite; every emitted host-adapter byte remains in the measured bundle. Library distribution files do not count toward completion.",
    objective: "Every completion-required project has a smaller candidate Brotli-11 bundle.",
    toolchain: buildReport.toolchain,
    codecs: canonicalCodecProvenance("VueLil actual-project size evidence"),
    scenarios: [scenarioResult],
    requiredScenarios: {
      ids: requiredIds,
      passedIds,
      missingIds,
      passed: requiredPassed,
    },
    passed: requiredPassed,
  };
  mkdirSync(dirname(reportPath), { recursive: true });
  writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  console.log(
    `${scenario.id}: candidate raw/gzip9/brotli11 ${candidate.sizes.raw}/${candidate.sizes.gzip9}/${candidate.sizes.brotli11}; ` +
      `upstream ${upstream.sizes.raw}/${upstream.sizes.gzip9}/${upstream.sizes.brotli11}; ` +
      `diagnostic ${sizePassed ? "passed" : "failed"}.`,
  );
  console.log(
    `Final project-size gate remains ${requiredPassed ? "passed" : "pending"}: ${missingIds.length} required scenarios are unmeasured.`,
  );
  return report;
}

function isMain() {
  return process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}

if (isMain()) measure();
