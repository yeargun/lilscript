import { fork, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import {
  brotliCompressSync,
  constants as zlibConstants,
  gzipSync,
} from "node:zlib";
import {
  benchmarkRoot,
  metadataPath,
  repositoryRoot,
  upstreamRoot,
} from "./paths.mjs";
import { hashString, measurementKey, shuffled } from "./measurement-utils.mjs";

const CPU_WORKLOADS = [
  "01_run1k",
  "02_replace1k",
  "03_update10th1k_x16",
  "04_select1k",
  "05_swap1k",
  "06_remove-one-1k",
  "07_create10k",
  "08_create1k-after1k_x2",
  "09_clear1k_x8",
];
const MEMORY_WORKLOADS = [
  "21_ready-memory",
  "22_run-memory",
  "25_run-clear-memory",
];
const COLD_WORKLOAD = "cold-page-load";
const chromePath = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const webdriverRoot = resolve(upstreamRoot, "webdriver-ts");
const artifactsRoot = resolve(benchmarkRoot, "artifacts");
const traceDirectory = resolve(webdriverRoot, "traces", "lilscript-randomized");
const metadata = JSON.parse(readFileSync(metadataPath, "utf8"));
function argumentValue(name) {
  const index = process.argv.indexOf(name);
  return index < 0 ? null : process.argv[index + 1];
}
const requestedPhase = process.argv.includes("--phase")
  ? argumentValue("--phase")
  : "all";
const requestedBlocks = process.argv.includes("--blocks")
  ? Number(argumentValue("--blocks"))
  : metadata.measurement.blocks;
const requestedFramework = argumentValue("--only");
const checkpointPath = resolve(
  artifactsRoot,
  argumentValue("--checkpoint") ?? "checkpoint.json",
);
const checkpointTemporaryPath = `${checkpointPath}.tmp`;
const requestedFrameworks = requestedFramework
  ? requestedFramework.split(",").map((value) => value.trim()).filter(Boolean)
  : [];
const selectedFrameworkMetadata = requestedFrameworks.length > 0
  ? metadata.frameworks.filter(({ path }) =>
      requestedFrameworks.some((requested) =>
        [path, path.split("/").at(-1)].includes(requested),
      ),
    )
  : metadata.frameworks;

if (!existsSync(chromePath)) throw new Error(`Google Chrome not found at ${chromePath}`);
if (!Number.isSafeInteger(requestedBlocks) || requestedBlocks < 1) {
  throw new Error("--blocks must be a positive integer");
}
if (!["all", "size", "cpu", "memory", "cold"].includes(requestedPhase)) {
  throw new Error(`Unknown phase ${requestedPhase}`);
}
if (selectedFrameworkMetadata.length === 0) {
  throw new Error(`No framework matches --only ${requestedFramework}`);
}

mkdirSync(artifactsRoot, { recursive: true });
mkdirSync(traceDirectory, { recursive: true });

function commandOutput(command, args, cwd = repositoryRoot) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(result.stderr.trim() || `${command} failed`);
  }
  return result.stdout.trim();
}

