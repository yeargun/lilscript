import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { isDeepStrictEqual } from "node:util";
import { fileURLToPath } from "node:url";
import {
  buildInventory,
  inventoryPath,
  projectRoot,
  scopePath,
  serializeInventory,
} from "./audit-scope.mjs";
import {
  buildSourceParity,
  candidatePathFor,
  serializeSourceParity,
  sourceParityPath,
} from "./audit-source-parity.mjs";
import { performanceEvidenceFailures } from "./performance-protocol.mjs";

const evidencePaths = {
  compatibility: resolve(projectRoot, "artifacts/compatibility-report.json"),
  size: resolve(projectRoot, "artifacts/project-size-report.json"),
  performance: resolve(projectRoot, "artifacts/performance-report.json"),
  pages: resolve(projectRoot, "web/evidence.json"),
};
const finalBundleScenarioIds = new Set([
  "runtime-only-client",
  "runtime-compiler-client",
  "ssr-app",
  "sfc-production-app",
]);

export function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function jsonBytes(value) {
  return Buffer.from(`${JSON.stringify(value, null, 2)}\n`);
}

function exactRows(actual, expected, key, label, failures) {
  if (!Array.isArray(actual)) {
    failures.push(`${label} is absent or is not an array`);
    return new Map();
  }
  const rows = new Map();
  for (const entry of actual) {
    const id = key(entry);
    if (typeof id !== "string" || id === "" || rows.has(id)) {
      failures.push(`${label} contains an invalid or duplicate entry`);
      continue;
    }
    rows.set(id, entry);
  }
  for (const id of expected) {
    if (!rows.has(id)) failures.push(`${label} is missing ${id}`);
  }
  for (const id of rows.keys()) {
    if (!expected.has(id)) failures.push(`${label} contains unexpected ${id}`);
  }
  return rows;
}

export function expectedFormatKeys(inventory) {
  return new Set(
    (inventory.packages ?? []).flatMap((entry) =>
      (entry.formats ?? []).map((format) => `${entry.name}:${format}`),
    ),
  );
}

export function expectedBundleScenarioIds(scope, requiredOnly = false) {
  return new Set(
    (scope?.bundleScenarios ?? [])
      .filter((entry) => !requiredOnly || entry.completionRequired === true)
      .map((entry) => entry.id),
  );
}

function checkSourceParityEvidence(report, inventory, failures) {
  if (!report) {
    failures.push("source-parity evidence is missing (compatibility/source-parity.json)");
    return;
  }
  if (report.schemaVersion !== 1) failures.push("source-parity evidence schema is not 1");
  if (
    report.upstream?.revision !== inventory.upstream?.revision ||
    report.upstream?.tree !== inventory.upstream?.tree ||
    report.upstream?.clean !== true
  ) {
    failures.push("source-parity evidence does not identify the pinned clean upstream tree");
  }
  const expectedFiles = new Set(
    (inventory.packages ?? []).flatMap((entry) => entry.sourceFiles ?? []),
  );
  const rows = exactRows(
    report.files,
    expectedFiles,
    (entry) => entry?.upstreamPath,
    "source-parity file evidence",
    failures,
  );
  const candidatePaths = new Set();
  let satisfied = 0;
  for (const [path, entry] of rows) {
    let expectedCandidate;
    try {
      expectedCandidate = candidatePathFor(path);
    } catch {
      failures.push(`${path} is outside the deterministic source-parity mapping`);
      continue;
    }
    if (entry.candidatePath !== expectedCandidate || candidatePaths.has(entry.candidatePath)) {
      failures.push(`${path} does not have a unique deterministic candidate path`);
    }
    candidatePaths.add(entry.candidatePath);
    if (!/^[a-f0-9]{64}$/u.test(entry.upstreamSha256 ?? "")) {
      failures.push(`${path} has no upstream SHA-256 provenance`);
    }
    if (entry.handling === "mapped") {
      satisfied += 1;
      if (!/^[a-f0-9]{64}$/u.test(entry.candidate?.sha256 ?? "")) {
        failures.push(`${path} has no candidate SHA-256 provenance`);
      }
    } else if (entry.handling === "declaration-only") {
      satisfied += 1;
      if (entry.classification !== "type-only") {
        failures.push(`${path} uses declaration-only handling for an algorithm file`);
      }
    }
  }
  if (report.totals?.upstreamFiles !== expectedFiles.size) {
    failures.push("source-parity required-file total does not match the source inventory");
  }
  if (
    report.totals?.satisfiedFiles !== satisfied ||
    report.totals?.mappedFiles + report.totals?.declarationOnlyFiles !== satisfied
  ) {
    failures.push("source-parity satisfied-file totals are inconsistent");
  }
  if (
    report.totals?.mappingConflicts !== 0 ||
    report.totals?.physicalManyToOneMappings !== 0 ||
    report.totals?.unmappedCandidateFiles !== 0
  ) {
    failures.push("source parity contains duplicate, many-to-one, or unmapped candidates");
  }
  if (
    !Array.isArray(report.hostAdapters) ||
    report.hostAdapters.some(
      (entry) =>
        entry.classification !== "primitive-host-adapter" ||
        !/^[a-f0-9]{64}$/u.test(entry.sha256 ?? "") ||
        entry.issues?.length !== 0,
    )
  ) {
    failures.push("source parity contains an invalid primitive host adapter");
  }
  if (
    report.complete !== true ||
    !Array.isArray(report.failures) ||
    report.failures.length !== 0 ||
    satisfied !== expectedFiles.size
  ) {
    failures.push(
      `source parity is incomplete (${satisfied}/${expectedFiles.size} upstream files satisfied)`,
    );
  }
}

