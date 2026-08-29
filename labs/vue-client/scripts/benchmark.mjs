import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { performance } from "node:perf_hooks";
import { dirname, relative, resolve } from "node:path";
import { isDeepStrictEqual } from "node:util";
import { fileURLToPath } from "node:url";
import {
  buildInventory,
  inventoryPath,
  projectRoot,
  serializeInventory,
} from "./audit-scope.mjs";
import { sha256 } from "./check-complete.mjs";

const requiredCategories = new Set(["browser", "compiler", "reactivity", "ssr"]);
const defaultManifestPath = resolve(projectRoot, "artifacts/benchmark-inputs.json");
const reportPath = resolve(projectRoot, "artifacts/performance-report.json");

function requireFile(path, label) {
  if (!existsSync(path)) throw new Error(`${label} does not exist: ${path}`);
  if (!statSync(path).isFile()) throw new Error(`${label} is not a file: ${path}`);
}

function resolveInput(value, label, manifestPath) {
  if (typeof value !== "string" || value === "") {
    throw new Error(`${label} must be a non-empty path`);
  }
  const path = resolve(dirname(manifestPath), value);
  requireFile(path, label);
  return path;
}

function finitePositive(value, label) {
  if (!Number.isFinite(value) || value <= 0) {
    throw new Error(`${label} must be a positive finite number`);
  }
  return value;
}

function deterministicBit(seed) {
  return createHash("sha256").update(seed).digest()[0] & 1;
}

function randomGenerator(seed) {
  const bytes = createHash("sha256").update(seed).digest();
  let state = bytes.readUInt32LE(0) || 0x6d2b79f5;
  return () => {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    return (state >>> 0) / 0x1_0000_0000;
  };
}

function percentile(values, probability) {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.min(sorted.length - 1, Math.floor(probability * sorted.length))];
}

function geometricMean(values) {
  return Math.exp(values.reduce((sum, value) => sum + Math.log(value), 0) / values.length);
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
  const ratios = upstream.map((baseline, index) => {
    finitePositive(baseline, `upstream sample ${index}`);
    finitePositive(candidate[index], `candidate sample ${index}`);
    return candidate[index] / baseline;
  });
  const random = randomGenerator(seed);
  const bootstrap = [];
  for (let iteration = 0; iteration < bootstrapIterations; iteration += 1) {
    const resample = [];
    for (let index = 0; index < ratios.length; index += 1) {
      resample.push(ratios[Math.floor(random() * ratios.length)]);
    }
    bootstrap.push(geometricMean(resample));
  }
  const pointEstimate = geometricMean(ratios);
  const confidenceInterval = {
    lower95: percentile(bootstrap, 0.025),
    upper95: percentile(bootstrap, 0.975),
  };
  return {
    method: "paired geometric ratio with deterministic percentile bootstrap",
    pointEstimate,
    confidenceInterval,
    nonInferiorityMarginRatio: margin,
    nonInferior: confidenceInterval.upper95 <= margin,
  };
}