function fileHash(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

const compilerPath = resolve(repositoryRoot, "target", "release", "lilscript");
const initialState = {
  schemaVersion: 1,
  provenance: {
    generatedAt: new Date().toISOString(),
    upstreamRepository: metadata.repository,
    upstreamCommit: metadata.commit,
    lilscriptCommit: commandOutput("git", ["rev-parse", "HEAD"]),
    compilerSha256: existsSync(compilerPath) ? fileHash(compilerPath) : null,
    node: process.version,
    chrome: commandOutput(chromePath, ["--version"]),
    platform: `${os.platform()} ${os.release()} ${os.arch()}`,
    cpu: os.cpus()[0]?.model ?? "unknown",
    logicalCpuCount: os.cpus().length,
    totalMemoryBytes: os.totalmem(),
  },
  configuration: {
    runner: "upstream Playwright implementation",
    browser: "installed Google Chrome",
    blocks: requestedBlocks,
    seed: metadata.measurement.seed,
    cpuThrottling: metadata.measurement.cpuThrottling,
    warmups: "upstream workload-specific warmupCount",
    memoryCollection: "forceGC + performance.measureUserAgentSpecificMemory",
    coldLoad: "fresh browser context with an empty cache",
    brotliQuality: metadata.measurement.brotliQuality,
    frameworkFilter: requestedFramework,
  },
  sizes: {},
  samples: [],
};

const state = existsSync(checkpointPath)
  ? JSON.parse(readFileSync(checkpointPath, "utf8"))
  : initialState;
if (state.configuration.blocks !== requestedBlocks) {
  throw new Error(
    `Checkpoint uses ${state.configuration.blocks} blocks; requested ${requestedBlocks}`,
  );
}

function save() {
  writeFileSync(checkpointTemporaryPath, `${JSON.stringify(state, null, 2)}\n`);
  renameSync(checkpointTemporaryPath, checkpointPath);
}

const benchmarkOptions = {
  port: 8080,
  host: "localhost",
  browser: "chrome",
  remoteDebuggingPort: 9999,
  chromePort: 9998,
  headless: true,
  chromeBinaryPath: chromePath,
  numIterationsForCPUBenchmarks: 1,
  numIterationsForMemBenchmarks: 1,
  numIterationsForStartupBenchmark: 1,
  numIterationsForSizeBenchmark: 1,
  batchSize: 1,
  resultsDirectory: resolve(webdriverRoot, "results"),
  tracesDirectory: traceDirectory,
  allowThrottling: metadata.measurement.cpuThrottling,
  puppeteerSleep: 0,
};

const commonModule = await import(
  pathToFileURL(resolve(webdriverRoot, "dist", "common.js"))
);
const wantedPaths = new Set(
  selectedFrameworkMetadata.map((framework) => framework.path),
);
const frameworks = await commonModule.initializeFrameworks(
  benchmarkOptions,
  (path) => wantedPaths.has(path),
);
if (frameworks.length !== selectedFrameworkMetadata.length) {
  throw new Error(
    `Expected ${selectedFrameworkMetadata.length} frameworks, found ${frameworks.length}`,
  );
}

const frameworkByPath = new Map(
  frameworks.map((framework) => [
    `${framework.keyed ? "keyed" : "non-keyed"}/${framework.name}`,
    framework,
  ]),
);
const orderedFrameworks = selectedFrameworkMetadata.map(({ path }) => {
  const framework = frameworkByPath.get(path);
  if (!framework) throw new Error(`Server did not expose ${path}`);
  return framework;
});

function collectJavaScriptFiles(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...collectJavaScriptFiles(path));
    else if (entry.isFile() && /\.(?:js|mjs)$/.test(entry.name)) files.push(path);
  }
  return files.sort();
}

function directBundleSize(framework) {
  const directory = resolve(
    upstreamRoot,
    "frameworks",
    framework.keyed ? "keyed" : "non-keyed",
    framework.name,
    "dist",
  );
  const files = collectJavaScriptFiles(directory);
  const quality = metadata.measurement.brotliQuality;
  const measurements = files.map((path) => {
    const content = readFileSync(path);
    return {
      file: path.slice(directory.length + 1),
      rawBytes: content.length,
      brotliBytes: brotliCompressSync(content, {
        params: { [zlibConstants.BROTLI_PARAM_QUALITY]: quality },
      }).length,
      gzipBytes: gzipSync(content, { level: 9 }).length,
      sha256: createHash("sha256").update(content).digest("hex"),
    };
  });
  return {
    files: measurements,
    rawBytes: measurements.reduce((sum, item) => sum + item.rawBytes, 0),
    brotliBytes: measurements.reduce((sum, item) => sum + item.brotliBytes, 0),
    gzipBytes: measurements.reduce((sum, item) => sum + item.gzipBytes, 0),
  };
}

const measurementFingerprint = createHash("sha256")
  .update(
    JSON.stringify({
      upstreamCommit: metadata.commit,
      chrome: initialState.provenance.chrome,
      blocks: requestedBlocks,
      seed: metadata.measurement.seed,
      cpuThrottling: metadata.measurement.cpuThrottling,
      bundles: orderedFrameworks.map((framework) => ({
        framework: framework.fullNameWithKeyedAndVersion,
        files: directBundleSize(framework).files.map(({ file, sha256 }) => ({
          file,
          sha256,
        })),
      })),
    }),
  )
  .digest("hex");

if (state.configuration.measurementFingerprint !== measurementFingerprint) {
  if (state.samples.length > 0) {
    console.log(
      `Discarding ${state.samples.length} samples because a measured bundle or environment changed`,
    );
  }
  state.samples = [];
  delete state.completedAt;
  state.configuration.measurementFingerprint = measurementFingerprint;
  save();
}

