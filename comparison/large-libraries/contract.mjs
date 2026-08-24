import { createHash } from "node:crypto";

const metrics = ["raw", "gzip9", "brotli11"];
const buildStatuses = new Set([
  "passed",
  "compile-error",
  "timeout",
  "aborted",
  "not-run",
]);
const failureStatuses = new Set([
  "compile-error",
  "timeout",
  "aborted",
  "not-run",
]);
const semanticStatuses = new Set([
  "passed",
  "failed",
  "partial",
  "not-run",
  "reported-passed",
]);

export function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function record(value, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  return value;
}

function text(value, label, nullable = false) {
  if (nullable && value === null) return value;
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`${label} must be a non-empty string`);
  }
  return value;
}

function exactSha256(value, label, nullable = false) {
  if (nullable && value === null) return;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) {
    throw new Error(`${label} must be a lowercase SHA-256 digest`);
  }
}

function exactGitObject(value, label, nullable = false) {
  if (nullable && value === null) return;
  if (typeof value !== "string" || !/^[0-9a-f]{40}$/.test(value)) {
    throw new Error(`${label} must be a full lowercase Git object id`);
  }
}

function nonnegative(value, label, nullable = false) {
  if (nullable && value === null) return;
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    throw new Error(`${label} must be a non-negative finite number`);
  }
}

