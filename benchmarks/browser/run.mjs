import { execFileSync } from "node:child_process";
import { createServer } from "node:http";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const directory = dirname(fileURLToPath(import.meta.url));
const root = resolve(directory, "../..");
const paired = join(root, "benchmarks/paired");
const regressionLimit = 1.03;
const checkOnly = process.argv.includes("--check");

execFileSync(process.execPath, [join(paired, "run.mjs"), ...(checkOnly ? ["--check"] : [])], {
  cwd: root,
  stdio: "inherit",
});
const pairedResults = JSON.parse(readFileSync(join(paired, "results.json"), "utf8"));

const server = createServer((request, response) => {
  const pathname = new URL(request.url, "http://127.0.0.1").pathname;
  const requested = pathname === "/" ? "benchmarks/browser/index.html" : pathname.slice(1);
  const path = resolve(root, normalize(requested));
  if (!path.startsWith(`${root}/`)) {
    response.writeHead(403).end();
    return;
  }
  try {
    const content = readFileSync(path);
    const type = extname(path) === ".js" ? "text/javascript" : "text/html";
    response.writeHead(200, { "content-type": `${type};charset=utf-8`, "cache-control": "no-store" });
    response.end(content);
  } catch {
    response.writeHead(404).end();
  }
});
await new Promise((resolveReady) => server.listen(0, "127.0.0.1", resolveReady));
const { port } = server.address();

function quantile(values, fraction) {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.min(sorted.length - 1, Math.floor(fraction * sorted.length))];
}

function median(values) {
  return quantile(values, 0.5);
}

function bootstrapUpperRatio(lilscript, closure) {
  let state = 0x6d2b79f5;
  const random = () => {
    state = Math.imul(state ^ (state >>> 15), state | 1);
    state ^= state + Math.imul(state ^ (state >>> 7), state | 61);
    return ((state ^ (state >>> 14)) >>> 0) / 4294967296;
  };
  const ratios = [];
  for (let sample = 0; sample < 4000; sample += 1) {
    const lilResample = [];
    const closureResample = [];
    for (let index = 0; index < lilscript.length; index += 1) {
      lilResample.push(lilscript[Math.floor(random() * lilscript.length)]);
      closureResample.push(closure[Math.floor(random() * closure.length)]);
    }
    ratios.push(median(lilResample) / median(closureResample));
  }
  return quantile(ratios, 0.95);
}

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
    const identicalCode = canonicalSource(lilscriptSource) === canonicalSource(closureSource);
    const loopHeavy = new Set(["bounded-induction", "exact-wrapping-multiply"]);
    const batches = loopHeavy.has(benchmark.id) ? 200 : 2000000;
    const timings = await page.evaluate(
      ({ lilscriptSource, closureSource, batches }) => {
        const originalLog = console.log;
        console.log = () => {};
        try {
          const functions = [new Function(lilscriptSource), new Function(closureSource)];
          const samples = [[], []];
          for (let iteration = 0; iteration < 58; iteration += 1) {
            const first = iteration % 2;
            for (let offset = 0; offset < 2; offset += 1) {
              const artifact = (first + offset) % 2;
              const start = performance.now();
              for (let batch = 0; batch < batches; batch += 1) functions[artifact]();
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
    const measuredUpper95 = bootstrapUpperRatio(timings[0], timings[1]);
    const upper95 = identicalCode ? 1 : measuredUpper95;
    const result = {
      id: benchmark.id,
      batches,
      samples: timings[0].length,
      lilscriptMedianMs: median(timings[0]),
      closureMedianMs: median(timings[1]),
      upper95Ratio: upper95,
      identicalCode,
    };
    results.push(result);
    if (upper95 > regressionLimit) {
      throw new Error(
        `${benchmark.id}: LilScript/Closure 95% upper runtime ratio ${upper95.toFixed(3)} exceeds ${regressionLimit}`,
      );
    }
  }
} finally {
  await browser.close();
  await new Promise((resolveClosed) => server.close(resolveClosed));
}

const report = {
  generatedAt: new Date().toISOString(),
  browser: `Chromium ${browserVersion}`,
  regressionLimit,
  results,
};
const serialized = `${JSON.stringify(report, null, 2)}\n`;
if (!checkOnly) {
  writeFileSync(join(directory, "results.json"), serialized);
  writeFileSync(join(root, "web/src/browser-results.json"), serialized);
}
console.log(`Chromium runtime gate passed for ${results.length} paired workloads.`);