function checkCompatibilityEvidence(report, inventory, inventoryDigest, failures) {
  if (!report) {
    failures.push("compatibility evidence is missing (artifacts/compatibility-report.json)");
    return;
  }
  if (report.schemaVersion !== 1) failures.push("compatibility evidence schema is not 1");
  if (report.upstream?.revision !== inventory.upstream?.revision) {
    failures.push("compatibility evidence does not identify the audited upstream revision");
  }
  if (report.inventory?.sha256 !== inventoryDigest) {
    failures.push("compatibility evidence does not identify the current inventory");
  }
  if (report.noUpstreamImplementationImports !== true) {
    failures.push("compatibility evidence has not proved upstream implementation independence");
  }

  const expectedTests = new Set([
    ...(inventory.packages ?? []).flatMap((entry) => entry.testFiles ?? []),
    ...(inventory.declarationTestFiles ?? []),
  ]);
  const tests = exactRows(
    report.tests,
    expectedTests,
    (entry) => entry?.path,
    "compatibility test evidence",
    failures,
  );
  for (const [path, entry] of tests) {
    if (entry.status !== "passed") failures.push(`${path} is not recorded as passed`);
  }

  const expectedPackages = new Set((inventory.packages ?? []).map(({ name }) => name));
  const packages = exactRows(
    report.packages,
    expectedPackages,
    (entry) => entry?.name,
    "package parity evidence",
    failures,
  );
  for (const [name, entry] of packages) {
    const audited = (inventory.packages ?? []).find((item) => item.name === name);
    const publicExportNames = Array.isArray(entry.publicExportNames)
      ? entry.publicExportNames
      : [];
    const packageEntrypoints = Array.isArray(entry.packageEntrypoints)
      ? entry.packageEntrypoints
      : [];
    if (
      entry.entrypoints !== "passed" ||
      entry.publicExports !== "passed" ||
      entry.status !== "passed" ||
      JSON.stringify(publicExportNames) !== JSON.stringify(audited?.publicExports ?? []) ||
      JSON.stringify(packageEntrypoints) !==
        JSON.stringify(audited?.packageEntrypoints ?? [])
    ) {
      failures.push(`${name} package/export parity is not passed`);
    }
  }

  const formats = exactRows(
    report.formats,
    expectedFormatKeys(inventory),
    (entry) =>
      typeof entry?.package === "string" && typeof entry?.format === "string"
        ? `${entry.package}:${entry.format}`
        : null,
    "format parity evidence",
    failures,
  );
  for (const [key, entry] of formats) {
    if (entry.status !== "passed") failures.push(`${key} format parity is not passed`);
  }
}

