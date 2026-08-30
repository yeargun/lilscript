import { isDeepStrictEqual } from "node:util";
import { createHash } from "node:crypto";

export const requiredPerformanceCategories = new Set([
  "browser",
  "compiler",
  "reactivity",
  "ssr",
]);

function finitePositive(value, label) {
  if (!Number.isFinite(value) || value <= 0) {
    throw new Error(`${label} must be a positive finite number`);
  }
  return value;
}

function randomGenerator(seed) {
  let state = 0x811c9dc5;
  for (const character of String(seed)) {
    state ^= character.codePointAt(0);
    state = Math.imul(state, 0x01000193);
  }
  state >>>= 0;
  return () => {
    state += 0x6d2b79f5;
    let value = state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return ((value ^ (value >>> 14)) >>> 0) / 0x1_0000_0000;
  };
}

function quantile(values, probability) {
  if (values.length === 0 || probability < 0 || probability > 1) {
    throw new Error("quantile needs samples and a probability in [0, 1]");
  }
  const sorted = [...values].sort((left, right) => left - right);
  const position = (sorted.length - 1) * probability;
  const lower = Math.floor(position);
  const upper = Math.ceil(position);
  const weight = position - lower;
  return sorted[lower] * (1 - weight) + sorted[upper] * weight;
}

function geometricMean(values) {
  return Math.exp(
    values.reduce((sum, value) => sum + Math.log(value), 0) / values.length,
  );
}

export function pairedRatioStatistics(
  upstream,
  candidate,
  { bootstrapIterations, margin, seed },
) {
  if (
    !Array.isArray(upstream) ||
    upstream.length === 0 ||
    upstream.length !== candidate?.length
  ) {
    throw new Error("paired benchmark arrays must have the same non-zero length");
  }
  if (!Number.isSafeInteger(bootstrapIterations) || bootstrapIterations < 1_000) {
    throw new Error("bootstrapIterations must be an integer of at least 1,000");
  }
  finitePositive(margin, "non-inferiority margin");
  const ratios = upstream.map((baseline, index) => {
    finitePositive(baseline, `upstream sample ${index}`);
    finitePositive(candidate[index], `candidate sample ${index}`);
    return candidate[index] / baseline;
  });
  const random = randomGenerator(seed);
  const bootstrap = [];
  for (let iteration = 0; iteration < bootstrapIterations; iteration += 1) {
    let logTotal = 0;
    for (let index = 0; index < ratios.length; index += 1) {
      logTotal += Math.log(ratios[Math.floor(random() * ratios.length)]);
    }
    bootstrap.push(Math.exp(logTotal / ratios.length));
  }
  const pointEstimate = geometricMean(ratios);
  const upper95 = quantile(bootstrap, 0.95);
  return {
    method: "paired geometric-mean ratio with deterministic percentile bootstrap",
    estimand: "candidate duration / upstream duration",
    pointEstimate,
    confidenceLevel: 0.95,
    confidenceBound: "one-sided upper",
    confidenceInterval: { upper95 },
    bootstrapIterations,
    nonInferiorityMarginRatio: margin,
    nonInferior: upper95 <= margin,
    ratios,
  };
}

function validFileEvidence(value) {
  return (
    typeof value?.path === "string" &&
    value.path !== "" &&
    /^[a-f0-9]{64}$/u.test(value.sha256 ?? "") &&
    Number.isSafeInteger(value.bytes) &&
    value.bytes > 0
  );
}

function validGraphEvidence(value) {
  if (!Array.isArray(value?.modules) || value.modules.length === 0) return false;
  if (!value.modules.every(validFileEvidence)) return false;
  if (new Set(value.modules.map(({ path }) => path)).size !== value.modules.length) {
    return false;
  }
  return value.sha256 === createHash("sha256")
    .update(`${JSON.stringify(value.modules)}\n`)
    .digest("hex");
}