function runObservation(workload, variant, block, phase, seed) {
  const started = performance.now();
  const result = spawnSync(process.execPath, [workload.runner, ...workload.args], {
    cwd: workload.cwd,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    env: {
      ...process.env,
      VUE_LAB_ARTIFACT:
        variant === "candidate" ? workload.candidateArtifact : workload.upstreamArtifact,
      VUE_LAB_BLOCK: String(block),
      VUE_LAB_PHASE: phase,
      VUE_LAB_SEED: seed,
      VUE_LAB_VARIANT: variant,
    },
  });
  const processWallMs = performance.now() - started;
  if (result.error) {
    throw new Error(`${workload.id}/${variant}: runner failed to start: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(
      `${workload.id}/${variant}: runner exited ${result.status}: ` +
        `${(result.stderr || result.stdout || "no diagnostic").trim()}`,
    );
  }
  if (result.stderr !== "") {
    throw new Error(`${workload.id}/${variant}: runner wrote stderr: ${result.stderr.trim()}`);
  }
  let output;
  try {
    output = JSON.parse(result.stdout);
  } catch (error) {
    throw new Error(`${workload.id}/${variant}: runner did not emit one JSON result: ${error.message}`);
  }
  finitePositive(output?.durationMs, `${workload.id}/${variant} durationMs`);
  if (!Number.isSafeInteger(output?.operations) || output.operations <= 0) {
    throw new Error(`${workload.id}/${variant} operations must be a positive integer`);
  }
  if (!Object.hasOwn(output, "checksum")) {
    throw new Error(`${workload.id}/${variant} must return a checksum`);
  }
  return {
    durationMs: output.durationMs,
    operations: output.operations,
    checksum: output.checksum,
    processWallMs,
  };
}

function runBlock(workload, order, block, phase, seed) {
  const results = {};
  for (const variant of order) {
    results[variant] = runObservation(workload, variant, block, phase, seed);
  }
  if (!isDeepStrictEqual(results.candidate.checksum, results.upstream.checksum)) {
    throw new Error(`${workload.id} block ${block}: candidate/upstream checksums differ`);
  }
  if (results.candidate.operations !== results.upstream.operations) {
    throw new Error(`${workload.id} block ${block}: candidate/upstream operation counts differ`);
  }
  return results;
}

export function validateBenchmarkManifest(manifest, inventory, manifestPath) {
  if (manifest?.schemaVersion !== 1) throw new Error("benchmark input schemaVersion must be 1");
  if (manifest.upstream?.revision !== inventory.upstream.revision) {
    throw new Error(`benchmark inputs must name upstream revision ${inventory.upstream.revision}`);
  }
  if (typeof manifest.seed !== "string" || manifest.seed === "") {
    throw new Error("benchmark inputs require a non-empty deterministic seed");
  }
  finitePositive(manifest.nonInferiorityMarginRatio, "nonInferiorityMarginRatio");
  if (manifest.nonInferiorityMarginRatio < 1) {
    throw new Error("nonInferiorityMarginRatio must be at least 1");
  }
  if (!Array.isArray(manifest.workloads) || manifest.workloads.length === 0) {
    throw new Error("benchmark inputs require workloads");
  }
  const ids = new Set();
  const workloads = manifest.workloads.map((entry) => {
    if (typeof entry?.id !== "string" || entry.id === "" || ids.has(entry.id)) {
      throw new Error("benchmark workload ids must be unique non-empty strings");
    }
    ids.add(entry.id);
    if (!requiredCategories.has(entry.category)) {
      throw new Error(`${entry.id} has unsupported category ${JSON.stringify(entry.category)}`);
    }
    if (
      entry.args !== undefined &&
      (!Array.isArray(entry.args) || !entry.args.every((value) => typeof value === "string"))
    ) {
      throw new Error(`${entry.id} args must be an array of strings`);
    }
    const runner = resolveInput(entry.runner, `${entry.id} runner`, manifestPath);
    const candidateArtifact = resolveInput(
      entry.candidateArtifact,
      `${entry.id} candidate artifact`,
      manifestPath,
    );
    const upstreamArtifact = resolveInput(
      entry.upstreamArtifact,
      `${entry.id} upstream artifact`,
      manifestPath,
    );
    const candidateStat = statSync(candidateArtifact);
    const upstreamStat = statSync(upstreamArtifact);
    if (
      realpathSync(candidateArtifact) === realpathSync(upstreamArtifact) ||
      (candidateStat.dev === upstreamStat.dev && candidateStat.ino === upstreamStat.ino)
    ) {
      throw new Error(`${entry.id} candidate and upstream artifacts must be distinct`);
    }
    const cwd = entry.cwd ? resolve(dirname(manifestPath), entry.cwd) : dirname(manifestPath);
    if (!existsSync(cwd) || !statSync(cwd).isDirectory()) {
      throw new Error(`${entry.id} cwd is not a directory: ${cwd}`);
    }
    const files = {
      runner: fileEvidence(runner),
      candidateArtifact: fileEvidence(candidateArtifact),
      upstreamArtifact: fileEvidence(upstreamArtifact),
    };
    return {
      id: entry.id,
      category: entry.category,
      runner,
      candidateArtifact,
      upstreamArtifact,
      cwd,
      args: entry.args ?? [],
      files,
    };
  });
  for (const category of requiredCategories) {
    if (!workloads.some((entry) => entry.category === category)) {
      throw new Error(`benchmark inputs omit the required ${category} workload category`);
    }
  }
  return workloads;
}

function fileEvidence(path) {
  const bytes = readFileSync(path);
  return { path: relative(projectRoot, path), sha256: sha256(bytes), bytes: bytes.length };
}

export function benchmark() {
  const manifestPath = process.env.VUE_LAB_BENCHMARK_MANIFEST
    ? resolve(process.cwd(), process.env.VUE_LAB_BENCHMARK_MANIFEST)
    : defaultManifestPath;
  if (!existsSync(manifestPath)) {
    throw new Error(
      `Benchmark inputs are not ready: ${manifestPath} is missing. ` +
        "Provide candidate/upstream artifacts and runners for browser, reactivity, compiler, and SSR workloads.",
    );
  }
  const inventoryBytes = readFileSync(inventoryPath);
  const inventory = JSON.parse(inventoryBytes);
  if (serializeInventory(buildInventory()) !== inventoryBytes.toString()) {
    throw new Error("compatibility/inventory.json is stale; run npm run audit:scope");
  }
  const manifestBytes = readFileSync(manifestPath);
  const manifest = JSON.parse(manifestBytes);
  const workloads = validateBenchmarkManifest(manifest, inventory, manifestPath);
  const samples = Number(process.env.VUE_LAB_BENCHMARK_SAMPLES ?? manifest.samples);
  const bootstrapIterations = Number(
    process.env.VUE_LAB_BOOTSTRAP_ITERATIONS ?? manifest.bootstrapIterations ?? 10_000,
  );
  const warmupRuns = Number(process.env.VUE_LAB_BENCHMARK_WARMUPS ?? manifest.warmupRuns ?? 2);
  const allowSmallSamples = process.env.VUE_LAB_ALLOW_SMALL_SAMPLES === "1";
  if (!Number.isSafeInteger(samples) || samples <= 0 || (!allowSmallSamples && samples < 21)) {
    throw new Error("benchmark samples must be an integer of at least 21");
  }
  if (!Number.isSafeInteger(bootstrapIterations) || bootstrapIterations < 1_000) {
    throw new Error("bootstrapIterations must be an integer of at least 1,000");
  }
  if (!Number.isSafeInteger(warmupRuns) || warmupRuns < 0) {
    throw new Error("warmupRuns must be a non-negative integer");
  }
  const seed = process.env.VUE_LAB_BENCHMARK_SEED ?? manifest.seed;
  if (typeof seed !== "string" || seed === "") {
    throw new Error("benchmark seed must be a non-empty string");
  }
  const results = [];

  for (const workload of workloads) {
    const startsWithCandidate = deterministicBit(`${seed}:${workload.id}`) === 1;
    const orderFor = (block) => {
      const candidateFirst = (block % 2 === 0) === startsWithCandidate;
      return candidateFirst ? ["candidate", "upstream"] : ["upstream", "candidate"];
    };
    for (let block = 0; block < warmupRuns; block += 1) {
      runBlock(workload, orderFor(block), block, "warmup", seed);
    }
    const observations = [];
    for (let block = 0; block < samples; block += 1) {
      const order = orderFor(block + warmupRuns);
      observations.push({
        block,
        order,
        ...runBlock(workload, order, block, "measure", seed),
      });
    }
    const statistics = {
      ratio: pairedRatioStatistics(
        observations.map((entry) => entry.upstream.durationMs),
        observations.map((entry) => entry.candidate.durationMs),
        {
          bootstrapIterations,
          margin: manifest.nonInferiorityMarginRatio,
          seed: `${seed}:${workload.id}:bootstrap`,
        },
      ),
    };
    const currentFiles = {
      runner: fileEvidence(workload.runner),
      candidateArtifact: fileEvidence(workload.candidateArtifact),
      upstreamArtifact: fileEvidence(workload.upstreamArtifact),
    };
    for (const name of Object.keys(currentFiles)) {
      if (currentFiles[name].sha256 !== workload.files[name].sha256) {
        throw new Error(`${workload.id} ${name} changed during the benchmark`);
      }
    }
    results.push({
      id: workload.id,
      category: workload.category,
      ...currentFiles,
      statistics,
      observations,
      passed: statistics.ratio.nonInferior,
    });
    console.error(`Benchmarked ${workload.id}: ${samples} deterministic alternating blocks`);
  }

  const report = {
    schemaVersion: 1,
    generatedAt: new Date().toISOString(),
    upstream: {
      version: inventory.upstream.version,
      tag: inventory.upstream.tag,
      revision: inventory.upstream.revision,
    },
    inventory: {
      path: relative(projectRoot, inventoryPath),
      sha256: sha256(inventoryBytes),
    },
    inputs: {
      path: relative(projectRoot, manifestPath),
      sha256: sha256(manifestBytes),
    },
    protocol: {
      design: "paired blocks with candidate/upstream execution order alternating every block",
      seed,
      samples,
      warmupRuns,
      bootstrapIterations,
      nonInferiorityMarginRatio: manifest.nonInferiorityMarginRatio,
      sampleAdequacyOverride: allowSmallSamples,
      timing:
        "Each runner reports positive workload durationMs; fresh-process wall time is retained as a diagnostic.",
      outlierPolicy: "No observations removed or winsorized.",
    },
    workloads: results,
    passed: results.every((entry) => entry.passed),
  };
  mkdirSync(dirname(reportPath), { recursive: true });
  writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  console.log(`Wrote ${reportPath}`);
  if (!report.passed) {
    throw new Error(
      `Performance non-inferiority failed for: ${results
        .filter((entry) => !entry.passed)
        .map((entry) => entry.id)
        .join(", ")}`,
    );
  }
  return report;
}

function isMain() {
  return process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}

if (isMain()) benchmark();