function validSizes(value) {
  return ["raw", "gzip9", "brotli11"].every(
    (metric) => Number.isSafeInteger(value?.[metric]) && value[metric] >= 0,
  );
}

function checkSizeEvidence(
  report,
  scope,
  inventory,
  scopeDigest,
  inventoryDigest,
  failures,
) {
  if (!report) {
    failures.push("size evidence is missing (artifacts/project-size-report.json)");
    return;
  }
  if (report.schemaVersion !== 2) failures.push("project-size evidence schema is not 2");
  if (report.upstream?.revision !== inventory.upstream?.revision) {
    failures.push("project-size evidence does not identify the audited upstream revision");
  }
  if (report.inventory?.sha256 !== inventoryDigest) {
    failures.push("project-size evidence does not identify the current inventory");
  }
  if (report.scope?.sha256 !== scopeDigest) {
    failures.push("project-size evidence does not identify the current scope");
  }
  if (
    report.codecs?.implementation !== "lilscript-codec" ||
    report.codecs?.schemaVersion !== 1 ||
    typeof report.codecs?.scorer?.sha256 !== "string" ||
    report.codecs.scorer.sha256.length !== 64 ||
    report.codecs?.gzip9?.libraryVersion !== "1.3.1" ||
    report.codecs?.gzip9?.level !== 9 ||
    report.codecs?.gzip9?.mtime !== 0 ||
    report.codecs?.brotli11?.libraryVersion !== "1.1.0" ||
    report.codecs?.brotli11?.quality !== 11 ||
    report.codecs?.brotli11?.lgwin !== 22 ||
    report.codecs?.brotli11?.mode !== "generic"
  ) {
    failures.push("project-size evidence was not produced by the canonical codec scorer");
  }
  if (
    report.toolchain?.vite?.version?.startsWith("8.") !== true ||
    report.toolchain?.bundler?.name !== "rolldown" ||
    typeof report.toolchain?.bundler?.version !== "string" ||
    report.toolchain?.minifier?.name !== "oxc" ||
    typeof report.toolchain?.minifier?.version !== "string" ||
    report.toolchain?.config?.mode !== "production" ||
    report.toolchain?.config?.target !== scope.bundleComparison?.target ||
    report.toolchain?.config?.minify !== "oxc" ||
    report.toolchain?.config?.treeShaking !== true ||
    report.toolchain?.commonConfigSha256 !==
      sha256(Buffer.from(`${JSON.stringify(report.toolchain?.config)}\n`))
  ) {
    failures.push("project-size evidence does not use the scoped Vite 8/Oxc configuration");
  }
  const expectedDefines = {
    __VUE_OPTIONS_API__: "false",
    __VUE_PROD_DEVTOOLS__: "false",
    __VUE_PROD_HYDRATION_MISMATCH_DETAILS__: "false",
    "process.env.NODE_ENV": '"production"',
  };
  if (!isDeepStrictEqual(report.toolchain?.config?.defines, expectedDefines)) {
    failures.push("project-size evidence does not use the required production defines");
  }
  if (
    typeof report.methodology !== "string" ||
    !report.methodology.includes("packages/vuelil/production") ||
    !report.methodology.includes("no scenario-specific candidate module path") ||
    !report.methodology.includes("Vite performs downstream tree shaking")
  ) {
    failures.push("project-size evidence does not state the reusable package methodology");
  }
  const requiredIds = expectedBundleScenarioIds(scope, true);
  const expectedIds = requiredIds;
  const rows = exactRows(
    report.scenarios,
    expectedIds,
    (entry) => entry?.id,
    "project-size evidence",
    failures,
  );
  for (const id of rows.keys()) {
    if (!expectedIds.has(id)) failures.push(`project-size evidence contains unknown ${id}`);
  }
  for (const [id, entry] of rows) {
    const scoped = scope.bundleScenarios.find((scenario) => scenario.id === id);
    const candidate = entry.candidate?.sizes;
    const upstream = entry.upstream?.sizes;
    const candidatePath = entry.candidate?.path;
    const upstreamPath = entry.upstream?.path;
    if (entry.completionRequired !== scoped?.completionRequired) {
      failures.push(`${id} completion-required flag differs from scope`);
    }
    if (
      !validSizes(candidate) ||
      !validSizes(upstream) ||
      upstream?.raw <= 0 ||
      upstream?.gzip9 <= 0 ||
      upstream?.brotli11 <= 0
    ) {
      failures.push(`${id} has invalid canonical raw/gzip9/Brotli-11 measurements`);
    }
    if (
      typeof candidatePath !== "string" ||
      typeof upstreamPath !== "string" ||
      candidatePath.includes("/dist/") ||
      upstreamPath.includes("/dist/") ||
      typeof entry.candidate?.sha256 !== "string" ||
      entry.candidate.sha256.length !== 64 ||
      typeof entry.upstream?.sha256 !== "string" ||
      entry.upstream.sha256.length !== 64
    ) {
      failures.push(`${id} has invalid project-bundle paths or SHA provenance`);
    }
    const candidateModules = entry.moduleAudit?.candidate?.graph?.modules;
    const upstreamModules = entry.moduleAudit?.upstream?.graph?.modules;
    const candidateGraphHash = entry.moduleAudit?.candidate?.graph?.sha256;
    const upstreamGraphHash = entry.moduleAudit?.upstream?.graph?.sha256;
    const candidateModuleIds = Array.isArray(candidateModules)
      ? candidateModules.map((module) => module?.id)
      : [];
    const upstreamModuleIds = Array.isArray(upstreamModules)
      ? upstreamModules.map((module) => module?.id)
      : [];
    const candidateUpstreamModules = candidateModuleIds.filter(
      (module) =>
        typeof module === "string" &&
        (module.includes("node_modules/vue/") ||
          module.includes("node_modules/@vue/") ||
          module.includes("upstream/vue/")),
    );
    const scenarioSpecificCandidateModules = candidateModuleIds.filter((module) => {
      if (typeof module !== "string") return false;
      const path = module.split("?", 1)[0];
      return path.startsWith("packages/vuelil/") &&
        !/^packages\/vuelil\/production\/[^/]+\.js$/u.test(path);
    });
    const upstreamVueModules = upstreamModuleIds.filter(
      (module) =>
        typeof module === "string" &&
        (module.includes("node_modules/vue/") || module.includes("node_modules/@vue/")),
    );
    if (
      entry.input?.importSpecifier !== "vue" ||
      typeof entry.input?.sha256 !== "string" ||
      entry.input.sha256.length !== 64 ||
      entry.execution?.matching !== true ||
      typeof entry.execution?.deterministicChecksum !== "string" ||
      entry.execution.deterministicChecksum.length !== 64 ||
      entry.build?.onlyModuleResolutionChanged !== true ||
      entry.build?.reusableProductionPackage !== true ||
      entry.build?.viteDownstreamTreeShaking !== true ||
      entry.moduleAudit?.candidate?.noUpstreamRuntime !== true ||
      entry.moduleAudit?.candidate?.includesCandidateModule !== true ||
      entry.moduleAudit?.candidate?.noScenarioSpecificCandidateModules !== true ||
      entry.moduleAudit?.candidate?.scenarioSpecificCandidateModules?.length !== 0 ||
      entry.moduleAudit?.candidate?.reusableProductionPackage !== true ||
      entry.moduleAudit?.candidate?.viteDownstreamTreeShaking !== true ||
      entry.moduleAudit?.candidate?.upstreamRuntimeModules?.length !== 0 ||
      entry.moduleAudit?.candidate?.allEmittedAdapterCodeCounted !== true ||
      !Array.isArray(entry.moduleAudit?.candidate?.adapters) ||
      entry.moduleAudit.candidate.adapters.length === 0 ||
      candidateUpstreamModules.length !== 0 ||
      scenarioSpecificCandidateModules.length !== 0 ||
      !candidateModuleIds.some(
        (module) =>
          typeof module === "string" &&
          module.startsWith("packages/vuelil/production/"),
      ) ||
      upstreamVueModules.length === 0 ||
      candidateGraphHash !==
        sha256(Buffer.from(`${JSON.stringify(candidateModules)}\n`)) ||
      upstreamGraphHash !==
        sha256(Buffer.from(`${JSON.stringify(upstreamModules)}\n`)) ||
      entry.moduleAudit?.passed !== true
    ) {
      failures.push(`${id} has not proved paired execution and module-graph independence`);
    }
    if (
      scoped?.completionRequired === true &&
      (candidate?.brotli11 >= upstream?.brotli11 ||
        entry.passed !== true ||
        entry.status !== "passed" ||
        entry.comparison?.smallerBrotli11 !== true)
    ) {
      failures.push(`${id} has not proved a smaller paired-project Brotli-11 bundle`);
    }
  }
  const reportedRequiredIds = report.requiredScenarios?.ids;
  const reportedPassedIds = report.requiredScenarios?.passedIds;
  if (
    !Array.isArray(reportedRequiredIds) ||
    reportedRequiredIds.length !== requiredIds.size ||
    !isDeepStrictEqual(new Set(reportedRequiredIds), requiredIds) ||
    !Array.isArray(reportedPassedIds) ||
    reportedPassedIds.length !== requiredIds.size ||
    !isDeepStrictEqual(new Set(reportedPassedIds), requiredIds) ||
    report.requiredScenarios?.missingIds?.length !== 0 ||
    report.requiredScenarios?.passed !== true ||
    report.passed !== true
  ) {
    failures.push("the aggregate required project-size gate is not passed");
  }
}