export function performanceEvidenceFailures(
  report,
  inventory,
  inventoryDigest,
  { requirePassing = true } = {},
) {
  const failures = [];
  if (!report || typeof report !== "object") {
    return ["performance evidence is missing (artifacts/performance-report.json)"];
  }
  if (report.schemaVersion !== 1) failures.push("performance evidence schema is not 1");
  if (
    report.upstream?.version !== inventory.upstream?.version ||
    report.upstream?.revision !== inventory.upstream?.revision
  ) {
    failures.push("performance evidence does not identify Vue 3.5.42 at the audited revision");
  }
  if (report.inventory?.sha256 !== inventoryDigest) {
    failures.push("performance evidence does not identify the current inventory");
  }

  const protocol = report.protocol ?? {};
  const samples = protocol.samples;
  const margin = protocol.nonInferiorityMarginRatio;
  if (
    protocol.design !==
      "paired isolated blocks with candidate/upstream execution order alternating every block" ||
    protocol.confidenceLevel !== 0.95 ||
    protocol.confidenceBound !== "one-sided upper" ||
    !Number.isSafeInteger(samples) ||
    samples < 21 ||
    !Number.isSafeInteger(protocol.bootstrapIterations) ||
    protocol.bootstrapIterations < 1_000 ||
    !Number.isFinite(margin) ||
    margin < 1 ||
    typeof protocol.seed !== "string" ||
    protocol.seed === "" ||
    protocol.sampleAdequacyOverride === true
  ) {
    failures.push("performance evidence does not meet the minimum sampling protocol");
  }

  const candidateImplementation = report.implementations?.candidate;
  const upstreamImplementations = report.implementations?.upstream;
  if (
    candidateImplementation?.package !== "vue" ||
    candidateImplementation?.version !== `${inventory.upstream?.version}-vuelil` ||
    !validFileEvidence(candidateImplementation.packageManifest)
  ) {
    failures.push("performance evidence does not identify the current VueLil package");
  }
  const requiredUpstreamPackages = new Set([
    "vue",
    "@vue/compiler-dom",
    "@vue/reactivity",
    "@vue/server-renderer",
  ]);
  if (
    !Array.isArray(upstreamImplementations) ||
    upstreamImplementations.length !== requiredUpstreamPackages.size ||
    upstreamImplementations.some(
      (entry) =>
        !requiredUpstreamPackages.has(entry?.package) ||
        entry.version !== inventory.upstream?.version ||
        !validFileEvidence(entry.packageManifest),
    ) ||
    new Set(upstreamImplementations?.map(({ package: name }) => name)).size !==
      requiredUpstreamPackages.size
  ) {
    failures.push("performance evidence does not identify all pinned Vue 3.5.42 packages");
  }

  const workloads = Array.isArray(report.workloads) ? report.workloads : [];
  for (const category of requiredPerformanceCategories) {
    if (!workloads.some((entry) => entry.category === category)) {
      failures.push(`performance evidence is missing the ${category} workload category`);
    }
  }
  const workloadIds = new Set();
  let allPassed = workloads.length > 0;
  for (const workload of workloads) {
    const label = workload?.id || "unnamed workload";
    if (typeof workload.id !== "string" || workload.id === "" || workloadIds.has(workload.id)) {
      failures.push("performance evidence contains an invalid or duplicate workload id");
    } else {
      workloadIds.add(workload.id);
    }
    if (
      !requiredPerformanceCategories.has(workload.category) ||
      typeof workload.description !== "string" ||
      workload.description.length < 20 ||
      workload.metric !== "durationMs" ||
      !new Set(["development", "production"]).has(workload.nodeEnv)
    ) {
      failures.push(`${label} does not describe a required duration workload`);
    }
    if (
      !validFileEvidence(workload.runner) ||
      !validFileEvidence(workload.candidateArtifact) ||
      !validFileEvidence(workload.upstreamArtifact) ||
      !validFileEvidence(workload.candidatePackageManifest) ||
      !validFileEvidence(workload.upstreamPackageManifest) ||
      !validGraphEvidence(workload.candidateGraph) ||
      !validGraphEvidence(workload.upstreamGraph) ||
      workload.candidateArtifact?.path === workload.upstreamArtifact?.path ||
      !workload.candidateArtifact?.path?.startsWith("packages/vuelil/") ||
      !workload.upstreamArtifact?.path?.startsWith("node_modules/") ||
      !workload.candidateGraph?.modules?.some(
        (entry) => isDeepStrictEqual(entry, workload.candidateArtifact),
      ) ||
      !workload.upstreamGraph?.modules?.some(
        (entry) => isDeepStrictEqual(entry, workload.upstreamArtifact),
      ) ||
      workload.candidateGraph?.modules?.some(({ path }) =>
        path.startsWith("node_modules/") || path.startsWith("upstream/vue/"),
      )
    ) {
      failures.push(`${label} has invalid or non-independent artifact provenance`);
    }
    if (
      workload.category === "browser" &&
      (workload.environment?.engine !== "Chromium" ||
        typeof workload.environment?.version !== "string" ||
        workload.environment.version === "")
    ) {
      failures.push(`${label} does not identify its Chromium environment`);
    }

    const observations = Array.isArray(workload.observations)
      ? workload.observations
      : [];
    const expectedOperations = observations[0]?.candidate?.operations;
    const expectedChecksum = observations[0]?.candidate?.checksum;
    const alternating = observations.every((entry, index) => {
      if (!Array.isArray(entry.order) || entry.order.length !== 2) return false;
      if (entry.order[0] === entry.order[1]) return false;
      if (!entry.order.includes("candidate") || !entry.order.includes("upstream")) return false;
      if (entry.block !== index) return false;
      return index === 0 || entry.order[0] !== observations[index - 1].order[0];
    });
    const paired = observations.every(
      (entry) =>
        Number.isFinite(entry.candidate?.durationMs) &&
        entry.candidate.durationMs > 0 &&
        Number.isFinite(entry.upstream?.durationMs) &&
        entry.upstream.durationMs > 0 &&
        Number.isSafeInteger(entry.candidate?.operations) &&
        entry.candidate.operations > 0 &&
        entry.candidate.operations === entry.upstream?.operations &&
        entry.candidate.operations === expectedOperations &&
        isDeepStrictEqual(entry.candidate?.checksum, entry.upstream?.checksum) &&
        isDeepStrictEqual(entry.candidate?.checksum, expectedChecksum),
    );
    let expectedStatistics;
    try {
      expectedStatistics = pairedRatioStatistics(
        observations.map((entry) => entry.upstream?.durationMs),
        observations.map((entry) => entry.candidate?.durationMs),
        {
          bootstrapIterations: protocol.bootstrapIterations,
          margin,
          seed: `${protocol.seed}:${workload.id}:bootstrap`,
        },
      );
    } catch {
      expectedStatistics = null;
    }
    const statisticsMatch = isDeepStrictEqual(
      workload.statistics?.ratio,
      expectedStatistics,
    );
    const claimConsistent =
      workload.passed === (expectedStatistics?.nonInferior === true);
    const passed =
      observations.length === samples &&
      alternating &&
      paired &&
      statisticsMatch &&
      claimConsistent &&
      expectedStatistics?.nonInferior === true &&
      workload.passed === true;
    allPassed &&= passed;
    if (!passed && requirePassing) {
      failures.push(`${label} has not passed its recomputed confidence bound`);
    } else if (
      !requirePassing &&
      (observations.length !== samples ||
        !alternating ||
        !paired ||
        !statisticsMatch ||
        !claimConsistent)
    ) {
      failures.push(`${label} contains invalid paired observations or statistics`);
    }
  }
  if (report.passed !== workloads.every((workload) => workload.passed === true)) {
    failures.push("the aggregate performance result is inconsistent with its workloads");
  }
  if (requirePassing && (report.passed !== true || !allPassed)) {
    failures.push("the aggregate performance gate is not passed");
  }
  return failures;
}
