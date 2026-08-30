import { spawnSync } from "node:child_process";
import {
  existsSync,
  readFileSync,
  realpathSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { performance } from "node:perf_hooks";
import { dirname, isAbsolute, relative, resolve } from "node:path";
import { isDeepStrictEqual } from "node:util";
import { fileURLToPath } from "node:url";
import {
  buildInventory,
  inventoryPath,
  projectRoot,
  scopePath,
  serializeInventory,
} from "./audit-scope.mjs";
import { sha256 } from "./check-complete.mjs";
import {
  pairedRatioStatistics,
  performanceEvidenceFailures,
  requiredPerformanceCategories,
} from "./performance-protocol.mjs";

const defaultManifestPath = resolve(projectRoot, "benchmarks/benchmark-inputs.json");
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
  return Number.parseInt(sha256(Buffer.from(seed)).slice(0, 2), 16) & 1;
}

function runObservation(workload, variant, block, phase, seed) {
  const started = performance.now();
  const result = spawnSync(process.execPath, [workload.runner, ...workload.args], {
    cwd: workload.cwd,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    env: {
      ...process.env,
      NODE_ENV: workload.nodeEnv,
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
    environment: output.environment,
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
  if (!isDeepStrictEqual(results.candidate.environment, results.upstream.environment)) {
    throw new Error(`${workload.id} block ${block}: candidate/upstream environments differ`);
  }
  return results;
}

export function validateBenchmarkManifest(manifest, inventory, manifestPath) {
  if (manifest?.schemaVersion !== 1) throw new Error("benchmark input schemaVersion must be 1");
  if (manifest.upstream?.revision !== inventory.upstream.revision) {
    throw new Error(`benchmark inputs must name upstream revision ${inventory.upstream.revision}`);
  }
  if (
    manifest.upstream?.version !== inventory.upstream.version ||
    manifest.upstream?.tag !== inventory.upstream.tag
  ) {
    throw new Error(`benchmark inputs must pin Vue ${inventory.upstream.version}`);
  }
  if (manifest.candidateVersion !== `${inventory.upstream.version}-vuelil`) {
    throw new Error(`benchmark inputs must pin VueLil ${inventory.upstream.version}-vuelil`);
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
    if (!requiredPerformanceCategories.has(entry.category)) {
      throw new Error(`${entry.id} has unsupported category ${JSON.stringify(entry.category)}`);
    }
    if (typeof entry.description !== "string" || entry.description.length < 20) {
      throw new Error(`${entry.id} requires a meaningful workload description`);
    }
    if (!new Set(["development", "production"]).has(entry.nodeEnv)) {
      throw new Error(`${entry.id} nodeEnv must be development or production`);
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
    const candidatePackageManifest = resolveInput(
      entry.candidatePackageManifest,
      `${entry.id} candidate package manifest`,
      manifestPath,
    );
    const upstreamPackageManifest = resolveInput(
      entry.upstreamPackageManifest,
      `${entry.id} upstream package manifest`,
      manifestPath,
    );
    const candidatePackage = JSON.parse(readFileSync(candidatePackageManifest, "utf8"));
    const upstreamPackage = JSON.parse(readFileSync(upstreamPackageManifest, "utf8"));
    if (
      candidatePackage.name !== "vue" ||
      candidatePackage.version !== manifest.candidateVersion
    ) {
      throw new Error(`${entry.id} does not use the pinned VueLil package`);
    }
    const expectedUpstreamPackage = {
      browser: "vue",
      compiler: "@vue/compiler-dom",
      reactivity: "@vue/reactivity",
      ssr: "@vue/server-renderer",
    }[entry.category];
    if (
      upstreamPackage.name !== expectedUpstreamPackage ||
      upstreamPackage.version !== inventory.upstream.version
    ) {
      throw new Error(`${entry.id} does not use pinned ${expectedUpstreamPackage}@${inventory.upstream.version}`);
    }
    for (const [artifact, packageManifest, label] of [
      [candidateArtifact, candidatePackageManifest, "candidate"],
      [upstreamArtifact, upstreamPackageManifest, "upstream"],
    ]) {
      const packageRoot = dirname(packageManifest);
      const packageRelative = relative(packageRoot, artifact);
      if (packageRelative.startsWith("..") || isAbsolute(packageRelative)) {
        throw new Error(`${entry.id} ${label} artifact is outside its package`);
      }
    }
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
      candidatePackageManifest: fileEvidence(candidatePackageManifest),
      upstreamPackageManifest: fileEvidence(upstreamPackageManifest),
      candidateGraph: moduleGraphEvidence(candidateArtifact),
      upstreamGraph: moduleGraphEvidence(upstreamArtifact),
    };
    return {
      id: entry.id,
      category: entry.category,
      runner,
      candidateArtifact,
      upstreamArtifact,
      candidatePackageManifest,
      upstreamPackageManifest,
      candidatePackage,
      upstreamPackage,
      description: entry.description,
      nodeEnv: entry.nodeEnv,
      cwd,
      args: entry.args ?? [],
      files,
    };
  });
  for (const category of requiredPerformanceCategories) {
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

function moduleGraphEvidence(entryPath) {
  const pending = [entryPath];
  const seen = new Set();
  const modules = [];
  const dependencyPattern = /(?:\bfrom\s*|\bimport\s*(?:\(\s*)?|\brequire\s*\(\s*)["']([^"']+)["']/gu;
  while (pending.length > 0) {
    const path = pending.pop();
    const canonical = realpathSync(path);
    if (seen.has(canonical)) continue;
    seen.add(canonical);
    const source = readFileSync(path, "utf8");
    modules.push(fileEvidence(path));
    for (const match of source.matchAll(dependencyPattern)) {
      const specifier = match[1];
      if (!specifier.startsWith(".")) continue;
      const base = resolve(dirname(path), specifier);
      const candidates = [base, `${base}.js`, `${base}.mjs`, resolve(base, "index.js")];
      const dependency = candidates.find(
        (candidate) => existsSync(candidate) && statSync(candidate).isFile(),
      );
      if (!dependency) throw new Error(`Cannot resolve ${specifier} from ${path}`);
      pending.push(dependency);
    }
  }
  modules.sort((left, right) => left.path.localeCompare(right.path));
  return {
    sha256: sha256(Buffer.from(`${JSON.stringify(modules)}\n`)),
    modules,
  };
}

export function benchmark() {
  rmSync(reportPath, { force: true });
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
      candidatePackageManifest: fileEvidence(workload.candidatePackageManifest),
      upstreamPackageManifest: fileEvidence(workload.upstreamPackageManifest),
      candidateGraph: moduleGraphEvidence(workload.candidateArtifact),
      upstreamGraph: moduleGraphEvidence(workload.upstreamArtifact),
    };
    for (const name of Object.keys(currentFiles)) {
      if (currentFiles[name].sha256 !== workload.files[name].sha256) {
        throw new Error(`${workload.id} ${name} changed during the benchmark`);
      }
    }
    results.push({
      id: workload.id,
      category: workload.category,
      description: workload.description,
      metric: "durationMs",
      nodeEnv: workload.nodeEnv,
      ...currentFiles,
      environment: observations[0]?.candidate.environment,
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
    implementations: {
      candidate: {
        package: workloads[0].candidatePackage.name,
        version: workloads[0].candidatePackage.version,
        packageManifest: fileEvidence(workloads[0].candidatePackageManifest),
      },
      upstream: [...new Map(workloads.map((workload) => [
        workload.upstreamPackage.name,
        {
          package: workload.upstreamPackage.name,
          version: workload.upstreamPackage.version,
          packageManifest: fileEvidence(workload.upstreamPackageManifest),
        },
      ])).values()],
    },
    inputs: {
      path: relative(projectRoot, manifestPath),
      sha256: sha256(manifestBytes),
    },
    protocol: {
      design: "paired isolated blocks with candidate/upstream execution order alternating every block",
      seed,
      samples,
      warmupRuns,
      bootstrapIterations,
      nonInferiorityMarginRatio: manifest.nonInferiorityMarginRatio,
      confidenceLevel: 0.95,
      confidenceBound: "one-sided upper",
      sampleAdequacyOverride: allowSmallSamples,
      timing:
        "Each runner reports positive workload durationMs; fresh-process wall time is retained as a diagnostic.",
      outlierPolicy: "No observations removed or winsorized.",
    },
    workloads: results,
    passed: results.every((entry) => entry.passed),
  };
  writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  console.log(`Wrote ${reportPath}`);
  const integrityFailures = performanceEvidenceFailures(
    report,
    inventory,
    sha256(inventoryBytes),
    { requirePassing: false },
  );
  if (integrityFailures.length > 0) {
    throw new Error(`Invalid performance evidence: ${integrityFailures.join("; ")}`);
  }
  if (!report.passed) {
    throw new Error(
      `Performance non-inferiority failed for: ${results
        .filter((entry) => !entry.passed)
        .map((entry) => entry.id)
        .join(", ")}`,
    );
  }
  const scope = JSON.parse(readFileSync(scopePath, "utf8"));
  scope.gates.performance = "passed";
  writeFileSync(scopePath, `${JSON.stringify(scope, null, 2)}\n`);
  return report;
}

function isMain() {
  return process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}

if (isMain()) benchmark();