function checkPerformanceEvidence(report, inventory, inventoryDigest, failures) {
  failures.push(...performanceEvidenceFailures(report, inventory, inventoryDigest));
}

function checkPagesEvidence(
  report,
  inventory,
  inventoryDigest,
  scopeDigest,
  evidenceDigests,
  failures,
) {
  if (!report) {
    failures.push("Pages evidence is missing (web/evidence.json)");
    return;
  }
  if (report.schemaVersion !== 1) failures.push("Pages evidence schema is not 1");
  if (report.upstream?.revision !== inventory.upstream?.revision) {
    failures.push("Pages evidence does not identify the audited upstream revision");
  }
  if (report.inventory?.sha256 !== inventoryDigest || report.scope?.sha256 !== scopeDigest) {
    failures.push("Pages evidence is stale for the current scope or inventory");
  }
  for (const name of ["compatibility", "size", "performance"]) {
    if (
      report.reports?.[name]?.present !== true ||
      report.reports[name].sha256 !== evidenceDigests?.[name]
    ) {
      failures.push(`Pages evidence does not identify the current ${name} report`);
    }
  }
  if (report.complete !== true || !Array.isArray(report.blockers) || report.blockers.length !== 0) {
    failures.push("Pages evidence does not make a fully backed completion claim");
  }
}

