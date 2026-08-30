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

const defaultBuildReportPath = resolve(
  projectRoot,
  "artifacts/generated/project-comparison/build-report.json",
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

function validateArtifact(artifact, label) {
  if (!Array.isArray(artifact?.files) || artifact.files.length === 0) {
    throw new Error(`${label} has no deploy files`);
  }
  const files = artifact.files.map((file) => ({
    ...file,
    absolutePath: validateFileEvidence(file, `${label} file`),
  }));
  if (
    artifact.bytes !== files.reduce((sum, file) => sum + file.bytes, 0) ||
    artifact.sha256 !== sourceManifestHash(artifact.files)
  ) {
    throw new Error(`${label} deployment manifest is invalid`);
  }
  const entry = artifactPath(artifact.entry, `${label} entry`);
  if (!files.some((file) => realpathSync(file.absolutePath) === realpathSync(entry))) {
    throw new Error(`${label} entry is not part of its deploy files`);
  }
  return { ...artifact, files, entryPath: entry };
}

function expectedDefines() {
  return {
    __VUE_OPTIONS_API__: "false",
    __VUE_PROD_DEVTOOLS__: "false",
    __VUE_PROD_HYDRATION_MISMATCH_DETAILS__: "false",
    "process.env.NODE_ENV": '"production"',
  };
}

export function validateProjectBuildReport(report, scope) {
  if (report?.schemaVersion !== 2) {
    throw new Error("project build report schemaVersion must be 2");
  }
  if (report.upstream?.revision !== scope.upstream.revision) {
    throw new Error(
      `project build report must name upstream revision ${scope.upstream.revision}`,
    );
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
  if (JSON.stringify(report.toolchain.config.defines) !== JSON.stringify(expectedDefines())) {
    throw new Error("project build report has unexpected production defines");
  }

  const required = scope.bundleScenarios.filter(
    ({ completionRequired }) => completionRequired,
  );
  const expectedIds = new Set(required.map(({ id }) => id));
  if (
    !Array.isArray(report.scenarios) ||
    report.scenarios.length !== expectedIds.size ||
    report.scenarios.some(({ scenario }) => !expectedIds.has(scenario?.id))
  ) {
    throw new Error("project build report must contain exactly the four required scenarios");
  }

  return report.scenarios.map((result) => {
    const id = result.scenario.id;
    if (result.scenario.completionRequired !== true) {
      throw new Error(`${id} must be completion-required`);
    }
    if (!Array.isArray(result.input?.files) || result.input.files.length === 0) {
      throw new Error(`${id} has no hashed input files`);
    }
    const inputFiles = result.input.files.map((entry) => {
      validateFileEvidence(entry, `${id} input`);
      return entry;
    });
    if (
      sourceManifestHash(inputFiles) !== result.input.sha256 ||
      result.input.importSpecifier !== "vue" ||
      !result.input.importSpecifiers?.includes("vue")
    ) {
      throw new Error(`${id} input provenance is invalid`);
    }
    if (
      result.comparison?.sameInput !== true ||
      result.comparison?.sameBuildConfig !== true ||
      result.comparison?.onlyModuleResolutionChanged !== true ||
      result.comparison?.matchingExecution !== true ||
      result.comparison?.candidateNoUpstreamRuntime !== true ||
      result.comparison?.passed !== true
    ) {
      throw new Error(`${id} paired build invariants did not pass`);
    }

    const variants = {};
    for (const name of ["candidate", "upstream"]) {
      const variant = result.variants?.[name];
      const artifact = validateArtifact(variant?.artifact, `${id}/${name}`);
      if (
        variant.execution?.expected !== true ||
        variant.execution?.checksum !== result.comparison.deterministicChecksum ||
        variant.buildConfigSha256 !== result.variants.candidate.buildConfigSha256 ||
        !Array.isArray(variant.moduleGraph?.modules) ||
        variant.moduleGraph.sha256 !== moduleGraphHash(variant.moduleGraph)
      ) {
        throw new Error(`${id}/${name} build provenance is invalid`);
      }
      variants[name] = { ...variant, artifact };
    }
    if (variants.candidate.execution.result !== variants.upstream.execution.result) {
      throw new Error(`${id} execution results differ`);
    }
    const candidateModuleIds = variants.candidate.moduleGraph.modules.map(({ id }) => id);
    const candidateUpstream = candidateModuleIds.filter(
      (module) =>
        module.includes("node_modules/vue/") ||
        module.includes("node_modules/@vue/") ||
        module.includes("upstream/vue/"),
    );
    if (
      variants.candidate.resolution?.kind !== "vuelil-package-alias" ||
      variants.candidate.audit?.includesCandidateModule !== true ||
      variants.candidate.audit?.noUpstreamRuntime !== true ||
      variants.candidate.audit?.allEmittedAdapterCodeCounted !== true ||
      candidateUpstream.length !== 0 ||
      !candidateModuleIds.some((module) => module.startsWith("packages/vuelil/"))
    ) {
      throw new Error(`${id} candidate did not prove VueLil-only module independence`);
    }
    if (
      !Array.isArray(variants.upstream.audit?.upstreamRuntimeModules) ||
      variants.upstream.audit.upstreamRuntimeModules.length === 0
    ) {
      throw new Error(`${id} upstream graph does not contain installed Vue`);
    }
    for (const adapter of variants.candidate.audit.adapters ?? []) {
      validateFileEvidence(adapter, `${id} host adapter`);
    }
    return { ...result, variants };
  });
}

function sumSizes(files) {
  return files.reduce(
    (total, file) => ({
      raw: total.raw + file.sizes.raw,
      gzip9: total.gzip9 + file.sizes.gzip9,
      brotli11: total.brotli11 + file.sizes.brotli11,
    }),
    { raw: 0, gzip9: 0, brotli11: 0 },
  );
}

function measuredArtifact(variant, measurements) {
  const files = variant.artifact.files.map((file) => {
    const measurement = measurements.get(file.path);
    if (!measurement) throw new Error(`missing canonical measurement for ${file.path}`);
    return {
      path: file.path,
      bytes: file.bytes,
      sha256: file.sha256,
      sizes: {
        raw: measurement.raw,
        gzip9: measurement.gzip,
        brotli11: measurement.brotli,
      },
    };
  });
  return {
    path: variant.artifact.entry,
    sha256: variant.artifact.sha256,
    files,
    sizes: sumSizes(files),
    buildSha256: variant.artifact.sha256,
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
  const scenarios = validateProjectBuildReport(buildReport, scope);
  const paths = scenarios.flatMap(({ variants }) =>
    ["candidate", "upstream"].flatMap((name) =>
      variants[name].artifact.files.map(({ absolutePath }) => absolutePath),
    ),
  );
  const measured = canonicalCodecMeasurementsForFiles(
    paths,
    "VueLil paired actual-project deploy assets",
  );
  const measurements = new Map(
    measured.map((entry) => [relative(projectRoot, entry.path), entry]),
  );

  const scenarioResults = scenarios.map((result) => {
    const candidate = measuredArtifact(result.variants.candidate, measurements);
    const upstream = measuredArtifact(result.variants.upstream, measurements);
    const sizePassed = candidate.sizes.brotli11 < upstream.sizes.brotli11;
    return {
      id: result.scenario.id,
      completionRequired: true,
      status: sizePassed ? "passed" : "failed",
      input: result.input,
      execution: {
        deterministicChecksum: result.comparison.deterministicChecksum,
        matching: true,
      },
      build: {
        commonConfigSha256: buildReport.toolchain.commonConfigSha256,
        scenarioConfigSha256: result.variants.candidate.buildConfigSha256,
        onlyModuleResolutionChanged: true,
      },
      moduleAudit: {
        candidate: {
          ...result.variants.candidate.audit,
          graph: result.variants.candidate.moduleGraph,
        },
        upstream: {
          upstreamRuntimeModules:
            result.variants.upstream.audit.upstreamRuntimeModules,
          graph: result.variants.upstream.moduleGraph,
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
      passed: sizePassed,
    };
  });
  const requiredIds = scenarios.map(({ scenario }) => scenario.id);
  const passedIds = scenarioResults.filter(({ passed }) => passed).map(({ id }) => id);
  const measuredIds = new Set(scenarioResults.map(({ id }) => id));
  const missingIds = requiredIds.filter((id) => !measuredIds.has(id));
  const failedIds = scenarioResults.filter(({ passed }) => !passed).map(({ id }) => id);
  const requiredPassed = missingIds.length === 0 && failedIds.length === 0;
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
      "Each required real application is built twice from identical source and identical Vite 8/Rolldown/Oxc production settings. Only Vue package resolution changes. Candidate graphs contain only VueLil implementation modules, with retained inlined host-adapter code counted. Every emitted deployment asset is scored independently and summed; published Vue distribution sizes are not used as the gate.",
    objective: "Every completion-required project has a smaller candidate Brotli-11 deployment.",
    toolchain: buildReport.toolchain,
    codecs: canonicalCodecProvenance("VueLil actual-project size evidence"),
    scenarios: scenarioResults,
    requiredScenarios: {
      ids: requiredIds,
      passedIds,
      missingIds,
      failedIds,
      passed: requiredPassed,
    },
    passed: requiredPassed,
  };
  mkdirSync(dirname(reportPath), { recursive: true });
  writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  for (const result of scenarioResults) {
    console.log(
      `${result.id}: candidate raw/gzip9/brotli11 ${result.candidate.sizes.raw}/${result.candidate.sizes.gzip9}/${result.candidate.sizes.brotli11}; ` +
        `upstream ${result.upstream.sizes.raw}/${result.upstream.sizes.gzip9}/${result.upstream.sizes.brotli11}; ${result.status}.`,
    );
  }
  console.log(
    `Required project-size gate ${requiredPassed ? "passed" : "failed"}: ${passedIds.length}/${requiredIds.length} strict Brotli-11 wins.`,
  );
  return report;
}

function isMain() {
  return process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}

if (isMain()) measure();