function integer(value, label) {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${label} must be a non-negative safe integer`);
  }
}

function sortedUnique(values, label, { allowEmpty = false } = {}) {
  if (!Array.isArray(values) || (!allowEmpty && values.length === 0)) {
    throw new Error(
      `${label} must be ${allowEmpty ? "an" : "a non-empty"} array`,
    );
  }
  if (new Set(values).size !== values.length) {
    throw new Error(`${label} must not contain duplicates`);
  }
}

function validatePolicy(policyValue, label) {
  const policy = record(policyValue, label);
  if (policy.semanticStatusRequired !== "passed") {
    throw new Error(`${label} must require passed semantics`);
  }
  const maximums = record(policy.maxRegressionBytes, `${label}.maxRegressionBytes`);
  for (const metric of metrics) integer(maximums[metric], `${label}.${metric}`);
  return policy;
}

function artifactMetrics(artifacts, label) {
  const owners = new Map();
  for (const [index, artifactValue] of artifacts.entries()) {
    const artifact = record(artifactValue, `${label}[${index}]`);
    text(artifact.id, `${label}[${index}].id`);
    if (artifact.role === "diagnostic") {
      if (artifact.gateMetrics.length !== 0) {
        throw new Error(`${artifact.id} is diagnostic but owns a gate metric`);
      }
      continue;
    }
    if (artifact.role !== undefined && artifact.role !== "gate") {
      throw new Error(`${artifact.id} has an unsupported artifact role`);
    }
    sortedUnique(artifact.gateMetrics, `${label}[${index}].gateMetrics`);
    for (const metric of artifact.gateMetrics) {
      if (!metrics.includes(metric)) {
        throw new Error(`${label}[${index}] has unknown metric ${metric}`);
      }
      if (owners.has(metric)) {
        throw new Error(
          `${label} assigns ${metric} to both ${owners.get(metric)} and ${artifact.id}`,
        );
      }
      owners.set(metric, artifact.id);
    }
    if (
      artifact.objective !== null &&
      (!metrics.includes(artifact.objective) ||
        artifact.gateMetrics.length !== 1 ||
        artifact.gateMetrics[0] !== artifact.objective)
    ) {
      throw new Error(
        `${label}[${index}] objective-specific artifacts must gate exactly their objective`,
      );
    }
  }
  if (owners.size === 0) throw new Error(`${label} does not assign a gate metric`);
}

function validateSemantic(value, label, { aggregate = false } = {}) {
  const semantic = record(value, label);
  if (!semanticStatuses.has(semantic.status)) {
    throw new Error(`${label}.status is unsupported`);
  }
  if (!aggregate && semantic.status === "partial") {
    throw new Error(`${label}.status cannot be partial at artifact scope`);
  }
  if (!["fresh", "published", "none"].includes(semantic.evidenceClass)) {
    throw new Error(`${label}.evidenceClass is unsupported`);
  }
  text(semantic.summary, `${label}.summary`);
  text(semantic.command, `${label}.command`, true);
  if (semantic.status === "passed" && semantic.evidenceClass !== "fresh") {
    throw new Error(`${label} may say passed only for fresh evidence`);
  }
  if (
    semantic.status === "reported-passed" &&
    semantic.evidenceClass !== "published"
  ) {
    throw new Error(`${label} reported evidence must be labelled published`);
  }
  if (semantic.status === "not-run" && semantic.evidenceClass !== "none") {
    throw new Error(`${label} not-run evidence must be labelled none`);
  }
  if (semantic.status === "partial" && semantic.evidenceClass !== "fresh") {
    throw new Error(`${label} partial evidence must be labelled fresh`);
  }
}

export function assertMatrix(matrixValue) {
  const matrix = record(matrixValue, "matrix");
  if (
    matrix.schemaVersion !== 1 ||
    matrix.format !== "lilscript-large-library-matrix"
  ) {
    throw new Error("matrix has an unsupported schema or format");
  }
  validatePolicy(matrix.regressionPolicy, "matrix.regressionPolicy");

  if (!Array.isArray(matrix.compilers) || matrix.compilers.length !== 2) {
    throw new Error("matrix must contain exactly baseline and checkpoint compilers");
  }
  const compilerIds = new Set();
  for (const compilerValue of matrix.compilers) {
    const compiler = record(compilerValue, "matrix compiler");
    text(compiler.id, "matrix compiler id");
    exactGitObject(compiler.revision, `${compiler.id} revision`);
    exactGitObject(compiler.tree, `${compiler.id} tree`);
    text(compiler.primarySourcePath, `${compiler.id} primary source path`);
    exactSha256(
      compiler.primarySourceSha256,
      `${compiler.id} primary source digest`,
    );
    compilerIds.add(compiler.id);
  }
  if (
    compilerIds.size !== 2 ||
    !compilerIds.has("baseline") ||
    !compilerIds.has("checkpoint")
  ) {
    throw new Error("matrix compiler ids must be baseline and checkpoint");
  }

  const codec = record(matrix.codec, "matrix.codec");
  if (codec.buildFromCompiler !== "checkpoint" || codec.schemaVersion !== 1) {
    throw new Error("matrix codec must be built from the checkpoint contract");
  }
  exactSha256(codec.sourceSha256, "matrix codec source digest");

  if (!Array.isArray(matrix.libraries) || matrix.libraries.length !== 4) {
    throw new Error("matrix must contain exactly four large libraries");
  }
  const ids = [];
  for (const libraryValue of matrix.libraries) {
    const library = record(libraryValue, "matrix library");
    ids.push(text(library.id, "matrix library id"));
    exactGitObject(library.revision, `${library.id} revision`);
    exactGitObject(library.tree, `${library.id} tree`);
    exactSha256(library.packageLockSha256, `${library.id} package lock`);
    exactSha256(library.entry?.sha256, `${library.id} entry`);
    if (!Array.isArray(library.configs) || library.configs.length === 0) {
      throw new Error(`${library.id} must pin at least one config`);
    }
    const configByPath = new Map();
    for (const config of library.configs) {
      text(config.path, `${library.id} config path`);
      exactSha256(config.sha256, `${library.id} config digest`);
      configByPath.set(config.path, config.sha256);
    }
    const build = record(library.build, `${library.id}.build`);
    text(build.program, `${library.id} build program`);
    integer(build.timeoutMs, `${library.id} build timeout`);
    if (!Array.isArray(build.cleanPaths)) {
      throw new Error(`${library.id}.build.cleanPaths must be an array`);
    }
    for (const path of build.cleanPaths) text(path, `${library.id} clean path`);
    artifactMetrics(build.artifacts, `${library.id}.build.artifacts`);
    for (const artifact of build.artifacts) {
      text(artifact.path, `${library.id} artifact path`);
      text(artifact.configPath, `${library.id} artifact config path`);
      if (!configByPath.has(artifact.configPath)) {
        throw new Error(`${library.id} artifact ${artifact.id} uses an unpinned config`);
      }
      const derivation = record(
        artifact.derivation,
        `${library.id} artifact derivation`,
      );
      if (!["identity", "prepend-first-line"].includes(derivation.kind)) {
        throw new Error(`${library.id} has an unsupported artifact derivation`);
      }
      if (derivation.kind === "prepend-first-line") {
        text(derivation.from, `${library.id} derivation source`);
      }
    }
    const semantic = record(library.semantic, `${library.id}.semantic`);
    if (!["observation", "artifact"].includes(semantic.scope)) {
      throw new Error(`${library.id}.semantic.scope is unsupported`);
    }
    text(semantic.program, `${library.id} semantic program`);
    if (!Array.isArray(semantic.args)) {
      throw new Error(`${library.id}.semantic.args must be an array`);
    }
    for (const argument of semantic.args) {
      text(argument, `${library.id} semantic argument`);
    }
    integer(semantic.timeoutMs, `${library.id} semantic timeout`);
  }
  const expected = ["jquerylil", "markedlil", "mobxlil", "solidlil"];
  if (JSON.stringify([...ids].sort()) !== JSON.stringify(expected)) {
    throw new Error(`matrix library ids must be ${expected.join(", ")}`);
  }
  return matrix;
}

function validateSizes(sizesValue, label) {
  const sizes = record(sizesValue, label);
  for (const metric of metrics) integer(sizes[metric], `${label}.${metric}`);
}

function validateDerivation(value, label) {
  const derivation = record(value, label);
  if (!["identity", "prepend-first-line"].includes(derivation.kind)) {
    throw new Error(`${label}.kind is unsupported`);
  }
  text(derivation.from, `${label}.from`, true);
}

function validateArtifact(artifactValue, library, label) {
  const artifact = record(artifactValue, label);
  text(artifact.id, `${label}.id`);
  if (!["gate", "diagnostic"].includes(artifact.role)) {
    throw new Error(`${label}.role is unsupported`);
  }
  exactSha256(artifact.sha256, `${label}.sha256`);
  exactSha256(artifact.configSha256, `${label}.configSha256`);
  validateSizes(artifact.sizes, `${label}.sizes`);
  text(artifact.relativePath, `${label}.relativePath`);
  validateDerivation(artifact.derivation, `${label}.derivation`);
  sortedUnique(artifact.gateMetrics, `${label}.gateMetrics`, {
    allowEmpty: artifact.role === "diagnostic",
  });
  for (const metric of artifact.gateMetrics) {
    if (!metrics.includes(metric)) throw new Error(`${label} has unknown metric`);
  }
  validateSemantic(artifact.semantic, `${label}.semantic`);
  if (
    artifact.role === "diagnostic" &&
    (artifact.objective !== null || artifact.gateMetrics.length !== 0)
  ) {
    throw new Error(`${label} diagnostic artifacts cannot own objectives or metrics`);
  }
  if (artifact.role === "gate") {
    const configured = library.build.artifacts.find(
      (candidate) => candidate.id === artifact.id,
    );
    if (!configured) throw new Error(`${label} is not declared in the matrix`);
    if (
      artifact.objective !== configured.objective ||
      JSON.stringify(artifact.gateMetrics) !==
        JSON.stringify(configured.gateMetrics)
    ) {
      throw new Error(`${label} does not match its configured objective lane`);
    }
    const config = library.configs.find(
      (candidate) => candidate.path === configured.configPath,
    );
    if (artifact.configSha256 !== config.sha256) {
      throw new Error(`${label} has stale objective config provenance`);
    }
  }
}

function validateObservation(observationValue, matrix, label) {
  const observation = record(observationValue, label);
  text(observation.id, `${label}.id`);
  if (!["comparison", "published", "diagnostic"].includes(observation.purpose)) {
    throw new Error(`${label}.purpose is unsupported`);
  }
  if (!buildStatuses.has(observation.status)) {
    throw new Error(`${label}.status is unsupported`);
  }
  if (Number.isNaN(Date.parse(observation.recordedAt))) {
    throw new Error(`${label}.recordedAt must be an ISO timestamp`);
  }
  const library = matrix.libraries.find((item) => item.id === observation.library);
  if (!library) throw new Error(`${label} names an unknown library`);
  const source = record(observation.source, `${label}.source`);
  if (source.revision !== library.revision || source.tree !== library.tree) {
    throw new Error(`${label} does not use the pinned library source`);
  }
  if (
    source.packageLockSha256 !== library.packageLockSha256 ||
    source.entrySha256 !== library.entry.sha256
  ) {
    throw new Error(`${label} has stale source provenance`);
  }
  exactSha256(source.configSha256, `${label}.source.configSha256`);
  text(source.configDerivation, `${label}.source.configDerivation`, true);
  if (
    observation.purpose !== "diagnostic" &&
    source.configSha256 !== library.configs[0].sha256
  ) {
    throw new Error(`${label} has stale primary-config provenance`);
  }
  if (
    observation.purpose === "diagnostic" &&
    source.configSha256 !== library.configs[0].sha256 &&
    source.configDerivation === null
  ) {
    throw new Error(`${label} changed config without recording its derivation`);
  }

  const compiler = record(observation.compiler, `${label}.compiler`);
  exactGitObject(compiler.revision, `${label}.compiler.revision`, true);
  exactGitObject(compiler.tree, `${label}.compiler.tree`, true);
  exactSha256(compiler.binarySha256, `${label}.compiler.binarySha256`, true);
  exactSha256(
    compiler.primarySourceSha256,
    `${label}.compiler.primarySourceSha256`,
    true,
  );
  text(compiler.sourceIdentity, `${label}.compiler.sourceIdentity`, true);
  if (
    observation.purpose === "published" &&
    (observation.evidenceClass !== "published" ||
      compiler.role !== "published-unknown")
  ) {
    throw new Error(`${label} published evidence has incompatible provenance`);
  }
  if (
    observation.purpose !== "published" &&
    (observation.evidenceClass !== "fresh" ||
      !["baseline", "checkpoint"].includes(compiler.role))
  ) {
    throw new Error(`${label} exact observations must be fresh and pinned`);
  }
  validateSemantic(observation.semantic, `${label}.semantic`, { aggregate: true });

  const timing = record(observation.timing, `${label}.timing`);
  nonnegative(timing.wallMs, `${label}.timing.wallMs`, true);
  nonnegative(timing.userCpuMs, `${label}.timing.userCpuMs`, true);
  nonnegative(timing.systemCpuMs, `${label}.timing.systemCpuMs`, true);
  text(timing.scope, `${label}.timing.scope`);
  if (timing.diagnosticOnly !== true) {
    throw new Error(`${label} timing must be diagnostic only`);
  }
  text(timing.unavailableReason, `${label}.timing.unavailableReason`, true);

  if (!Array.isArray(observation.artifacts)) {
    throw new Error(`${label}.artifacts must be an array`);
  }
  for (const [index, artifact] of observation.artifacts.entries()) {
    validateArtifact(artifact, library, `${label}.artifacts[${index}]`);
  }
  if (
    new Set(observation.artifacts.map((artifact) => artifact.id)).size !==
    observation.artifacts.length
  ) {
    throw new Error(`${label} has duplicate artifact ids`);
  }
  if (observation.status === "passed") {
    if (observation.artifacts.length === 0 || observation.failure !== null) {
      throw new Error(`${label} passed without artifacts or with a failure`);
    }
    artifactMetrics(observation.artifacts, `${label}.artifacts`);
  } else if (
    !failureStatuses.has(observation.status) ||
    observation.failure === null
  ) {
    throw new Error(`${label} failure status requires failure evidence`);
  }
  if (observation.failure !== null) {
    const failure = record(observation.failure, `${label}.failure`);
    if (!["prepare", "compile", "measure"].includes(failure.phase)) {
      throw new Error(`${label}.failure.phase is unsupported`);
    }
    if (
      ![
        "timeout",
        "compile-error",
        "crash",
        "aborted",
        "not-run",
      ].includes(failure.kind)
    ) {
      throw new Error(`${label}.failure.kind is unsupported`);
    }
    text(failure.diagnostic, `${label}.failure.diagnostic`);
    if (typeof failure.artifactEmitted !== "boolean") {
      throw new Error(`${label}.failure.artifactEmitted must be boolean`);
    }
  }
  if (!Array.isArray(observation.notes)) {
    throw new Error(`${label}.notes must be an array`);
  }
  for (const note of observation.notes) text(note, `${label}.note`);

  const pinnedCompiler = matrix.compilers.find(
    (item) => item.id === observation.compiler.role,
  );
  if (pinnedCompiler) {
    if (
      compiler.revision !== pinnedCompiler.revision ||
      compiler.tree !== pinnedCompiler.tree ||
      compiler.primarySourceSha256 !== pinnedCompiler.primarySourceSha256
    ) {
      throw new Error(`${label} falsely attributes an exact compiler revision`);
    }
  }
}

export function artifactForMetric(observation, metric) {
  const matches = observation.artifacts.filter(
    (artifact) =>
      artifact.role === "gate" && artifact.gateMetrics.includes(metric),
  );
  if (matches.length !== 1) {
    throw new Error(
      `${observation.id} must have exactly one gate artifact for ${metric}, found ${matches.length}`,
    );
  }
  return matches[0];
}

function exactComparisonObservation(observations, library, compilerRole) {
  const matches = observations.filter(
    (item) =>
      item.library === library &&
      item.purpose === "comparison" &&
      item.compiler.role === compilerRole,
  );
  if (matches.length > 1) {
    throw new Error(
      `${library} has ${matches.length} comparison observations for ${compilerRole}`,
    );
  }
  return matches[0];
}

export function buildComparisons(
  observations,
  matrix,
  { maxRegressionBytes = matrix.regressionPolicy.maxRegressionBytes } = {},
) {
  for (const metric of metrics) {
    integer(maxRegressionBytes[metric], `comparison override ${metric}`);
  }
  const comparisons = [];
  for (const library of matrix.libraries) {
    const before = exactComparisonObservation(
      observations,
      library.id,
      "baseline",
    );
    const after = exactComparisonObservation(
      observations,
      library.id,
      "checkpoint",
    );
    for (const metric of metrics) {
      const comparison = {
        library: library.id,
        metric,
        beforeObservation: before?.id ?? null,
        afterObservation: after?.id ?? null,
        maxRegressionBytes: maxRegressionBytes[metric],
        outcome: "ineligible",
        gatePassed: false,
        reason: "",
      };
      if (!before || !after) {
        comparison.reason = "missing exact baseline or checkpoint observation";
      } else if (
        !library.build.artifacts.some(
          (artifact) =>
            artifact.role !== "diagnostic" && artifact.gateMetrics.includes(metric),
        )
      ) {
        comparison.reason = `library has no configured ${metric} objective lane`;
      } else if (before.status !== "passed" || after.status !== "passed") {
        comparison.reason =
          "both exact revisions must emit every configured artifact";
      } else {
        const beforeArtifact = artifactForMetric(before, metric);
        const afterArtifact = artifactForMetric(after, metric);
        if (
          beforeArtifact.semantic.status !== "passed" ||
          afterArtifact.semantic.status !== "passed"
        ) {
          comparison.reason =
            `both ${metric} artifacts must independently pass fresh semantics`;
        } else {
          const beforeBytes = beforeArtifact.sizes[metric];
          const afterBytes = afterArtifact.sizes[metric];
          comparison.outcome =
            afterBytes < beforeBytes
              ? "win"
              : afterBytes === beforeBytes
                ? "tie"
                : "regression";
          comparison.gatePassed =
            afterBytes <= beforeBytes + maxRegressionBytes[metric];
          comparison.reason =
            `checkpoint ${afterBytes} bytes vs baseline ${beforeBytes} bytes; ` +
            `allowed regression ${maxRegressionBytes[metric]} bytes`;
        }
      }
      comparisons.push(comparison);
    }
  }
  return comparisons.sort(
    (left, right) =>
      left.library.localeCompare(right.library) ||
      metrics.indexOf(left.metric) - metrics.indexOf(right.metric),
  );
}

function normalized(value, key = "", omitFingerprint = true) {
  if (Array.isArray(value)) {
    const items = value.map((item) => normalized(item, "", omitFingerprint));
    if (key === "observations") {
      return items.sort((left, right) => left.id.localeCompare(right.id));
    }
    if (key === "comparisons") {
      return items.sort(
        (left, right) =>
          left.library.localeCompare(right.library) ||
          metrics.indexOf(left.metric) - metrics.indexOf(right.metric),
      );
    }
    if (key === "artifacts") {
      return items.sort((left, right) => left.id.localeCompare(right.id));
    }
    return items;
  }
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value)
        .filter((name) => !omitFingerprint || name !== "evidenceFingerprint")
        .sort()
        .map((name) => [name, normalized(value[name], name, omitFingerprint)]),
    );
  }
  return value;
}

export function evidenceFingerprint(result) {
  return sha256(JSON.stringify(normalized(result)));
}

export function canonicalResult(resultValue, matrix) {
  const result = structuredClone(resultValue);
  result.regressionPolicy ??= structuredClone(matrix.regressionPolicy);
  result.observations = [...result.observations].sort((left, right) =>
    left.id.localeCompare(right.id),
  );
  result.comparisons = buildComparisons(result.observations, matrix, {
    maxRegressionBytes: result.regressionPolicy.maxRegressionBytes,
  });
  result.evidenceFingerprint = evidenceFingerprint(result);
  return result;
}

export function stableJson(value) {
  return `${JSON.stringify(normalized(value, "", false), null, 2)}\n`;
}

export function assertResult(resultValue, matrixValue) {
  const matrix = assertMatrix(matrixValue);
  const result = record(resultValue, "result");
  if (
    result.schemaVersion !== 1 ||
    result.format !== "lilscript-large-library-observations"
  ) {
    throw new Error("result has an unsupported schema or format");
  }
  exactSha256(result.matrixSha256, "result.matrixSha256");
  const policy = validatePolicy(result.regressionPolicy, "result.regressionPolicy");
  const codec = record(result.codec, "result.codec");
  exactSha256(codec.binarySha256, "result.codec.binarySha256", true);
  exactSha256(codec.sourceSha256, "result.codec.sourceSha256");
  exactGitObject(codec.builtFromRevision, "result.codec.builtFromRevision", true);
  if (codec.schemaVersion !== 1) throw new Error("codec schema is unsupported");
  const gzip = record(codec.gzip9, "result.codec.gzip9");
  const brotli = record(codec.brotli11, "result.codec.brotli11");
  for (const key of ["encoder", "libraryVersion", "level", "mtime"]) {
    if (gzip[key] !== matrix.codec.gzip9[key]) {
      throw new Error(`result codec gzip9.${key} violates the matrix contract`);
    }
  }
  for (const key of [
    "encoder",
    "libraryVersion",
    "quality",
    "lgwin",
    "mode",
  ]) {
    if (brotli[key] !== matrix.codec.brotli11[key]) {
      throw new Error(`result codec brotli11.${key} violates the matrix contract`);
    }
  }
  if (
    codec.builtFromRevision !== null &&
    codec.builtFromRevision !==
      matrix.compilers.find((item) => item.id === matrix.codec.buildFromCompiler)
        .revision
  ) {
    throw new Error("result codec is attributed to the wrong source revision");
  }
  if (!Array.isArray(result.observations)) {
    throw new Error("result.observations must be an array");
  }
  const ids = new Set();
  for (const [index, observation] of result.observations.entries()) {
    validateObservation(observation, matrix, `observation[${index}]`);
    if (ids.has(observation.id)) {
      throw new Error(`duplicate observation id ${observation.id}`);
    }
    ids.add(observation.id);
  }
  const expectedComparisons = buildComparisons(result.observations, matrix, {
    maxRegressionBytes: policy.maxRegressionBytes,
  });
  if (
    JSON.stringify(normalized(result.comparisons, "comparisons", false)) !==
    JSON.stringify(normalized(expectedComparisons, "comparisons", false))
  ) {
    throw new Error("result comparisons are stale or not canonically ordered");
  }
  exactSha256(result.evidenceFingerprint, "result.evidenceFingerprint");
  if (result.evidenceFingerprint !== evidenceFingerprint(result)) {
    throw new Error("result evidence fingerprint is stale");
  }
  return result;
}

export const LARGE_LIBRARY_METRICS = Object.freeze([...metrics]);