export function evaluateCompletion({
  scope,
  inventory,
  sourceParity,
  evidence = {},
  scopeDigest = sha256(jsonBytes(scope)),
  inventoryDigest = sha256(jsonBytes(inventory)),
  evidenceDigests = {},
  requirePagesEvidence = true,
}) {
  const failures = [];
  if (scope?.schemaVersion !== 1) failures.push("scope schema is not 1");
  if (inventory?.schemaVersion !== 1) failures.push("inventory schema is not 1");
  if (scope?.upstream?.revision !== inventory?.upstream?.revision) {
    failures.push("scope and inventory upstream revisions differ");
  }
  if (inventory?.upstream?.clean !== true) failures.push("inventory was not produced from a clean checkout");
  checkSourceParityEvidence(sourceParity, inventory, failures);

  const scopedPackages = Array.isArray(scope?.packages) ? scope.packages : [];
  const inventoryPackages = new Map(
    (inventory?.packages ?? []).map((entry) => [entry.name, entry]),
  );
  if (scopedPackages.length !== inventoryPackages.size) {
    failures.push("scope and inventory package counts differ");
  }
  for (const entry of scopedPackages) {
    const audited = inventoryPackages.get(entry.name);
    if (!audited) {
      failures.push(`${entry.name} is not present in the audited inventory`);
      continue;
    }
    const acceptable = audited.private
      ? entry.status === "test-host-only" || entry.status === "complete"
      : entry.status === "complete";
    if (!acceptable) failures.push(`${entry.name} status is ${JSON.stringify(entry.status)}`);
  }
  for (const name of inventoryPackages.keys()) {
    if (!scopedPackages.some((entry) => entry.name === name)) {
      failures.push(`${name} is inventoried but absent from scope`);
    }
  }

  const inventoryFormats = new Set(
    (inventory?.packages ?? []).flatMap((entry) => entry.formats ?? []),
  );
  const scopeFormats = new Set(Array.isArray(scope?.formats) ? scope.formats : []);
  for (const format of inventoryFormats) {
    if (!scopeFormats.has(format)) failures.push(`${format} is absent from format scope`);
  }
  for (const format of scopeFormats) {
    if (!inventoryFormats.has(format)) failures.push(`${format} is not emitted by an inventoried package`);
  }

  if (
    scope?.bundleComparison?.metric !== "brotli11" ||
    scope?.bundleComparison?.codecs !== "canonical-lilscript-codec" ||
    scope?.bundleComparison?.candidateResolution !== "packages/vuelil/production" ||
    scope?.bundleComparison?.requiresMatchingExecutionChecksum !== true ||
    scope?.bundleComparison?.requiresNoCandidateUpstreamRuntime !== true ||
    scope?.bundleComparison?.requiresReusableOpenWorldCandidate !== true ||
    scope?.bundleComparison?.requiresViteDownstreamTreeShaking !== true ||
    scope?.bundleComparison?.forbidsScenarioSpecificCandidateModules !== true ||
    scope?.bundleComparison?.libraryDistributionComparisonsCountForCompletion !== false
  ) {
    failures.push("scope does not define the required paired actual-project size methodology");
  }
  const bundleScenarios = exactRows(
    scope?.bundleScenarios,
    new Set(["reactivity-app", ...finalBundleScenarioIds]),
    (entry) => entry?.id,
    "scope bundle scenarios",
    failures,
  );
  const diagnostic = bundleScenarios.get("reactivity-app");
  if (diagnostic?.completionRequired !== false) {
    failures.push("reactivity-app must remain diagnostic rather than completion-required");
  }
  for (const id of finalBundleScenarioIds) {
    const entry = bundleScenarios.get(id);
    if (entry?.completionRequired !== true) {
      failures.push(`${id} is not completion-required`);
    }
    if (entry?.status !== "passed") {
      failures.push(`${id} project-size status is ${JSON.stringify(entry?.status)}`);
    }
  }

  const expectedUpstream = inventory?.totals?.upstreamTestFiles;
  const expectedDeclarations = inventory?.totals?.declarationTestFiles;
  const expectedCandidate = expectedUpstream + expectedDeclarations;
  const gates = scope?.gates ?? {};
  const sourceParityTotals = sourceParity?.totals ?? {};
  const sourceParityGateValues = {
    sourceParityRequired: sourceParityTotals.upstreamFiles,
    sourceParitySatisfied: sourceParityTotals.satisfiedFiles,
    sourceParityMissing: sourceParityTotals.missingFiles,
    sourceParityLegacyMapped: sourceParityTotals.legacyMappedFiles,
    sourceParityLegacyNonconforming: sourceParityTotals.legacyNonconformingFiles,
  };
  for (const [gate, value] of Object.entries(sourceParityGateValues)) {
    if (gates[gate] !== value) {
      failures.push(`${gate} is ${JSON.stringify(gates[gate])}; audited value is ${value}`);
    }
  }
  if (gates.sourceParity !== "passed") {
    failures.push(`sourceParity gate is ${JSON.stringify(gates.sourceParity)}`);
  }
  if (gates.upstreamTestFiles !== expectedUpstream) {
    failures.push(`scope upstream test count is not ${expectedUpstream}`);
  }
  if (gates.declarationTestFiles !== expectedDeclarations) {
    failures.push(`scope declaration test count is not ${expectedDeclarations}`);
  }
  if (gates.candidatePassed !== expectedCandidate) {
    failures.push(`candidatePassed is ${gates.candidatePassed}; expected ${expectedCandidate}`);
  }
  if (gates.candidateFailed !== 0) failures.push(`candidateFailed is ${gates.candidateFailed}; expected 0`);
  if (gates.candidatePending !== 0) failures.push(`candidatePending is ${gates.candidatePending}; expected 0`);
  if (gates.projectSizeRequiredTotal !== finalBundleScenarioIds.size) {
    failures.push(
      `projectSizeRequiredTotal is ${gates.projectSizeRequiredTotal}; expected ${finalBundleScenarioIds.size}`,
    );
  }
  if (gates.projectSizeRequiredPassed !== finalBundleScenarioIds.size) {
    failures.push(
      `projectSizeRequiredPassed is ${gates.projectSizeRequiredPassed}; expected ${finalBundleScenarioIds.size}`,
    );
  }
  for (const gate of ["projectSize", "performance", "pages"]) {
    if (gates[gate] !== "passed") failures.push(`${gate} gate is ${JSON.stringify(gates[gate])}`);
  }

  checkCompatibilityEvidence(evidence.compatibility, inventory, inventoryDigest, failures);
  checkSizeEvidence(
    evidence.size,
    scope,
    inventory,
    scopeDigest,
    inventoryDigest,
    failures,
  );
  checkPerformanceEvidence(evidence.performance, inventory, inventoryDigest, failures);
  if (requirePagesEvidence) {
    checkPagesEvidence(
      evidence.pages,
      inventory,
      inventoryDigest,
      scopeDigest,
      evidenceDigests,
      failures,
    );
  }
  return { complete: failures.length === 0, failures };
}