function loadPlaywright() {
  const require = createRequire(resolve(webdriverRoot, "package.json"));
  return require("playwright");
}

async function measureSizes() {
  const { chromium } = loadPlaywright();
  const browser = await chromium.launch({
    headless: true,
    executablePath: chromePath,
    args: ["--headless=new", "--window-size=1000,800"],
  });
  try {
    for (const framework of orderedFrameworks) {
      const bundle = directBundleSize(framework);
      const previous = state.sizes[framework.fullNameWithKeyedAndVersion];
      if (
        previous &&
        JSON.stringify(previous.bundle.files.map((file) => file.sha256)) ===
          JSON.stringify(bundle.files.map((file) => file.sha256))
      ) {
        continue;
      }
      const context = await browser.newContext();
      const page = await context.newPage();
      const errors = [];
      page.on("pageerror", (error) => errors.push(error.stack ?? error.message));
      await fetch("http://localhost:8080/enableCompression");
      await page.goto(`http://localhost:8080/${framework.uri}/index.html`, {
        waitUntil: "networkidle",
      });
      await page.locator("#run").click();
      await page.locator("tbody > tr").nth(999).waitFor({ state: "attached" });
      if (errors.length > 0) throw new Error(`${framework.name}: ${errors.join("\n")}`);
      const paint = await page.evaluate(() =>
        performance.getEntriesByType("paint").map((entry) => ({
          name: entry.name,
          startTime: entry.startTime,
        })),
      );
      const pageSize = await fetch("http://localhost:8080/sizeInfo").then((response) =>
        response.json(),
      );
      await fetch("http://localhost:8080/disableCompression");
      state.sizes[framework.fullNameWithKeyedAndVersion] = {
        pageRawBytes: pageSize.size_uncompressed,
        pageBrotliBytes: pageSize.size_compressed,
        firstPaintMs: paint.find((entry) => entry.name === "first-paint")?.startTime ?? null,
        bundle,
      };
      await context.close();
      save();
      console.log(
        `size ${framework.fullNameWithKeyedAndVersion}: ` +
          `${pageSize.size_compressed} B page Brotli, ` +
          `${state.sizes[framework.fullNameWithKeyedAndVersion].bundle.brotliBytes} B JS Brotli`,
      );
    }
  } finally {
    await fetch("http://localhost:8080/disableCompression").catch(() => {});
    await browser.close();
  }
}

const childConfig = {
  ...commonModule.config,
  WRITE_RESULTS: false,
  EXIT_ON_ERROR: true,
  LOG_PROGRESS: false,
  LOG_DETAILS: false,
  LOG_DEBUG: false,
  BENCHMARK_RUNNER: "playwright",
};

function runOfficialPlaywright(framework, workload) {
  return new Promise((resolvePromise, rejectPromise) => {
    const modulePath = resolve(
      webdriverRoot,
      "dist",
      "forkedBenchmarkRunnerPlaywright.js",
    );
    const child = fork(modulePath, [], {
      cwd: webdriverRoot,
      silent: true,
    });
    let log = "";
    child.stdout.on("data", (chunk) => {
      log = `${log}${chunk}`.slice(-200_000);
    });
    child.stderr.on("data", (chunk) => {
      log = `${log}${chunk}`.slice(-200_000);
    });
    const timeout = setTimeout(() => {
      child.kill("SIGTERM");
      rejectPromise(new Error(`${framework.name}/${workload} timed out\n${log}`));
    }, 300_000);
    child.once("message", (message) => {
      clearTimeout(timeout);
      if (message.error) {
        rejectPromise(new Error(`${framework.name}/${workload}: ${message.error}\n${log}`));
      } else {
        const result = Array.isArray(message.result) ? message.result[0] : message.result;
        resolvePromise({ result, warnings: message.warnings ?? [] });
      }
    });
    child.once("error", (error) => {
      clearTimeout(timeout);
      rejectPromise(error);
    });
    child.send({
      config: childConfig,
      framework,
      benchmarkId: workload,
      benchmarkOptions: { ...benchmarkOptions, batchSize: 1 },
    });
  });
}

