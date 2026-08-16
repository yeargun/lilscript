import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import {
  dirname,
  extname,
  join,
  normalize,
  relative,
  resolve,
  sep,
} from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "./playwright-runtime.mjs";
import { median, quantile, requireNonInferiority } from "../statistics.mjs";

const directory = dirname(fileURLToPath(import.meta.url));
const root = resolve(directory, "../..");
const paired = join(root, "benchmarks/paired");
const regressionLimit = 1.03;
const checkOnly = process.argv.includes("--check");
const build = join(directory, "build");
const pairedReportPath = join(build, "paired-results.json");
mkdirSync(build, { recursive: true });

execFileSync(
  process.execPath,
  [
    join(paired, "run.mjs"),
    ...(checkOnly ? ["--check"] : []),
    "--output",
    pairedReportPath,
  ],
  {
    cwd: root,
    stdio: "inherit",
  },
);
const pairedReportBytes = readFileSync(pairedReportPath);
const pairedResults = JSON.parse(pairedReportBytes);

function repositoryEvidencePath(path, label) {
  assert.equal(typeof path, "string", `${label} path`);
  assert.ok(path.length > 0, `${label} path`);
  const absolute = resolve(root, path);
  const repositoryRelative = relative(root, absolute);
  assert.ok(
    repositoryRelative !== "" &&
      repositoryRelative !== ".." &&
      !repositoryRelative.startsWith(`..${sep}`),
    `${label} path must stay inside the repository`,
  );
  return absolute;
}

assert.equal(pairedResults.schemaVersion, 2, "paired report schema");
assert.equal(pairedResults.costModel, "brotli", "paired runtime objective");
assert.equal(
  pairedResults.objectiveContract?.gateMetric,
  "brotli",
  "paired objective gate",
);
assert.equal(
  pairedResults.objectiveContract?.config,
  pairedResults.objectiveConfig?.path,
  "paired objective config identity",
);
assert.match(
  pairedResults.objectiveConfig?.sha256 ?? "",
  /^[a-f0-9]{64}$/u,
  "paired objective config digest",
);
const objectiveConfigPath = repositoryEvidencePath(
  pairedResults.objectiveConfig?.path,
  "paired objective config",
);
assert.equal(
  createHash("sha256").update(readFileSync(objectiveConfigPath)).digest("hex"),
  pairedResults.objectiveConfig.sha256,
  "paired objective config bytes",
);
const compilerPath = repositoryEvidencePath(
  pairedResults.compiler?.path,
  "paired compiler",
);
assert.ok(
  (pairedResults.compiler?.version?.length ?? 0) > 0,
  "paired compiler version",
);
assert.match(
  pairedResults.compiler?.sha256 ?? "",
  /^[a-f0-9]{64}$/u,
  "paired compiler digest",
);
assert.equal(
  createHash("sha256").update(readFileSync(compilerPath)).digest("hex"),
  pairedResults.compiler.sha256,
  "paired compiler bytes",
);
assert.ok(
  Array.isArray(pairedResults.results) && pairedResults.results.length > 0,
  "paired workloads",
);

const server = createServer((request, response) => {
  const pathname = new URL(request.url, "http://127.0.0.1").pathname;
  const requested =
    pathname === "/" ? "benchmarks/browser/index.html" : pathname.slice(1);
  const path = resolve(root, normalize(requested));
  if (!path.startsWith(`${root}/`)) {
    response.writeHead(403).end();
    return;
  }
  try {
    const content = readFileSync(path);
    const type = extname(path) === ".js" ? "text/javascript" : "text/html";
    response.writeHead(200, {
      "content-type": `${type};charset=utf-8`,
      "cache-control": "no-store",
    });
    response.end(content);
  } catch {
    response.writeHead(404).end();
  }
});
await new Promise((resolveReady) =>
  server.listen(0, "127.0.0.1", resolveReady),
);
const { port } = server.address();

const browser = await chromium.launch({ headless: true });
const browserVersion = browser.version();
const page = await browser.newPage();
await page.goto(`http://127.0.0.1:${port}/`);
const results = [];
try {
  for (const benchmark of pairedResults.results) {
    const base = `http://127.0.0.1:${port}/benchmarks/paired/build/${benchmark.id}`;
    const [lilscriptSource, closureSource] = await Promise.all([
      fetch(`${base}/lilscript.js`).then((response) => response.text()),
      fetch(`${base}/closure.js`).then((response) => response.text()),
    ]);
    const canonicalSource = (source) => source.trim().replace(/;$/, "");
    const identicalCode =
      canonicalSource(lilscriptSource) === canonicalSource(closureSource);
    const batches =
      benchmark.id === "bounded-induction"
        ? 200
        : benchmark.id === "exact-wrapping-multiply" ||
            benchmark.id === "ordinary-integer-multiply"
          ? 2000
          : 2000000;
    const timings = await page.evaluate(
      ({ lilscriptSource, closureSource, batches }) => {
        const originalLog = console.log;
        console.log = () => {};
        try {
          const functions = [
            new Function(lilscriptSource),
            new Function(closureSource),
          ];
          const samples = [[], []];
          for (let iteration = 0; iteration < 408; iteration += 1) {
            const first = iteration % 2;
            for (let offset = 0; offset < 2; offset += 1) {
              const artifact = (first + offset) % 2;
              const start = performance.now();
              for (let batch = 0; batch < batches; batch += 1)
                functions[artifact]();
              const elapsed = performance.now() - start;
              if (iteration >= 8) samples[artifact].push(elapsed);
            }
          }
          return samples;
        } finally {
          console.log = originalLog;
        }
      },
      { lilscriptSource, closureSource, batches },
    );
    const statistics = identicalCode
      ? null
      : requireNonInferiority(timings[0], timings[1], {
          label: `${benchmark.id}/Chromium runtime`,
          maxRatio: regressionLimit,
        });
    const upper95 = statistics?.upperConfidenceRatio.median ?? 1;
    const p95Upper95 = statistics?.upperConfidenceRatio.p95 ?? 1;
    const result = {
      id: benchmark.id,
      batches,
      samples: timings[0].length,
      lilscriptMedianMs: median(timings[0]),
      closureMedianMs: median(timings[1]),
      lilscriptP95Ms: quantile(timings[0], 0.95),
      closureP95Ms: quantile(timings[1], 0.95),
      upper95Ratio: upper95,
      p95Upper95Ratio: p95Upper95,
      identicalCode,
    };
    results.push(result);
  }
} finally {
  await browser.close();
  await new Promise((resolveClosed) => server.close(resolveClosed));
}

const report = {
  schemaVersion: 2,
  generatedAt: new Date().toISOString(),
  browser: `Chromium ${browserVersion}`,
  runtimeObjective: pairedResults.costModel,
  artifactSource: "benchmarks/paired Brotli-objective LilScript JavaScript",
  objectiveContract: pairedResults.objectiveContract,
  objectiveConfig: pairedResults.objectiveConfig,
  compiler: pairedResults.compiler,
  codecs: pairedResults.codecs,
  provenance: {
    pairedReport: {
      path: "benchmarks/browser/build/paired-results.json",
      digest: createHash("sha256").update(pairedReportBytes).digest("hex"),
      schemaVersion: pairedResults.schemaVersion,
    },
  },
  regressionLimit,
  results,
};
const serialized = `${JSON.stringify(report, null, 2)}\n`;
if (!checkOnly) {
  writeFileSync(join(directory, "results.json"), serialized);
  writeFileSync(join(root, "web/src/browser-results.json"), serialized);
}
console.log(
  `Chromium runtime gate passed for ${results.length} paired workloads.`,
);