function readEvidence(path) {
  if (!existsSync(path)) return { digest: null, report: null };
  const bytes = readFileSync(path);
  try {
    return { digest: sha256(bytes), report: JSON.parse(bytes) };
  } catch (error) {
    return { digest: sha256(bytes), report: { invalidJson: error.message } };
  }
}

export function checkComplete() {
  const scopeBytes = readFileSync(scopePath);
  const inventoryBytes = readFileSync(inventoryPath);
  const scope = JSON.parse(scopeBytes);
  const inventory = JSON.parse(inventoryBytes);
  const sourceParityBytes = existsSync(sourceParityPath)
    ? readFileSync(sourceParityPath)
    : null;
  const sourceParity = sourceParityBytes
    ? JSON.parse(sourceParityBytes)
    : null;
  const liveInventory = buildInventory();
  const liveSourceParity = buildSourceParity();
  const staleInventory = serializeInventory(liveInventory) !== inventoryBytes.toString();
  const staleSourceParity =
    sourceParityBytes === null ||
    serializeSourceParity(liveSourceParity) !== sourceParityBytes.toString();
  const evidenceFiles = Object.fromEntries(
    Object.entries(evidencePaths).map(([name, path]) => [name, readEvidence(path)]),
  );
  const result = evaluateCompletion({
    scope,
    inventory,
    sourceParity,
    scopeDigest: sha256(scopeBytes),
    inventoryDigest: sha256(inventoryBytes),
    evidence: Object.fromEntries(
      Object.entries(evidenceFiles).map(([name, entry]) => [name, entry.report]),
    ),
    evidenceDigests: Object.fromEntries(
      Object.entries(evidenceFiles).map(([name, entry]) => [name, entry.digest]),
    ),
  });
  if (staleInventory) result.failures.unshift("compatibility/inventory.json is stale; run npm run audit:scope");
  if (staleSourceParity) {
    result.failures.unshift(
      "compatibility/source-parity.json is stale; run npm run audit:source-parity",
    );
  }
  result.complete = result.failures.length === 0;
  return result;
}

function isMain() {
  return process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}

if (isMain()) {
  const result = checkComplete();
  if (!result.complete) {
    console.error("VueLil completion gate failed closed:");
    for (const failure of result.failures) console.error(`- ${failure}`);
    process.exitCode = 1;
  } else {
    console.log("VueLil completion gate passed with complete machine-readable evidence.");
  }
}