async function measureOfficialPhase(phase, workloads) {
  const completed = new Set(
    state.samples.map((sample) =>
      measurementKey(sample.phase, sample.block, sample.workload, sample.framework),
    ),
  );
  let completedCount = state.samples.filter((sample) => sample.phase === phase).length;
  const targetCount = requestedBlocks * workloads.length * orderedFrameworks.length;
  for (let block = 0; block < requestedBlocks; block += 1) {
    const workloadOrder = shuffled(
      workloads,
      metadata.measurement.seed ^ hashString(`${phase}:${block}:workloads`),
    );
    for (const workload of workloadOrder) {
      const frameworkOrder = shuffled(
        orderedFrameworks,
        metadata.measurement.seed ^ hashString(`${phase}:${block}:${workload}`),
      );
      for (const framework of frameworkOrder) {
        const key = measurementKey(
          phase,
          block,
          workload,
          framework.fullNameWithKeyedAndVersion,
        );
        if (completed.has(key)) continue;
        const startedAt = Date.now();
        const { result, warnings } = await runOfficialPlaywright(framework, workload);
        state.samples.push({
          phase,
          block,
          workload,
          framework: framework.fullNameWithKeyedAndVersion,
          elapsedMs: Date.now() - startedAt,
          value: result,
          warnings,
        });
        completed.add(key);
        completedCount += 1;
        save();
        const value = typeof result === "number" ? result : result.total;
        console.log(
          `${phase} ${completedCount}/${targetCount} block ${block + 1}: ` +
            `${framework.name} ${workload} = ${value.toFixed(3)}`,
        );
      }
    }
  }
}

async function measureColdLoads() {
  const completed = new Set(
    state.samples.map((sample) =>
      measurementKey(sample.phase, sample.block, sample.workload, sample.framework),
    ),
  );
  const { chromium } = loadPlaywright();
  const browser = await chromium.launch({
    headless: true,
    executablePath: chromePath,
    args: ["--headless=new", "--window-size=1000,800"],
  });
  let completedCount = state.samples.filter((sample) => sample.phase === "cold").length;
  const targetCount = requestedBlocks * orderedFrameworks.length;
  try {
    for (let block = 0; block < requestedBlocks; block += 1) {
      const frameworkOrder = shuffled(
        orderedFrameworks,
        metadata.measurement.seed ^ hashString(`cold:${block}`),
      );
      for (const framework of frameworkOrder) {
        const key = measurementKey(
          "cold",
          block,
          COLD_WORKLOAD,
          framework.fullNameWithKeyedAndVersion,
        );
        if (completed.has(key)) continue;
        const context = await browser.newContext();
        const page = await context.newPage();
        const errors = [];
        page.on("pageerror", (error) => errors.push(error.stack ?? error.message));
        const startedAt = Date.now();
        await page.goto(`http://localhost:8080/${framework.uri}/index.html`, {
          waitUntil: "networkidle",
        });
        await page.locator("#run").waitFor({ state: "attached" });
        if (errors.length > 0) throw new Error(`${framework.name}: ${errors.join("\n")}`);
        const value = await page.evaluate(() => {
          const navigation = performance.getEntriesByType("navigation")[0];
          const firstPaint = performance
            .getEntriesByType("paint")
            .find((entry) => entry.name === "first-paint");
          return {
            domContentLoadedMs: navigation.domContentLoadedEventEnd,
            loadMs: navigation.loadEventEnd,
            firstPaintMs: firstPaint?.startTime ?? null,
          };
        });
        state.samples.push({
          phase: "cold",
          block,
          workload: COLD_WORKLOAD,
          framework: framework.fullNameWithKeyedAndVersion,
          elapsedMs: Date.now() - startedAt,
          value,
          warnings: [],
        });
        await context.close();
        completed.add(key);
        completedCount += 1;
        save();
        console.log(
          `cold ${completedCount}/${targetCount} block ${block + 1}: ` +
            `${framework.name} load = ${value.loadMs.toFixed(3)}`,
        );
      }
    }
  } finally {
    await browser.close();
  }
}

if (requestedPhase === "all" || requestedPhase === "size") await measureSizes();
if (requestedPhase === "all" || requestedPhase === "cpu") {
  await measureOfficialPhase("cpu", CPU_WORKLOADS);
}
if (requestedPhase === "all" || requestedPhase === "memory") {
  await measureOfficialPhase("memory", MEMORY_WORKLOADS);
}
if (requestedPhase === "all" || requestedPhase === "cold") await measureColdLoads();

state.completedAt = new Date().toISOString();
save();
console.log(`Checkpoint written to ${checkpointPath}`);
